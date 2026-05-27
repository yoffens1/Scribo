use std::collections::HashMap;
use std::sync::Arc;
use crate::AppError;
use crate::DbState;
use crate::db::repos::fragments;
use crate::ai::LlmService;
use crate::retrieval::language::detect_language;
use crate::retrieval::types::{
    RetrievalConfig, RetrieveOptions, RetrieveFilters, SearchResult, FragmentRef, FetchQuery, FetchResult
};
use crate::retrieval::fusion::rrf;
use crate::retrieval::stages::{
    hyde::run_hyde,
    synonyms::{expand_static, expand_llm},
    translation::run_translation,
};
use crate::retrieval::rerankers::{
    listwise::rerank_listwise,
    scoring::rerank_scoring,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct QueryVariant {
    text: String,
    lang: String,
    source: String,
    weight: f32,
}

pub fn default_synonyms() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    m.insert("ai".to_string(), vec!["artificial intelligence".to_string(), "machine intelligence".to_string()]);
    m.insert("ml".to_string(), vec!["machine learning".to_string()]);
    m.insert("nn".to_string(), vec!["neural network".to_string(), "neural networks".to_string()]);
    m.insert("dl".to_string(), vec!["deep learning".to_string()]);
    m.insert("nlp".to_string(), vec!["natural language processing".to_string()]);
    m.insert("cv".to_string(), vec!["computer vision".to_string()]);
    m.insert("rl".to_string(), vec!["reinforcement learning".to_string()]);
    m.insert("llm".to_string(), vec!["large language model".to_string(), "large language models".to_string()]);
    m.insert("rag".to_string(), vec!["retrieval augmented generation".to_string()]);
    m
}

fn get_vault_language(state: &DbState) -> String {
    let fragments = state.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT clean_text FROM chunks WHERE level = 1 AND clean_text IS NOT NULL LIMIT 50")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }).unwrap_or_default();

    if fragments.is_empty() {
        return "en".to_string();
    }

    let mut counts = HashMap::new();
    for text in fragments {
        if text.trim().len() < 10 {
            continue;
        }
        if let Some(lang) = detect_language(&text) {
            *counts.entry(lang).or_insert(0) += 1;
        }
    }

    let mut best_lang = "en".to_string();
    let mut max_count = 0;
    for (l, c) in counts {
        if c > max_count {
            max_count = c;
            best_lang = l;
        }
    }
    best_lang
}

fn dedup_variants(variants: Vec<QueryVariant>) -> Vec<QueryVariant> {
    let mut seen: HashMap<String, QueryVariant> = HashMap::new();
    for v in variants {
        let key = v.text.to_lowercase().trim().to_string();
        let existing = seen.get(&key);
        if existing.is_none() || v.weight > existing.unwrap().weight {
            seen.insert(key, v);
        }
    }
    seen.into_values().collect()
}

fn apply_filters(results: Vec<SearchResult>, filters: &Option<RetrieveFilters>) -> Vec<SearchResult> {
    if let Some(ref f) = filters {
        results.into_iter().filter(|r| {
            if let Some(ref note_id) = f.note_id {
                if r.fragment_ref.note_id != *note_id {
                    return false;
                }
            }
            true
        }).collect()
    } else {
        results
    }
}

fn hydrate_texts(state: &DbState, candidates: &mut [SearchResult]) {
    let mut by_note: HashMap<i64, Vec<usize>> = HashMap::new();
    for (idx, c) in candidates.iter().enumerate() {
        if c.text.is_none() || c.text.as_ref().unwrap().is_empty() {
            by_note.entry(c.fragment_ref.note_id.0).or_default().push(idx);
        }
    }

    for (note_id, indices) in by_note {
        if let Ok(fragments) = state.with_conn(|conn| fragments::list_fragments_with_note(conn, Some(note_id), false)) {
            let by_index: HashMap<usize, String> = fragments.into_iter()
                .map(|ch| (ch.fragment.fragment_index as usize, ch.fragment.text_clean))
                .collect();
            for idx in indices {
                let fragment_idx = candidates[idx].fragment_ref.fragment_index;
                if let Some(text) = by_index.get(&fragment_idx) {
                    candidates[idx].text = Some(text.clone());
                }
            }
        }
    }
}

pub async fn retrieve(
    state: &DbState,
    query: &str,
    query_embedding: Option<&[f32]>,
    config: &RetrievalConfig,
    options: &RetrieveOptions,
) -> Result<Vec<SearchResult>, AppError> {
    let detected_lang = detect_language(query).unwrap_or_else(|| "en".to_string());
    let vault_lang = config.vault_lang.clone().unwrap_or_else(|| get_vault_language(state));

    let llm_service = if let Some(llm_cfg) = &config.llm_config {
        Some(Arc::new(LlmService::new(llm_cfg.clone(), None)))
    } else {
        None
    };

    let mut variants = vec![QueryVariant {
        text: query.to_string(),
        lang: detected_lang.clone(),
        source: "original".to_string(),
        weight: 1.0,
    }];

    // 1. Translation Stage
    let pipeline = config.pipeline.as_ref();
    let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
    if auto_translate && detected_lang != vault_lang {
        if let Some(ref llm) = llm_service {
            if let Some(translated) = run_translation(llm, query, &vault_lang).await {
                variants.push(QueryVariant {
                    text: translated,
                    lang: vault_lang.clone(),
                    source: "translated".to_string(),
                    weight: 1.0,
                });
            }
        }
    }

    // 2. HyDE Stage
    let hyde_enabled = pipeline.and_then(|p| p.hyde).unwrap_or(false);
    let mut hyde_variant: Option<QueryVariant> = None;
    if hyde_enabled {
        if let Some(ref llm) = llm_service {
            let target_lang = if auto_translate { &vault_lang } else { &detected_lang };
            if let Some(hyde_text) = run_hyde(llm, query, target_lang).await {
                hyde_variant = Some(QueryVariant {
                    text: hyde_text,
                    lang: target_lang.clone(),
                    source: "hyde".to_string(),
                    weight: 0.8,
                });
            }
        }
    }

    // 3. Synonym Expansion Stage
    let expand_synonyms = pipeline.and_then(|p| p.expand_synonyms.as_deref()).unwrap_or("off");
    if expand_synonyms == "static" {
        let default_dict = default_synonyms();
        let dict = pipeline.and_then(|p| p.synonym_dict.as_ref()).unwrap_or(&default_dict);
        let mut new_syns = Vec::new();
        for v in &variants {
            let syns = expand_static(&v.text, dict);
            for syn in syns {
                new_syns.push(QueryVariant {
                    text: syn,
                    lang: v.lang.clone(),
                    source: "synonym".to_string(),
                    weight: 0.6,
                });
            }
        }
        variants.extend(new_syns);
    } else if expand_synonyms == "llm" {
        if let Some(ref llm) = llm_service {
            let target_lang = if auto_translate { &vault_lang } else { &detected_lang };
            let syns = expand_llm(llm, query, target_lang).await;
            for syn in syns {
                variants.push(QueryVariant {
                    text: syn,
                    lang: target_lang.clone(),
                    source: "synonym".to_string(),
                    weight: 0.6,
                });
            }
        }
    }

    // Deduplicate query variants
    let deduped = dedup_variants(variants);

    // Apply adaptive weights
    let mut weighted = Vec::new();
    for mut v in deduped {
        if v.lang == vault_lang {
            v.weight *= 1.5;
        } else {
            v.weight *= 0.5;
        }
        weighted.push(v);
    }

    // 4. Retrieve for each variant
    let top_k = options.top_k.unwrap_or(5);
    let over_fetch = (top_k * 3).min(50); // cap inner retrieves at 50

    let mut variant_lists = Vec::new();

    for v in weighted {
        let mut keyword_results = Vec::new();
        let mut vector_results = Vec::new();

        if config.mode == "keyword" || config.mode == "hybrid" {
            if let Ok(hits) = state.with_conn(|conn| fragments::search(conn, &v.text, over_fetch as i64)) {
                keyword_results = hits.into_iter().map(|h| SearchResult {
                    fragment_ref: FragmentRef {
                        note_id: h.hit.note_id,
                        fragment_index: h.hit.fragment_index as usize,
                    },
                    score: h.score,
                    text: h.hit.snippet,
                }).collect();
            }
        }

        if config.mode == "embedding" || config.mode == "hybrid" {
            let emb_to_use = if v.text == query && query_embedding.is_some() {
                query_embedding.map(|e| e.to_vec())
            } else if let Some(ref llm) = llm_service {
                if let Ok(embs) = llm.generate_embeddings(vec![v.text.clone()]).await {
                    embs.into_iter().next()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(emb) = emb_to_use {
                let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb).to_vec();
                if let Ok(hits) = state.with_conn(|conn| fragments::vector_search(conn, &emb_bytes, options.target_level, over_fetch)) {
                    vector_results = hits.into_iter().map(|h| SearchResult {
                        fragment_ref: FragmentRef {
                            note_id: h.hit.note_id,
                            fragment_index: h.hit.fragment_index as usize,
                        },
                        score: h.score,
                        text: Some(h.hit.text),
                    }).collect();
                }
            }
        }

        let results = if config.mode == "hybrid" {
            rrf(
                vec![
                    (keyword_results, 1.0),
                    (vector_results, config.embedding_weight.unwrap_or(1.0)),
                ],
                60.0,
                over_fetch,
            )
        } else if config.mode == "keyword" {
            keyword_results
        } else {
            vector_results
        };

        variant_lists.push((results, v.weight));
    }

    // HyDE list (if enabled)
    if let Some(hyde) = hyde_variant {
        if let Some(ref llm) = llm_service {
            if let Ok(embs) = llm.generate_embeddings(vec![hyde.text.clone()]).await {
                if let Some(emb) = embs.into_iter().next() {
                    let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb).to_vec();
                    if let Ok(hits) = state.with_conn(|conn| fragments::vector_search(conn, &emb_bytes, options.target_level, over_fetch)) {
                        let hyde_results: Vec<SearchResult> = hits.into_iter().map(|h| SearchResult {
                            fragment_ref: FragmentRef {
                                note_id: h.hit.note_id,
                                fragment_index: h.hit.fragment_index as usize,
                            },
                            score: h.score,
                            text: Some(h.hit.text),
                        }).collect();
                        variant_lists.push((hyde_results, hyde.weight));
                    }
                }
            }
        }
    }

    // Fuse all lists with RRF
    let candidates_limit = if config.ai_rerank.as_ref().map(|r| r.enabled).unwrap_or(false) {
        top_k * 4
    } else {
        top_k
    };
    let mut fused = rrf(variant_lists, 60.0, candidates_limit);

    // Apply filters
    fused = apply_filters(fused, &options.filters);

    // 5. Reranking Stage
    let rerank_config = config.ai_rerank.as_ref();
    if let Some(rc) = rerank_config {
        if rc.enabled && !fused.is_empty() {
            if let Some(ref llm) = llm_service {
                // Hydrate texts from DB
                hydrate_texts(state, &mut fused);

                let max_cands = rc.max_candidates.unwrap_or(25).min(fused.len());
                let rerank_mode = rc.mode.as_deref().unwrap_or("scoring");

                if rerank_mode == "listwise" {
                    if let Some(reranked) = rerank_listwise(llm, query, &fused[..max_cands]).await {
                        let mut final_fused = reranked;
                        if fused.len() > max_cands {
                            final_fused.extend(fused[max_cands..].iter().cloned());
                        }
                        fused = final_fused;
                    }
                } else {
                    // "scoring" mode
                    let (to_sort, _rest) = fused.split_at_mut(max_cands);
                    rerank_scoring(llm, query, to_sort).await;
                }
            }
        }
    }

    fused.truncate(top_k);
    Ok(fused)
}

pub fn fetch(
    state: &DbState,
    query: &FetchQuery,
) -> Result<Vec<FetchResult>, AppError> {
    let include_deleted = query.include_deleted.unwrap_or(false);
    let raw = state.with_conn(|conn| {
        fragments::list_fragments_with_note(
            conn,
            query.note_id.map(|id| id.0),
            include_deleted,
        )
    })?;

    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(raw.len());

    let page = if offset >= raw.len() {
        Vec::new()
    } else {
        let end = (offset + limit).min(raw.len());
        raw[offset..end].to_vec()
    };

    let results = page.into_iter().map(|ch| FetchResult {
        fragment_id: Some(ch.fragment.id.0),
        note_id: ch.fragment.note_id,
        fragment_index: ch.fragment.fragment_index as usize,
        fragment_text: Some(ch.fragment.text_clean),
        token_count: ch.fragment.token_count,
        embedding: ch.fragment.embedding.unwrap_or_default(),
    }).collect();

    Ok(results)
}

//! # Query Variant Building
//!
//! Builds and deduplicates the set of [`QueryVariant`]s from the original query
//! by running translation, HyDE, and synonym expansion in sequence.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use crate::ai::LlmService;
use crate::retrieval::types::{
    QueryVariant, VariantSource, SynonymExpansion, RetrievalConfig, RetrievalTuning,
};
use crate::retrieval::cache::cached_or_run;
use super::{maybe_translate_query, run_hyde, expand_static, expand_llm};

static SYNONYMS: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

/// Returns the global static synonym dictionary loaded from `data/synonyms.json`.
pub fn default_synonyms() -> &'static HashMap<String, Vec<String>> {
    SYNONYMS.get_or_init(|| {
        let json_str = include_str!("../../../data/synonyms.json");
        serde_json::from_str(json_str).unwrap_or_default()
    })
}

/// Deduplicates query variants and fuses their weights.
/// Variants with the same normalized text and language are merged by combining their weights.
pub fn dedup_variants(variants: Vec<QueryVariant>) -> Vec<QueryVariant> {
    let mut seen: HashMap<(String, String), QueryVariant> = HashMap::new();
    for v in variants {
        let key = (v.text.trim().to_lowercase(), v.lang.clone());
        match seen.entry(key) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(v);
            }
            std::collections::hash_map::Entry::Occupied(mut e) => {
                let existing = e.get_mut();
                existing.weight = existing.weight.max(v.weight) + 0.1 * existing.weight.min(v.weight);
            }
        }
    }
    seen.into_values().collect()
}

/// Builds the full list of weighted [`QueryVariant`]s from the original query.
///
/// Runs translation, HyDE, and synonym expansion stages in order,
/// then deduplicates and applies adaptive language weights.
pub async fn build_variants(
    state: &crate::DbState,
    llm: Option<&Arc<LlmService>>,
    config: &RetrievalConfig,
    tuning: &RetrievalTuning,
    query: &str,
    detected_lang: &str,
    vault_lang: &str,
) -> Vec<QueryVariant> {
    let pipeline = config.pipeline.as_ref();

    let mut variants = vec![QueryVariant {
        text: query.to_string(),
        lang: detected_lang.to_string(),
        source: VariantSource::Original,
        weight: 1.0,
        vector_only: false,
    }];

    // 1. Translation Stage
    let auto_translate = pipeline.and_then(|p| p.auto_translate).unwrap_or(false);
    if auto_translate && detected_lang != vault_lang {
        if let Some(llm) = llm {
            let model_id = llm.config().model.clone();
            let translated = cached_or_run(
                state,
                query,
                &model_id,
                "translation",
                vault_lang,
                maybe_translate_query(llm, query, vault_lang)
            ).await;
            if let Some(text) = translated {
                variants.push(QueryVariant {
                    text,
                    lang: vault_lang.to_string(),
                    source: VariantSource::Translated,
                    weight: 1.0,
                    vector_only: false,
                });
            }
        }
    }

    // 2. HyDE Stage
    let hyde_enabled = pipeline.and_then(|p| p.hyde).unwrap_or(false);
    if hyde_enabled {
        if let Some(llm) = llm {
            let model_id = llm.config().model.clone();
            let target_lang = if auto_translate { vault_lang } else { detected_lang };
            let hyde_text = cached_or_run(
                state,
                query,
                &model_id,
                "hyde",
                target_lang,
                run_hyde(llm, query, target_lang)
            ).await;
            if let Some(text) = hyde_text {
                variants.push(QueryVariant {
                    text,
                    lang: target_lang.to_string(),
                    source: VariantSource::Hyde,
                    weight: tuning.hyde_weight,
                    vector_only: true,
                });
            }
        }
    }

    // 3. Synonym Expansion Stage
    let expand_synonyms = pipeline.and_then(|p| p.expand_synonyms).unwrap_or(SynonymExpansion::Off);
    let mut new_syns = Vec::new();
    if expand_synonyms == SynonymExpansion::Static {
        let default_dict = default_synonyms();
        let dict = pipeline.and_then(|p| p.synonym_dict.as_ref()).unwrap_or(default_dict);
        for v in &variants {
            if v.vector_only { continue; }
            let syns = expand_static(&v.text, dict);
            for syn in syns {
                new_syns.push(QueryVariant {
                    text: syn,
                    lang: v.lang.clone(),
                    source: VariantSource::Synonym,
                    weight: tuning.synonym_weight,
                    vector_only: false,
                });
            }
        }
    } else if expand_synonyms == SynonymExpansion::Llm {
        if let Some(llm) = llm {
            let target_lang = if auto_translate { vault_lang } else { detected_lang };
            let syns = expand_llm(llm, query, target_lang).await;
            for syn in syns {
                new_syns.push(QueryVariant {
                    text: syn,
                    lang: target_lang.to_string(),
                    source: VariantSource::Synonym,
                    weight: tuning.synonym_weight,
                    vector_only: false,
                });
            }
        }
    }
    variants.extend(new_syns);

    // Deduplication + adaptive language weights
    let mut weighted = Vec::new();
    let adaptive = config.adaptive_weights.clone().unwrap_or_default();
    for mut v in dedup_variants(variants) {
        if v.lang == vault_lang {
            v.weight *= adaptive.same_lang;
        } else {
            v.weight *= adaptive.other_lang;
        }
        weighted.push(v);
    }

    weighted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::types::{QueryVariant, VariantSource};

    #[test]
    fn test_dedup_variants_weights() {
        let variants = vec![
            QueryVariant {
                text: "Query text".to_string(),
                lang: "en".to_string(),
                source: VariantSource::Original,
                weight: 1.0,
                vector_only: false,
            },
            QueryVariant {
                text: "query text ".to_string(),
                lang: "en".to_string(),
                source: VariantSource::Synonym,
                weight: 0.6,
                vector_only: false,
            },
            QueryVariant {
                text: "Another".to_string(),
                lang: "en".to_string(),
                source: VariantSource::Translated,
                weight: 0.8,
                vector_only: false,
            },
        ];

        let deduped = dedup_variants(variants);
        assert_eq!(deduped.len(), 2);

        let qt = deduped.iter().find(|v| v.text.trim().to_lowercase() == "query text").unwrap();
        // weight calculation: max(1.0, 0.6) + 0.1 * min(1.0, 0.6) = 1.0 + 0.06 = 1.06
        assert!((qt.weight - 1.06).abs() < 1e-5);

        let another = deduped.iter().find(|v| v.text == "Another").unwrap();
        assert_eq!(another.weight, 0.8);
    }
}

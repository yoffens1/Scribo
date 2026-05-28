# Retrieval Subsystem

This directory contains the core hybrid retrieval engine of Scribo. It integrates keyword search (FTS5 BM25) and semantic vector search (cosine similarity scanning) using Reciprocal Rank Fusion (RRF) and LLM-driven reranking.

## Execution Flow

The typical lifecycle of a search/RAG query proceeds as follows:

```
1. CLI/Tauri Command (e.g. cli/query.rs or commands/search.rs)
   └─ Parses incoming arguments and options.

2. search service (services/search.rs)
   └─ Resolves DbState and prepares the RetrievalConfig.

3. retrieval pipeline (retrieval/pipeline.rs::retrieve)
   ├─ stages/translate_query.rs (optional query translation)
   ├─ stages/hyde.rs            (optional synthetic document generation)
   ├─ stages/synonyms.rs        (optional query expansion)
   │
   ├─ Parallel Execution (per variant):
   │   ├─ db/repos/fragments.rs::search        (Keyword FTS5 branch)
   │   └─ db/repos/fragments.rs::vector_search (Vector Cosine branch)
   │
   ├─ fusion.rs::rrf (Reciprocal Rank Fusion over branches and variants)
   ├─ rerankers/ (optional scoring or listwise LLM reranking)
   │
   └─ Returns: Vec<SearchResult>
```

## Module Directory Structure

- `pipeline.rs` — Central orchestrator defining `retrieve` and `fetch`.
- `types.rs` — Public configurations, filter/retrieve options and API structs.
- `fusion.rs` — Reciprocal Rank Fusion (RRF) algorithm.
- `language.rs` — Language detection and mapping logic.
- `stages/` — Pre-fusion query expansions/manipulations:
  - `translate_query.rs` — Query translation stage.
  - `hyde.rs` — Hypothetical Document Embeddings stage.
  - `synonyms.rs` — Static and LLM-driven synonym expansion.
- `rerankers/` — Post-fusion LLM-driven ranking updates.

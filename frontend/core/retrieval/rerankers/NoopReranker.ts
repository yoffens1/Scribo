// src/core/retrieval/rerankers/NoopReranker.ts
import type { Reranker } from "./types/reranker";
import type { SearchResult } from "../types/search";

/** Pass-through reranker — returns candidates unchanged. */
export class NoopReranker implements Reranker {
  async rerank(_query: string, candidates: SearchResult[]): Promise<SearchResult[]> {
    return candidates;
  }
}

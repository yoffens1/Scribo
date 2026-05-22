// src/core/retrieval/rerankers/IReranker.ts
import type { SearchResult } from "../../types/search";

export interface Reranker {
  /**
   * Re-rank candidates for a given query.
   * @param query — original user query
   * @param candidates — over-fetched results from primary retrieval
   * @returns re-ranked results (same set, reordered + rescored)
   */
  rerank(query: string, candidates: SearchResult[]): Promise<SearchResult[]>;
}

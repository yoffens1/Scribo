// src/core/rag/KeywordRetriever.ts
import type { Retriever, RetrieveOptions } from "./types";
import type { SearchResult } from "../types/search";
import { applyFilters } from "./filter";

/**
 * Keyword BM25 retriever.
 *
 * Accepts a factory for the BM25 index rather than a cached instance —
 * every retrieve() gets the latest index from the registry so markDirty()
 * works without recreating the retriever chain.
 */
export class KeywordRetriever implements Retriever {
  constructor(private bm25Factory: () => any) {}

  async retrieve(
    query: string,
    options?: RetrieveOptions,
  ): Promise<SearchResult[]> {
    const index = await this.bm25Factory();
    const bm25Results = index.search(query, options?.topK ?? 5);
    const results: SearchResult[] = bm25Results.map((r: any) => ({
      chunkRef: r.chunkRef,
      score: r.score,
    }));
    return applyFilters(results, options);
  }
}

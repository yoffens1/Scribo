// src/core/rag/EmbeddingRetriever.ts
import type { Retriever, RetrieveOptions } from "./types";
import type { SearchResult } from "../types/search";
import { applyFilters } from "./filter";

/**
 * Embedding-based retriever.
 *
 * Accepts a factory function for the SearchEngine rather than a cached
 * instance — every retrieve() gets the latest engine from the registry.
 * This avoids stale index references after markDirty() without recreating
 * the retriever chain.
 */
export class EmbeddingRetriever implements Retriever {
  constructor(private engineFactory: () => any) {}

  async retrieve(
    query: string,
    options?: RetrieveOptions,
  ): Promise<SearchResult[]> {
    const engine = await this.engineFactory();
    const results = await engine.search(query, options?.topK ?? 5);
    return applyFilters(results, options);
  }
}

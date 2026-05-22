// src/core/rag/HybridRetriever.ts
import type { Retriever, RetrieveOptions } from "./types";
import type { SearchResult } from "../types/search";
import { rrf } from "./fusion/rrf";
import { applyFilters } from './filter';
import { RETRIEVAL_CONSTANTS } from '../constants';
import { RetrievalLogger } from "../logging/RetrievalLogger";

export class HybridRetriever implements Retriever {
  constructor(
    private embedding: Retriever,
    private keyword: Retriever,
    private k = 60,
    private overFetchMultiplier = 3,
    private embeddingWeight = 1,
    private logger?: RetrievalLogger,
  ) {}

  async retrieve(
    query: string,
    options?: RetrieveOptions,
  ): Promise<SearchResult[]> {
    const topK = options?.topK ?? 5;
    const overFetch = Math.min(topK * this.overFetchMultiplier, RETRIEVAL_CONSTANTS.MAX_OVERFETCH);
    this.logger?.log("debug", "hybrid.start", "fetching", { topK, overFetch });

    const t0 = performance.now();
    const [vecResults, keyResults] = await Promise.all([
      this.embedding.retrieve(query, { topK: overFetch }),
      this.keyword.retrieve(query, { topK: overFetch }),
    ]);
    const fetchMs = (performance.now() - t0).toFixed(0);

    this.logger?.log("debug", "hybrid.retrieved", "got results", {
      embeddingCount: vecResults.length,
      keywordCount: keyResults.length,
      durationMs: Number(fetchMs),
    });

    const fused = rrf(
      [
        { results: vecResults, weight: this.embeddingWeight },
        { results: keyResults, weight: 1 },
      ],
      this.k,
      topK,
    );

    this.logger?.log("info", "hybrid.rrf", "fused", {
      inputLists: 2,
      outputCount: fused.length,
      topScores: fused.slice(0, 3).map(r => r.score.toFixed(4)),
    });

    return applyFilters(fused, options);
  }
}

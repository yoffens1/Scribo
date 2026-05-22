// src/core/retrieval/retrievers/RerankingRetriever.ts
import type { Retriever, RetrieveOptions } from "./types";
import type { Reranker } from "../rerankers/types/reranker";
import type { SearchResult } from "../types/search";
import type { ChunkSource } from "../types/chunk-source";
import { RetrievalLogger } from "../logging/RetrievalLogger";
import { RETRIEVAL_CONSTANTS } from "../constants";

export class RerankingRetriever implements Retriever {
  constructor(
    private inner: Retriever,
    private reranker: Reranker,
    /** For hydrating text from DB when inner retriever returns undefined text. */
    private chunkSource?: ChunkSource,
    private overFetch = 4,
    private logger?: RetrievalLogger,
  ) {}

  async retrieve(
    query: string,
    options?: RetrieveOptions,
  ): Promise<SearchResult[]> {
    const topK = options?.topK ?? 5;
    // Fetch topK × overFetch candidates, capped to avoid explosive chains.
    // Chain: MultiQuery(topK×3,capped) → Hybrid(topK×overFetch,capped) → Rerank(topK×4,capped)
    // Worst-case: 5×3→15, 15×3→45, 45×4→50 (cap) — single inner call max 50.
    let candidates = await this.inner.retrieve(query, { topK: Math.min(topK * this.overFetch, RETRIEVAL_CONSTANTS.MAX_OVERFETCH) });

    // Hydrate missing texts from DB.
    // Batch by filePath: one getByFilePath() call per unique file, not N per candidate.
    if (this.chunkSource) {
      const byPath = new Map<string, SearchResult[]>();
      for (const c of candidates) {
        if (c.text) continue;
        const arr = byPath.get(c.chunkRef.filePath);
        if (arr) arr.push(c);
        else byPath.set(c.chunkRef.filePath, [c]);
      }

      for (const [filePath, group] of byPath) {
        const chunks = await this.chunkSource.getByFilePath(filePath);
        const byIndex = new Map(chunks.map(ch => [ch.chunkIndex, ch]));
        for (const c of group) {
          c.text = byIndex.get(c.chunkRef.chunkIndex)?.chunkText ?? "";
        }
      }
    }

    console.debug(
      `[Reranker] candidates=${candidates.length} type=${this.reranker.constructor.name} → awaiting rerank`,
    );
    this.logger?.log("info", "rerank.before", "candidates", {
      count: candidates.length,
      topScores: candidates.slice(0, 5).map(c => c.score),
    });

    const t0 = performance.now();
    const reranked = await this.reranker.rerank(query, candidates);
    const rerankMs = (performance.now() - t0).toFixed(0);

    console.debug(
      `[Reranker] reranked=${reranked.length} → topK=${topK}`,
    );
    this.logger?.log("info", "rerank.after", "reranked", {
      count: reranked.length,
      durationMs: Number(rerankMs),
      reorder: reranked.slice(0, 5).map(r => ({
        chunk: `${r.chunkRef.filePath}#${r.chunkRef.chunkIndex}`,
        score: r.score,
      })),
    });

    return reranked.slice(0, topK);
  }
}

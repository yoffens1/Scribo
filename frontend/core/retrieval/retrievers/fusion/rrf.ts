// src/core/retrieval/retrievers/rrf.ts
import type { SearchResult } from "../../types/search";

/**
 * Reciprocal Rank Fusion with optional per-list weights.
 *
 * Formula: score(d) = Σ weight_r / (k + rank_r(d))
 *   k = 60 (magic constant from Cormack et al.)
 *   rank_r(d) = 1-based position of document d in list r
 *   weight_r = per-list bias (default 1)
 */
export function rrf(
  rankedLists: Array<{ results: SearchResult[]; weight?: number }>,
  k = 60,
  topK = 5,
): SearchResult[] {
  const fused = new Map<string, { result: SearchResult; score: number }>();

  for (const { results, weight = 1 } of rankedLists) {
    results.forEach((r, idx) => {
      // \u0000 can't appear in file paths → collision-free key
      const id = `${r.chunkRef.filePath}\u0000${r.chunkRef.chunkIndex}`;
      const contribution = weight / (k + idx + 1);
      const existing = fused.get(id);
      if (existing) {
        existing.score += contribution;
      } else {
        fused.set(id, { result: r, score: contribution });
      }
    });
  }

  return [...fused.values()]
    .sort((a, b) => b.score - a.score)
    .slice(0, topK)
    .map((x) => ({ ...x.result, score: x.score }));
}

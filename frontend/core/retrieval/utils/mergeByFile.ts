// src/core/retrieval/utils/mergeByFile.ts
import type { SearchResult } from "../types/search";

export interface FileResult {
  filePath: string;
  score: number;
  chunkCount: number;
  topChunk: { chunkIndex: number; text: string };
}

/**
 * Merge chunk-level results into file-level results.
 * Groups by filePath, takes the best score per file, returns unique files.
 */
export function mergeByFile(
  results: SearchResult[],
): FileResult[] {
  const fileMap = new Map<string, { bestScore: number; chunks: SearchResult[] }>();

  for (const r of results) {
    const existing = fileMap.get(r.chunkRef.filePath);
    if (existing) {
      if (r.score > existing.bestScore) existing.bestScore = r.score;
      existing.chunks.push(r);
    } else {
      fileMap.set(r.chunkRef.filePath, { bestScore: r.score, chunks: [r] });
    }
  }

  return Array.from(fileMap.entries())
    .map(([filePath, { bestScore, chunks }]) => {
      // Find the chunk with the best score (or first if tie)
      const topChunk = chunks.reduce((best, c) => c.score > best.score ? c : best, chunks[0]);
      return {
        filePath,
        score: bestScore,
        chunkCount: chunks.length,
        topChunk: {
          chunkIndex: topChunk.chunkRef.chunkIndex,
          text: topChunk.text ?? "",
        },
      };
    })
    .sort((a, b) => b.score - a.score);
}

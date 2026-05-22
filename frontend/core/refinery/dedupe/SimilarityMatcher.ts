// src/core/refinery/dedupe/SimilarityMatcher.ts
import type { RetrievalService } from "@retrieval/RetrievalService";
import type { ChunkWithHash, ChunkDecision } from "../types/chunk-decision";
import { REFINERY_CONSTANTS } from "../constants";

/**
 * Queries the output vault's retrieval index to find near-duplicate chunks.
 */
export class SimilarityMatcher {
  constructor(
    private retrieval: RetrievalService,
    private threshold = REFINERY_CONSTANTS.MERGE_SIMILARITY_THRESHOLD,
    private topK = REFINERY_CONSTANTS.DEDUP_TOP_K,
  ) {}

  /**
   * Find similar chunks in the output vault for a given incoming chunk.
   * Returns the best match if above threshold, or null.
   */
  async findBestMatch(chunk: ChunkWithHash): Promise<{
    filePath: string;
    chunkText: string;
    score: number;
  } | null> {
    if (!chunk.embeddingText || chunk.embeddingText.length < REFINERY_CONSTANTS.MIN_CHUNK_LENGTH_FOR_MERGE) {
      return null;
    }

    const results = await this.retrieval.query(chunk.embeddingText, {
      topK: this.topK,
    });

    const filtered = results.filter(r => r.chunkRef.filePath !== chunk.sourcePath);
    if (filtered.length === 0) return null;

    const best = filtered[0];
    if (best.score < this.threshold) return null;

    return {
      filePath: best.chunkRef.filePath,
      chunkText: best.text ?? "",
      score: best.score,
    };
  }

  /**
   * Classify a chunk as merge/keep/reject based on similarity results.
   */
  async classify(chunk: ChunkWithHash): Promise<ChunkDecision> {
    const match = await this.findBestMatch(chunk);

    if (!match) {
      return { action: "keep", chunk, reason: "no similar chunk found" };
    }

    if (match.score >= REFINERY_CONSTANTS.NEAR_DUP_THRESHOLD) {
      return {
        action: "reject",
        chunk,
        reason: `near-exact duplicate of ${match.filePath} (score: ${match.score.toFixed(3)})`,
      };
    }

    return {
      action: "merge",
      targetPath: match.filePath,
      sourceChunk: chunk,
      reason: `similar to ${match.filePath} (score: ${match.score.toFixed(3)})`,
    };
  }
}

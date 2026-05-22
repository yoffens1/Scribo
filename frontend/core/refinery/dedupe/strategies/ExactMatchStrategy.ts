// src/core/refinery/dedupe/strategies/ExactMatchStrategy.ts
import type { ChunkWithHash } from "../../types/chunk-decision";

export interface MergeStrategy {
  readonly name: string;
  canHandle(existing: string, incoming: ChunkWithHash): boolean;
  merge(existing: string, incoming: ChunkWithHash): Promise<string>;
}

/**
 * Exact text match — if two chunks have identical text, keep the longer one.
 */
export class ExactMatchStrategy implements MergeStrategy {
  readonly name = "exact-match";

  canHandle(existing: string, incoming: ChunkWithHash): boolean {
    return existing.trim() === incoming.generationText.trim();
  }

  async merge(existing: string, incoming: ChunkWithHash): Promise<string> {
    // Keep the longer version (might have more context)
    return existing.length >= incoming.generationText.length ? existing : incoming.generationText;
  }
}

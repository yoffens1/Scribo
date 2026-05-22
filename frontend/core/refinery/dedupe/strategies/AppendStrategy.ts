// src/core/refinery/dedupe/strategies/AppendStrategy.ts
import type { ChunkWithHash } from "../../types/chunk-decision";
import type { MergeStrategy } from "./ExactMatchStrategy";

/**
 * Simple append — when chunks are similar but not duplicates,
 * append the new content to the existing file.
 */
export class AppendStrategy implements MergeStrategy {
  readonly name = "append";

  canHandle(_existing: string, _incoming: ChunkWithHash): boolean {
    // This can always handle, but should be prioritized last
    return true;
  }

  async merge(existing: string, incoming: ChunkWithHash): Promise<string> {
    return existing + "\n\n---\n\n" + incoming.generationText;
  }
}

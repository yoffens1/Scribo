// src/test/refinery/helpers/mkChunk.ts
import type { ChunkWithHash } from "@refinery/types/chunk-decision";

/**
 * Create a minimal ChunkWithHash for tests.
 * Sets both embeddingText and generationText (and .text for compat).
 * Each call with a numeric suffix produces deterministic hashes.
 */
let _seq = 0;
export const mkChunk = (text = "default chunk content long enough for dedup", hash?: string): ChunkWithHash => ({
  hash: hash ?? `hash-${++_seq}`,
  embeddingText: text,
  generationText: text,
  text,
  index: 0,
  sourcePath: "inbox/test.md",
});

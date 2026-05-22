// src/core/refinery/types/chunk-decision.ts

/** Decision on what to do with an incoming chunk after deduplication. */
export type ChunkDecision =
  | { action: "merge"; targetPath: string; sourceChunk: ChunkWithHash; reason: string }
  | { action: "keep"; chunk: ChunkWithHash; reason: string }
  | { action: "reject"; chunk: ChunkWithHash; reason: string };

/** A chunk with its stable content hash and twin text representations. */
export interface ChunkWithHash {
  /** Stable SHA-256 hash — computed from embeddingText (canonical form). */
  hash: string;
  /** Cleaned text for similarity search, dedup, and embedding indexing.
   *  This is the canonical form: lowercase, no links, no formatting markers. */
  embeddingText: string;
  /** Original markdown text for LLM prompts, file writes, and merges.
   *  Preserves [[wikilinks]], **bold**, *italic*, tables, LaTeX, etc. */
  generationText: string;
  /** Backward-compat alias — points to embeddingText.
   *  @deprecated Use embeddingText or generationText explicitly. */
  text: string;
  /** Position in the source file (0-based). */
  index: number;
  /** Source file path (in inbox). */
  sourcePath: string;
  /** Optional metadata from YAML frontmatter. */
  metadata?: Record<string, unknown>;
}

/** Result of the deduplication stage. */
export interface DeduplicationResult {
  decisions: ChunkDecision[];
  /** Chunks that survived dedup and should proceed to taxonomy. */
  remaining: ChunkWithHash[];
}

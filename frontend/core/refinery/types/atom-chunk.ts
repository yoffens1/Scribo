// src/core/refinery/types/atom-chunk.ts
import type { ChunkWithHash } from "./chunk-decision";

/**
 * AtomChunk extends ChunkWithHash with metadata added by downstream stages.
 *
 * Stage pipeline:
 *   ChunkingStage → base fields (hash, text, index, sourcePath)
 *   AtomizationStage → questionHeading, filename
 *   EnrichmentStage → aliases, tags
 *
 * Consequence: early stages (Dedup) see undefined for late fields —
 * they only read hash/text, not affected.
 */
export interface AtomChunk extends ChunkWithHash {
  /** LLM-generated question-heading (e.g. "## What is an atom?") */
  questionHeading?: string;
  /** LLM-generated filename (e.g. "Atom.md" or just "Atom") */
  filename?: string;
  /** Aliases / synonyms extracted by LLM */
  aliases?: string[];
  /** Tags / categories extracted by LLM */
  tags?: string[];
  /** Source file references */
  sources?: string[];
  /** Whether this chunk is primarily a table */
  isTable?: boolean;
}

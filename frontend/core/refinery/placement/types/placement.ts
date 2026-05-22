// src/core/refinery/placement/types/placement.ts

/**
 * A single placement decision for one chunk.
 */
export interface PlacementDecision {
  /** Chunk hash this decision applies to. */
  chunkHash: string;
  /** Final output path for this chunk (relative to outputRoot). */
  outputPath: string;
  /** The action to take. */
  action: "create" | "merge" | "rename" | "nest";
  /** If action is "merge" or "nest", the existing target path. */
  existingTarget?: string;
  /** If action is "rename", the proposed name that should be renamed. */
  proposedRename?: string;
  /** Brief rationale from the LLM. */
  reason: string;
}

/**
 * Full placement plan — all decisions for a batch of chunks
 * plus folder creation plan.
 */
export interface PlacementPlan {
  /** Per-chunk placement decisions. */
  decisions: PlacementDecision[];
  /** Folders to create (relative to outputRoot). */
  foldersToCreate: string[];
  /** Brief rationale from the LLM. */
  rationale: string;
}

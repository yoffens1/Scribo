// src/core/refinery/types/refinery-result.ts
import type { ChunkWithHash, DeduplicationResult } from "./chunk-decision";
import type { ProposedTaxonomy } from "../taxonomy/types/taxonomy";
import type { PlacementPlan } from "../placement/types/placement";

/** Result of a single file refinement. */
export interface RefineryResult {
  /** Source file path (in inbox). */
  sourcePath: string;
  /** All chunks extracted from the source file. */
  chunks: ChunkWithHash[];
  /** Full deduplication result: decisions (merge/keep/reject) + remaining chunks. */
  dedup: DeduplicationResult;
  /** Proposed taxonomy tree (new folders to create). */
  taxonomy: ProposedTaxonomy;
  /** Placement decisions mapping chunks to output paths. */
  placement: PlacementPlan;
  /** Filesystem operations that were (or would be) performed. */
  operations: WriteOperation[];
  /** Whether this was a dry run. */
  dryRun: boolean;
}

/** A single filesystem operation in the write stage. */
export type WriteOperation =
  | { type: "create_file"; path: string; content: string }
  | { type: "merge_chunk"; sourceFile: string; targetFile: string; chunkText: string }
  | { type: "create_folder"; path: string }
  | { type: "move_file"; from: string; to: string }
  | { type: "delete_file"; path: string };

/** Result of a batch refinement. */
export interface BatchRefineryResult {
  results: RefineryResult[];
  errors: Array<{ sourcePath: string; error: string }>;
  totalChunks: number;
  mergedChunks: number;
  createdFiles: number;
  createdFolders: number;
}

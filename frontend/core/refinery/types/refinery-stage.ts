// src/core/refinery/types/refinery-stage.ts
import type { RefineryContext } from "./refinery-context";

/** Known log scopes for structured logging — single source of truth. */
export enum LogScope {
  CHUNKING_READ = "chunking.read",
  CHUNKING_DONE = "chunking.done",
  DEDUPE_START = "dedupe.start",
  DEDUPE_DONE = "dedupe.done",
  DEDUPE_MERGE = "dedupe.merge",
  TAXONOMY_GENERATE = "taxonomy.generate",
  TAXONOMY_BATCH = "taxonomy.batch",
  PLACEMENT_BATCH = "placement.batch",
  PLACEMENT_PLAN = "placement.plan",
  WRITE_DRYRUN = "write.dryrun",
  WRITE_START = "write.start",
  WRITE_DONE = "write.done",
  WRITER_CREATE_FILE = "writer.create_file",
  WRITER_MERGE_CHUNK = "writer.merge_chunk",
  WRITER_CREATE_FOLDER = "writer.create_folder",
  WRITER_MOVE_FILE = "writer.move_file",
  WRITER_BATCH = "writer.batch",
  WRITER_ROLLBACK = "writer.rollback",
  PIPELINE = "pipeline",
  TRACE_START = "trace.start",
  TRACE_END = "trace.end",
}

/**
 * Generic stage interface for the refinery pipeline.
 * Each stage receives typed input, produces typed output,
 * with access to shared RefineryContext.
 */
export interface RefineryStage<TIn, TOut> {
  readonly name: string;
  run(input: TIn, ctx: RefineryContext): Promise<TOut>;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type AnyRefineryStage = RefineryStage<any, any>;

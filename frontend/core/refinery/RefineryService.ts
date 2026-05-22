// src/core/refinery/RefineryService.ts
//
// PUBLIC FACADE — the only import point for external consumers.
// Provides: plan(file), refine(file), refineBatch(files).
// Internal orchestration is delegated to RefineryPipeline.
//
// Thread safety: dryRun is passed as a parameter to pipeline.refine(),
// not mutated on shared ctx — safe for concurrent calls.

import type { IFileAccess } from "@utils/_types";
import type { RetrievalService } from "@retrieval/RetrievalService";
import type { LLMService } from "@ai/llm/LLMService";
import type { Logger } from "@logging/Logger";
import type {
  RefineryResult,
  BatchRefineryResult,
} from "./types/refinery-result";
import type { RefineryContext } from "./types/refinery-context";
import { RefineryPipeline } from "./RefineryPipeline";
import { REFINERY_CONSTANTS } from "./constants";
import * as path from "path";

import type { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";

export interface RefineryServiceOptions {
  fileAccess: IFileAccess;
  retrieval: RetrievalService;
  llm: LLMService;
  logger: Logger;
  dbConnection?: TauriDbConnection;
  outputRoot?: string;
  inboxRoot?: string;
  dryRun?: boolean;
  refineryModel?: string;
  deleteFromDbOnGc?: boolean;
  overwriteOnMerge?: boolean;
  mergeTags?: boolean;
  indexSourceFile?: boolean;
}

export class RefineryService {
  private ctx: RefineryContext;
  private pipeline: RefineryPipeline;
  private defaultDryRun: boolean;

  constructor(opts: RefineryServiceOptions) {
    this.defaultDryRun = opts.dryRun ?? REFINERY_CONSTANTS.DEFAULT_DRY_RUN;
    
    // Create a mock transaction wrapper if no DB connection is supplied (e.g. in legacy tests)
    const dbConnection = opts.dbConnection ?? ({
      withTransaction: async <T>(fn: () => Promise<T>): Promise<T> => {
        return await fn();
      }
    } as any);

    this.ctx = {
      fileAccess: opts.fileAccess,
      retrieval: opts.retrieval,
      llm: opts.llm,
      logger: opts.logger,
      dbConnection,
      outputRoot: opts.outputRoot ?? REFINERY_CONSTANTS.DEFAULT_OUTPUT_ROOT,
      inboxRoot: opts.inboxRoot ?? REFINERY_CONSTANTS.DEFAULT_INBOX_ROOT,
      dryRun: this.defaultDryRun,
      refineryModel: opts.refineryModel,
      deleteFromDbOnGc: opts.deleteFromDbOnGc ?? true,
      overwriteOnMerge: opts.overwriteOnMerge ?? true,
      mergeTags: opts.mergeTags ?? true,
      indexSourceFile: opts.indexSourceFile ?? true,
    };
    this.pipeline = new RefineryPipeline(this.ctx);
  }

  async plan(sourcePath: string) {
    return this.pipeline.plan(sourcePath);
  }

  async refine(
    sourcePath: string,
    opts?: { dryRun?: boolean },
  ): Promise<RefineryResult> {
    const dryRun = opts?.dryRun ?? this.defaultDryRun;
    return this.pipeline.refine(sourcePath, { dryRun });
  }

  async refineBatch(
    sourcePaths: string[],
    opts?: { dryRun?: boolean },
  ): Promise<BatchRefineryResult> {
    const results: RefineryResult[] = [];
    const errors: Array<{ sourcePath: string; error: string }> = [];

    let totalChunks = 0;
    let mergedChunks = 0;
    let createdFiles = 0;
    let createdFolders = 0;

    for (const sourcePath of sourcePaths) {
      try {
        const result = await this.refine(sourcePath, opts);
        results.push(result);
        totalChunks += result.chunks.length;
        mergedChunks += result.operations.filter((o) => o.type === "merge_chunk").length;
        createdFiles += result.operations.filter((o) => o.type === "create_file").length;
        createdFolders += result.operations.filter((o) => o.type === "create_folder").length;
      } catch (err) {
        errors.push({
          sourcePath,
          error: err instanceof Error ? err.stack ?? err.message : String(err),
        });
      }
    }

    return { results, errors, totalChunks, mergedChunks, createdFiles, createdFolders };
  }
}

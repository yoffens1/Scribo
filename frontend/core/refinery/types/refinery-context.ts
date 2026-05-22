// src/core/refinery/types/refinery-context.ts
import type { IFileAccess } from "@utils/_types";
import type { RetrievalService } from "@retrieval/RetrievalService";
import type { LLMService } from "@ai/llm/LLMService";
import type { Logger } from "@logging/Logger";
import type { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";

/**
 * Shared context passed through all refinery stages.
 * Provides access to infrastructure services without DI framework.
 */
export interface RefineryContext {
  /** File system access (read/write/exists). */
  fileAccess: IFileAccess;
  /** Retrieval service for similarity search in output vault. */
  retrieval: RetrievalService;
  /** LLM service for taxonomy generation, placement, and chunk merging. */
  llm: LLMService;
  /** Structured logger for tracing refinery decisions. */
  logger: Logger;
  /** Root path of the output vault (where refined notes are written). */
  outputRoot: string;
  /** Root path of the inbox (source of raw markdown files). */
  inboxRoot: string;
  /** If true, stages should skip writes and only produce plans. */
  dryRun: boolean;
  /** Database connection for transactional operations. */
  dbConnection: TauriDbConnection;
  /** LLM model used for refinery tasks. */
  refineryModel?: string;
  /** Whether to delete records from the DB entirely on garbage collection. */
  deleteFromDbOnGc?: boolean;
  /** Whether to overwrite existing files on disk during a merge. */
  overwriteOnMerge?: boolean;
  /** Whether to merge old and new frontmatter tags on card merge. */
  mergeTags?: boolean;
  /** Whether to index the source file metadata record in the files table. */
  indexSourceFile?: boolean;
}

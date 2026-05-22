// src/core/retrieval/logging/types/retrieval-trace.ts
import type { LogEvent } from "@logging/types/log-event";

/**
 * Retrieval-specific trace metadata.
 * Attached to Trace.meta when a retrieval trace completes.
 */
export interface RetrievalTraceMeta {
  query: string;
  topK?: number;
  count: number;
  top: Array<{ filePath: string; chunkIndex: number; score: number }>;
}

/**
 * Backward-compatible trace shape.
 * @deprecated Use Trace<RetrievalTraceMeta> from core/logging for new code.
 */
export interface RetrievalTrace {
  traceId: string;
  query: string;
  startedAt: number;
  finishedAt?: number;
  totalDurationMs?: number;
  events: LogEvent[];
  result?: RetrievalTraceMeta;
}

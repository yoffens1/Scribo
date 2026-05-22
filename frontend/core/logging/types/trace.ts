// src/core/logging/types/trace.ts
import type { LogEvent } from "./log-event";

/**
 * Generic trace with typed metadata.
 *
 * Retrieval: Trace<{ query: string; topK: number; count: number; top: ... }>
 * Refinery:  Trace<{ filePath: string; chunkCount: number; mergedCount: number }>
 * Database:  Trace<{ sql: string; params: unknown[]; rowCount: number }>
 */
export interface Trace<TMeta = Record<string, unknown>> {
  id: string;
  /** Module namespace: "retrieval", "refinery", "database.sql", etc. */
  namespace: string;
  /** Human-readable label for this trace. */
  label: string;
  startedAt: number;
  endedAt?: number;
  durationMs?: number;
  events: LogEvent[];
  /** Module-specific metadata attached on end(). */
  meta?: TMeta;
}

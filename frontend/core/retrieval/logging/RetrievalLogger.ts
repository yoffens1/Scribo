// src/core/retrieval/logging/RetrievalLogger.ts
import { Logger } from "@logging/Logger";
import type { LogLevel, Sink } from "@logging/types";
import type { RetrievalTrace, RetrievalTraceMeta } from "./types";

/**
 * Retrieval-specific logger — thin wrapper around the shared Logger.
 *
 * @deprecated New code should use Logger from core/logging directly.
 *   const logger = loggerFactory.create("retrieval");
 *   const t = logger.startTrace<RetrievalTraceMeta>("search", { query: "ml" });
 * Kept for backward compatibility with existing importers.
 */
export class RetrievalLogger {
  private inner: Logger;
  private lastQuery = "";

  constructor(opts: {
    enabled: boolean;
    minLevel?: LogLevel;
    sinks?: Sink[];
  }) {
    this.inner = new Logger("retrieval", {
      enabled: opts.enabled,
      minLevel: opts.minLevel ?? "debug",
      sinks: opts.sinks,
    });
  }

  startTrace(query: string): string {
    this.lastQuery = query;
    return this.inner.startTrace<RetrievalTraceMeta>(query, {
      query,
      count: 0,
      top: [],
    });
  }

  log(
    level: LogLevel,
    stage: string,
    message: string,
    data?: Record<string, unknown>,
    durationMs?: number,
  ): void {
    this.inner.log(level, stage, message, data, durationMs);
  }

  async time<T>(
    stage: string,
    fn: () => Promise<T>,
    data?: Record<string, unknown>,
  ): Promise<T> {
    return this.inner.time(stage, fn, data);
  }

  async endTrace(resultDetails?: {
    count: number;
    top: Array<{ filePath: string; chunkIndex: number; score: number }>;
  }): Promise<RetrievalTrace | null> {
    const meta: RetrievalTraceMeta = {
      query: this.lastQuery,
      count: resultDetails?.count ?? 0,
      top: resultDetails?.top ?? [],
    };
    const trace = await this.inner.endTrace<RetrievalTraceMeta>(meta);
    if (!trace) return null;

    return {
      traceId: trace.id,
      query: meta.query,
      startedAt: trace.startedAt,
      finishedAt: trace.endedAt,
      totalDurationMs: trace.durationMs,
      events: trace.events,
      result: meta,
    };
  }

  exportTraceAsJson(trace: RetrievalTrace): string {
    return JSON.stringify(trace, null, 2);
  }

  get isEnabled(): boolean {
    return this.inner.isEnabled;
  }
}

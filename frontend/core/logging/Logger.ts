// src/core/logging/Logger.ts
import type { LogEvent, LogLevel, Trace } from "./types";
import type { Sink } from "./types";
import { ConsoleSink } from "./sinks/ConsoleSink";

const LEVEL_ORDER: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };

/**
 * Structured tracing logger — reusable across retrieval, refinery, database, etc.
 *
 * Each logger is scoped to a namespace (e.g. "retrieval", "refinery", "database.sql").
 * Traces carry typed module-specific metadata via Trace<TMeta>.
 *
 * Usage:
 *   const logger = new Logger("retrieval", { sinks: [new ConsoleSink()] });
 *   const t = logger.startTrace<{ query: string }>("search", { query: "ml" });
 *   logger.log("info", "hybrid.rrf", "fused", { lists: 2 });
 *   const trace = logger.endTrace({ count: 5 });
 */
export class Logger {
  private currentTrace: Trace | null = null;
  private sinks: Sink[];
  private enabled: boolean;
  private minLevel: number;

  constructor(
    /** Module namespace: "retrieval", "refinery", "database.sql", etc. */
    public readonly namespace: string,
    opts: {
      enabled?: boolean;
      minLevel?: LogLevel;
      sinks?: Sink[];
    } = {},
  ) {
    this.enabled = opts.enabled ?? true;
    this.minLevel = LEVEL_ORDER[opts.minLevel ?? "debug"];
    this.sinks = opts.enabled === false ? [] : (opts.sinks ?? [new ConsoleSink()]);
  }

  /**
   * Create a child logger with a sub-namespace.
   * e.g. logger.child("index").namespace === "retrieval.index"
   */
  child(subNamespace: string): Logger {
    return new Logger(`${this.namespace}.${subNamespace}`, {
      enabled: this.enabled,
      minLevel: Object.keys(LEVEL_ORDER).find(k => LEVEL_ORDER[k as LogLevel] === this.minLevel) as LogLevel,
      sinks: this.sinks,
    });
  }

  /** Start a new trace with typed metadata. Returns trace id. */
  startTrace<TMeta = Record<string, unknown>>(
    label: string,
    meta?: TMeta,
  ): string {
    if (!this.enabled) return "";
    const id = crypto.randomUUID();
    this.currentTrace = {
      id,
      namespace: this.namespace,
      label,
      startedAt: performance.now(),
      events: [],
      meta,
    } as Trace;
    this.log("info", "trace.start", label);
    return id;
  }

  log(
    level: LogLevel,
    stage: string,
    message: string,
    data?: Record<string, unknown>,
    durationMs?: number,
  ): void {
    if (!this.enabled || LEVEL_ORDER[level] < this.minLevel) return;
    const event: LogEvent = {
      traceId: this.currentTrace?.id ?? "",
      timestamp: performance.now(),
      level, stage, message, data, durationMs,
    };
    if (this.currentTrace) this.currentTrace.events.push(event);
    for (const sink of this.sinks) sink.write(event);
  }

  /** Convenience timing wrapper. Logs duration on success, error message on failure. */
  async time<T>(
    stage: string,
    fn: () => Promise<T>,
    data?: Record<string, unknown>,
  ): Promise<T> {
    const t0 = performance.now();
    try {
      const result = await fn();
      this.log("debug", stage, "ok", data, performance.now() - t0);
      return result;
    } catch (e) {
      this.log("error", stage, (e as Error).message, { ...data, error: String(e) }, performance.now() - t0);
      throw e;
    }
  }

  /**
   * End the current trace with module-specific metadata.
   * Returns the completed trace (or null if no trace was active).
   */
  async endTrace<TMeta = Record<string, unknown>>(meta?: TMeta): Promise<Trace<TMeta> | null> {
    if (!this.currentTrace) return null;
    const trace = this.currentTrace as Trace<TMeta>;
    trace.endedAt = performance.now();
    trace.durationMs = trace.endedAt - trace.startedAt;
    if (meta !== undefined) {
      trace.meta = { ...trace.meta, ...meta } as TMeta;
    }
    this.log("info", "trace.end", "done", { durationMs: trace.durationMs?.toFixed(0) });
    for (const sink of this.sinks) {
      await sink.flushTrace?.(trace as Trace);
    }
    this.currentTrace = null;
    return trace;
  }

  /** Dump trace as pretty JSON string. */
  exportTraceAsJson<TMeta = Record<string, unknown>>(trace: Trace<TMeta>): string {
    return JSON.stringify(trace, null, 2);
  }

  // ── Convenience methods (backward compat with old database/embedder loggers) ──

  debug(...args: unknown[]): void {
    this.log("debug", "general", args.map(String).join(" "));
  }
  info(...args: unknown[]): void {
    this.log("info", "general", args.map(String).join(" "));
  }
  warn(...args: unknown[]): void {
    this.log("warn", "general", args.map(String).join(" "));
  }
  error(...args: unknown[]): void {
    this.log("error", "general", args.map(String).join(" "));
  }

  get isEnabled(): boolean {
    return this.enabled;
  }
}

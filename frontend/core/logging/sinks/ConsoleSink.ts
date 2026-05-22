// src/core/logging/sinks/ConsoleSink.ts
import type { Sink } from "../types/sink";
import type { LogEvent } from "../types";

/** Pretty-print to console with colors by level. */
export class ConsoleSink implements Sink {
  write(e: LogEvent): void {
    const prefix = e.traceId ? `[${e.traceId.slice(0, 8)}] ` : "";
    const stage = `[${e.stage}]`;
    const dur = e.durationMs !== undefined ? ` (${e.durationMs.toFixed(0)}ms)` : "";
    const msg = `${prefix}${stage} ${e.message}${dur}`;

    const fn =
      e.level === "error" ? console.error :
      e.level === "warn"  ? console.warn  :
      e.level === "info"  ? console.info  :
      console.debug;

    if (e.data && Object.keys(e.data).length > 0) {
      fn(msg, e.data);
    } else {
      fn(msg);
    }
  }
}

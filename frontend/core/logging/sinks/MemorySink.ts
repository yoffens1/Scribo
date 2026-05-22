// src/core/logging/sinks/MemorySink.ts
import type { Sink } from "../types/sink";
import type { LogEvent, Trace } from "../types";

/** Collects full traces in memory for later export. Cap at 50 traces. */
export class MemorySink implements Sink {
  private traces: Trace[] = [];
  private maxTraces = 50;

  write(_e: LogEvent): void { /* no-op: we collect whole traces */ }

  flushTrace(t: Trace): void {
    this.traces.push(t);
    if (this.traces.length > this.maxTraces) {
      this.traces.shift();
    }
  }

  getAll(): Trace[] { return [...this.traces]; }
  clear(): void { this.traces = []; }
}

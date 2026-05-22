// src/core/logging/sinks/NoopSink.ts
import type { Sink } from "../types/sink";
import type { LogEvent } from "../types";

/** Silent sink — used when logging is disabled. */
export class NoopSink implements Sink {
  write(_e: LogEvent): void {}
}

// src/core/logging/types/sink.ts
import type { LogEvent } from "./log-event";
import type { Trace } from "./trace";

export interface Sink {
  write(event: LogEvent): void;
  flushTrace?(trace: Trace): void | Promise<void>;
}

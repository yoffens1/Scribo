// src/core/logging/types/log-event.ts
import type { LogLevel } from "./log-level";

export interface LogEvent {
  traceId: string;
  timestamp: number;
  level: LogLevel;
  stage: string;
  message: string;
  data?: Record<string, unknown>;
  durationMs?: number;
}

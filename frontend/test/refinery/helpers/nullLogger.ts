// src/test/refinery/helpers/nullLogger.ts
import { Logger } from "@logging/Logger";
import type { LogLevel, LogEvent } from "@logging/types";

/**
 * Silent logger for tests — all methods are no-ops.
 */
export const nullLogger = (): Logger => {
  const logger = new Logger("test", { enabled: false });
  return logger;
};

/** Spy logger that records all log calls. */
export const spyLogger = (): Logger & { calls: Array<{ level: LogLevel; stage: string; message: string }> } => {
  const calls: Array<{ level: LogLevel; stage: string; message: string }> = [];
  const logger = new Logger("test", { enabled: false });

  const origLog = logger.log.bind(logger);
  (logger as any).log = (level: LogLevel, stage: string, message: string, data?: Record<string, unknown>, durationMs?: number) => {
    calls.push({ level, stage, message });
    origLog(level, stage, message, data, durationMs);
  };
  (logger as any).calls = calls;
  return logger as any;
};

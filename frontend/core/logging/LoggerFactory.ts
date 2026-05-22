// src/core/logging/LoggerFactory.ts
import { Logger } from "./Logger";
import type { LogLevel, Sink } from "./types";
import { ConsoleSink } from "./sinks/ConsoleSink";

export interface LoggerFactoryConfig {
  enabled?: boolean;
  minLevel?: LogLevel;
  sinks?: Sink[];
}

/**
 * Central factory for creating namespaced loggers.
 *
 * Usage:
 *   const factory = new LoggerFactory({ minLevel: "debug", sinks: [new ConsoleSink()] });
 *   const retrievalLog = factory.create("retrieval");
 *   const refineryLog = factory.create("refinery");
 *   const sqlLog = factory.create("database.sql");
 */
export class LoggerFactory {
  private config: LoggerFactoryConfig;

  constructor(config: LoggerFactoryConfig = {}) {
    this.config = config;
  }

  /** Create a logger for the given namespace. */
  create(namespace: string): Logger {
    return new Logger(namespace, {
      enabled: this.config.enabled ?? true,
      minLevel: this.config.minLevel ?? "debug",
      sinks: this.config.sinks ?? [new ConsoleSink()],
    });
  }

  /** Update config for all future loggers. */
  reconfigure(config: Partial<LoggerFactoryConfig>): void {
    this.config = { ...this.config, ...config };
  }
}

/** Singleton factory instance — import and use across modules. */
export const loggerFactory = new LoggerFactory();

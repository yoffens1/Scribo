// src/core/database/infrastructure/Logger.ts
import { Logger, loggerFactory } from "@logging/index";

/**
 * Database-layer logger.
 * Delegates to shared core/logging/Logger with namespace "database".
 *
 * @deprecated Use `loggerFactory.create("database.sql")` directly for new code.
 * Kept as singleton for backward compatibility.
 */
export const logger: Logger = loggerFactory.create("database");

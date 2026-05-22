// src/core/ai/transport/retry.ts

import { AI_DEFAULTS } from "@settings";
import { HttpError, ValidationError, ParseError } from "./HttpError";
import { eventBus } from "../events/EventBus";

/**
 * Determines whether a given error is transient and worth retrying.
 * Retries: 429, 5xx, network errors.
 * Does NOT retry: 400, 401, 403, validation errors, aborts.
 */
export function shouldRetry(error: unknown): boolean {
  // Never retry user-initiated aborts
  if (
    (error instanceof DOMException || error instanceof Error) &&
    error.name === "AbortError"
  ) {
    return false;
  }
  // Validation / parse errors are non-transient
  if (error instanceof ValidationError || error instanceof ParseError) {
    return false;
  }
  if (error instanceof HttpError) {
    if (error.status === 429) return true; // rate limit
    if (error.status >= 500) return true; // server error
    if (error.status === 0) return true; // network
    return false;
  }
  // Unknown errors — retry to be safe
  return true;
}

const INITIAL_DELAY_MS = 250;
const MAX_DELAY_MS = 2000;

/**
 * Exponential backoff retry wrapper.
 * Uses gentle delays suitable for UI interactions.
 */
export async function withRetry<T>(
  fn: () => Promise<T>,
  label: string,
  maxRetries = AI_DEFAULTS.maxRetries,
): Promise<T> {
  let lastErr: unknown;
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await fn();
    } catch (err: unknown) {
      lastErr = err;
      if (attempt < maxRetries && shouldRetry(err)) {
        const delay = Math.min(INITIAL_DELAY_MS * 2 ** (attempt - 1), MAX_DELAY_MS);
        const errorStr = err instanceof HttpError
          ? `${err.provider} HTTP ${err.status}`
          : err instanceof Error
            ? err.message
            : String(err);
        await eventBus.emit("ai:retry", {
          provider: err instanceof HttpError ? err.provider : "unknown",
          label,
          attempt,
          maxRetries,
          delayMs: delay,
          error: errorStr,
          timestamp: Date.now(),
        });
        console.warn(`${label}: attempt ${attempt} failed, retrying in ${delay}ms`);
        await new Promise((r) => setTimeout(r, delay));
      } else {
        throw err;
      }
    }
  }
  throw lastErr;
}

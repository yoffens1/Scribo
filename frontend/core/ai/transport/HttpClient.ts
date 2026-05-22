// src/core/ai/transport/HttpClient.ts

import { AI_DEFAULTS } from "@settings";
import { HttpError } from "./HttpError";
import { report } from "./telemetry";
import { eventBus } from "../events/EventBus";
import { withRetry } from "./retry";
import { RateLimiter } from "./rateLimit";

import type { HttpRequest } from "../types/http";

// ─── Low-level fetch wrapper ──────────────────────────────────

/**
 * Shared HTTP transport with timeout, status check, JSON parsing, and telemetry.
 * Used by all AI providers (LLM + embedder).
 */
export async function fetchJson(
  req: HttpRequest,
  provider: string,
  timeoutMs = AI_DEFAULTS.timeout,
  signal?: AbortSignal,
): Promise<unknown> {
  const start = Date.now();
  let status = 0;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  // Link external signal to internal controller
  if (signal) {
    if (signal.aborted) {
      controller.abort(signal.reason);
      clearTimeout(timer);
    } else {
      signal.addEventListener("abort", () => controller.abort(signal.reason), { once: true });
    }
  }

  // Fire-and-forget — diagnostics don't need to block the request
  eventBus.emit("ai:request:start", {
    provider,
    path: req.url,
    timestamp: start,
  });

  try {
    let resp: Response;
    try {
      resp = await fetch(req.url, {
        method: req.method ?? "POST",
        headers: { "Content-Type": "application/json", ...req.headers },
        body: req.body,
        signal: controller.signal,
      });
    } catch (err) {
      throw new HttpError(`Failed to connect to ${provider}`, 0, provider, String(err));
    }

    status = resp.status;

    if (!resp.ok) {
      let body = "";
      try { body = await resp.text(); } catch {}
      let hint = "";
      if (resp.status === 401) hint = " (bad API key)";
      if (resp.status === 429) hint = " (rate limited)";
      throw new HttpError(
        `${provider} returned ${resp.status}${hint}`,
        resp.status, provider, body || undefined,
      );
    }

    const json = await resp.json();
    await eventBus.emit("ai:request:done", {
      provider,
      path: req.url,
      latencyMs: Date.now() - start,
      status,
      timestamp: Date.now(),
    });
    return json;
  } catch (err) {
    const latencyMs = Date.now() - start;
    await eventBus.emit("ai:request:error", {
      provider,
      path: req.url,
      status: err instanceof HttpError ? err.status : 0,
      error: String(err),
      latencyMs,
      timestamp: Date.now(),
    });
    throw err;
  } finally {
    clearTimeout(timer);
    report({ provider, path: req.url, latencyMs: Date.now() - start, status });
  }
}

// ─── High-level HTTP client ───────────────────────────────────

export interface HttpClientOptions {
  apiKey?: string;
  label?: string;
  maxRPS?: number;
  streamTimeoutMs?: number;
  headers?: Record<string, string>;
}

/**
 * Unified HTTP transport.
 * Encapsulates retry, timeout, rate limiting, auth headers, and AbortSignal.
 */
export class HttpClient {
  private rateLimiter: RateLimiter | null = null;
  protected readonly streamTimeoutMs: number;

  constructor(
    protected baseUrl: string,
    protected options: HttpClientOptions = {},
  ) {
    if (options.maxRPS) this.rateLimiter = new RateLimiter(options.maxRPS);
    this.streamTimeoutMs = options.streamTimeoutMs ?? AI_DEFAULTS.timeout;
  }

  /** Override for custom auth (e.g. `x-api-key`). */
  protected buildHeaders(): Record<string, string> {
    const headers: Record<string, string> = { ...this.options.headers };
    if (this.options.apiKey) {
      headers.Authorization = `Bearer ${this.options.apiKey}`;
    }
    return headers;
  }

  /** JSON POST with retry, timeout, rate limiting. */
  async post(path: string, body: unknown, signal?: AbortSignal): Promise<unknown> {
    return withRetry(
      async () => {
        if (this.rateLimiter) await this.rateLimiter.acquire();
        return fetchJson(
          {
            url: `${this.baseUrl}${path}`,
            headers: this.buildHeaders(),
            body: JSON.stringify(body),
          },
          this.options.label ?? "http",
          undefined,
          signal,
        );
      },
      `[${this.options.label}] ${path}`,
    );
  }

  /**
   * SSE/NDJSON stream POST with telemetry, optional retry, and timeout.
   * Respects rate limiter, checks resp.ok.
   */
  async stream(path: string, body: unknown, signal?: AbortSignal): Promise<Response> {
    const url = `${this.baseUrl}${path}`;
    const label = this.options.label ?? "http";

    const doFetch = async (): Promise<Response> => {
      const start = Date.now();

      // Emit per-attempt (including retries)
      eventBus.emit("ai:request:start", { provider: label, path: url, timestamp: start });

      if (this.rateLimiter) await this.rateLimiter.acquire();

      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), this.streamTimeoutMs);

      if (signal) {
        if (signal.aborted) {
          controller.abort(signal.reason);
          clearTimeout(timer);
        } else {
          signal.addEventListener("abort", () => controller.abort(signal.reason), { once: true });
        }
      }

      try {
        const resp = await fetch(url, {
          method: "POST",
          headers: { ...this.buildHeaders(), "Content-Type": "application/json" },
          body: JSON.stringify(body),
          signal: controller.signal,
        });

        if (!resp.ok) {
          const text = await resp.text().catch(() => "");
          throw new HttpError(
            `[${label}] HTTP ${resp.status}: ${text.slice(0, 300)}`,
            resp.status,
            label,
            text || undefined,
          );
        }

        await eventBus.emit("ai:request:done", {
          provider: label,
          path: url,
          latencyMs: Date.now() - start,
          status: resp.status,
          timestamp: Date.now(),
        });

        return resp;
      } catch (err) {
        const latencyMs = Date.now() - start;
        await eventBus.emit("ai:request:error", {
          provider: label,
          path: url,
          status: err instanceof HttpError ? err.status : 0,
          error: String(err),
          latencyMs,
          timestamp: Date.now(),
        });
        throw err;
      } finally {
        clearTimeout(timer);
      }
    };

    return withRetry(doFetch, `[${label}] stream`, 2);
  }
}

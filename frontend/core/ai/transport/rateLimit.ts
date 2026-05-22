// src/core/ai/transport/rateLimit.ts

/**
 * Token-bucket rate limiter with serialized access.
 * Avoids races when multiple concurrent callers invoke `acquire()`.
 */
export class RateLimiter {
  private tokens: number;
  private lastRefill: number;
  private chain: Promise<void> = Promise.resolve();

  constructor(private maxPerSecond: number) {
    this.tokens = maxPerSecond;
    this.lastRefill = Date.now();
  }

  async acquire(): Promise<void> {
    const next = this.chain.then(() => this.doAcquire());
    this.chain = next.catch(() => {});
    return next;
  }

  private async doAcquire(): Promise<void> {
    const now = Date.now();
    const elapsed = (now - this.lastRefill) / 1000;
    this.tokens = Math.min(this.maxPerSecond, this.tokens + elapsed * this.maxPerSecond);
    this.lastRefill = now;

    if (this.tokens < 1) {
      const waitMs = ((1 - this.tokens) / this.maxPerSecond) * 1000;
      await new Promise((r) => setTimeout(r, waitMs));
      // Re-check refill after wait
      const now2 = Date.now();
      const elapsed2 = (now2 - this.lastRefill) / 1000;
      this.tokens = Math.min(this.maxPerSecond, this.tokens + elapsed2 * this.maxPerSecond);
      this.lastRefill = now2;
    }

    this.tokens -= 1;
  }
}

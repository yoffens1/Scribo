import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { RateLimiter } from "@ai/transport/rateLimit";

describe("RateLimiter", () => {
  it("single acquire passes immediately", async () => {
    const l = new RateLimiter(5);
    const start = Date.now();
    await l.acquire();
    assert.ok(Date.now() - start < 50);
  });

  it("burst within limit passes quickly", async () => {
    const l = new RateLimiter(1000);
    const start = Date.now();
    for (let i = 0; i < 10; i++) await l.acquire();
    assert.ok(Date.now() - start < 200);
  });

  it("serializes concurrent access", async () => {
    const l = new RateLimiter(1000);
    const order: number[] = [];
    await Promise.all([1, 2, 3, 4, 5].map(i =>
      l.acquire().then(() => order.push(i)),
    ));
    assert.equal(order.length, 5);
    assert.equal(new Set(order).size, 5);
  });

  it("no throws on concurrent access", async () => {
    const l = new RateLimiter(5);
    const results = await Promise.allSettled(
      Array.from({ length: 10 }, () => l.acquire()),
    );
    assert.equal(results.filter(r => r.status === "rejected").length, 0);
  });
});

// src/test/database/services/HashService.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { HashService } from "@database/services/indexing/HashService";

describe("HashService", () => {
  const hash = new HashService();

  it("produces stable hashes (same input → same output)", async () => {
    const a = await hash.compute("hello world");
    const b = await hash.compute("hello world");
    assert.equal(a, b);
  });

  it("different content → different hash", async () => {
    const a = await hash.compute("hello");
    const b = await hash.compute("world");
    assert.notEqual(a, b);
  });

  it("handles unicode content", async () => {
    const h = await hash.compute("привет мир 🎉");
    assert.ok(h.length === 64, "SHA-256 hex should be 64 chars");
    assert.match(h, /^[0-9a-f]{64}$/);
  });

  it("handles empty content", async () => {
    const h = await hash.compute("");
    assert.ok(h.length === 64);
    assert.match(h, /^[0-9a-f]{64}$/);
  });

  it("handles large content", async () => {
    const content = "x".repeat(1_000_000);
    const h = await hash.compute(content);
    assert.equal(h.length, 64);
    assert.match(h, /^[0-9a-f]{64}$/);
  });

  it("hash is deterministic across calls", async () => {
    const content = "deterministic test";
    const results = new Set<string>();
    for (let i = 0; i < 10; i++) {
      results.add(await hash.compute(content));
    }
    assert.equal(results.size, 1);
  });
});

// src/test/refinery/unit/Strategies.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ExactMatchStrategy } from "@refinery/dedupe/strategies/ExactMatchStrategy";
import { AppendStrategy } from "@refinery/dedupe/strategies/AppendStrategy";
import type { ChunkWithHash } from "@refinery/types/chunk-decision";

const c = (text: string): ChunkWithHash => ({ hash: "h", embeddingText: text, generationText: text, text, index: 0, sourcePath: "" });

describe("ExactMatchStrategy", () => {
  const s = new ExactMatchStrategy();

  it("has correct name", async () => {
    assert.strictEqual(s.name, "exact-match");
  });

  it("canHandle matches identical text", async () => {
    assert.strictEqual(s.canHandle("hello world", c("hello world")), true);
  });

  it("canHandle ignores trailing whitespace", async () => {
    assert.strictEqual(s.canHandle("hello", c("hello\n")), true);
    assert.strictEqual(s.canHandle("hello\n", c("hello")), true);
  });

  it("canHandle returns false for different text", async () => {
    assert.strictEqual(s.canHandle("hello", c("world")), false);
  });

  it("canHandle returns false for extra content", async () => {
    assert.strictEqual(s.canHandle("hello", c("hello world")), false);
  });

  it("merge keeps longer version", async () => {
    assert.strictEqual(await s.merge("short", c("longer text here")), "longer text here");
    assert.strictEqual(await s.merge("longer text here", c("short")), "longer text here");
  });

  it("merge keeps existing when equal length", async () => {
    assert.strictEqual(await s.merge("abcde", c("vwxyz")), "abcde");
  });
});

describe("AppendStrategy", () => {
  const s = new AppendStrategy();

  it("has correct name", async () => {
    assert.strictEqual(s.name, "append");
  });

  it("canHandle always returns true", async () => {
    assert.strictEqual(s.canHandle("anything", c("whatever")), true);
  });

  it("merge appends with separator", async () => {
    const result = await s.merge("Existing text.", c("New text."));
    assert.strictEqual(result, "Existing text.\n\n---\n\nNew text.");
  });

  it("merge handles empty existing", async () => {
    const result = await s.merge("", c("content"));
    assert.strictEqual(result, "\n\n---\n\ncontent");
  });

  it("merge handles empty incoming", async () => {
    const result = await s.merge("content", c(""));
    assert.strictEqual(result, "content\n\n---\n\n");
  });
});

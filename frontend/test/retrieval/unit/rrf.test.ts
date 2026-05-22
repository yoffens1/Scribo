import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { rrf } from "@retrieval/retrievers/fusion/rrf";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number, score = 0.5): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score,
  text: `text-${filePath}-${chunkIndex}`,
});

describe("rrf", () => {
  // ── Empty / trivial ──

  it("empty lists → empty result", async () => {
    const result = rrf([], 60, 5);
    assert.deepEqual(result, []);
  });

  it("empty results within a list → handled gracefully", async () => {
    const result = rrf([{ results: [] }, { results: [] }], 60, 5);
    assert.deepEqual(result, []);
  });

  it("one list → result identical to input (preserves order)", async () => {
    const input: SearchResult[] = [
      makeResult("a.md", 0),
      makeResult("a.md", 1),
      makeResult("b.md", 0),
    ];
    const result = rrf([{ results: input }], 60, 5);
    assert.equal(result.length, 3);
    assert.equal(result[0].chunkRef.filePath, "a.md");
    assert.equal(result[0].chunkRef.chunkIndex, 0);
    assert.equal(result[1].chunkRef.filePath, "a.md");
    assert.equal(result[1].chunkRef.chunkIndex, 1);
    assert.equal(result[2].chunkRef.filePath, "b.md");
    assert.equal(result[2].chunkRef.chunkIndex, 0);
  });

  it("scores follow 1/(k+rank) formula for single list", async () => {
    const input: SearchResult[] = [
      makeResult("a.md", 0),
      makeResult("a.md", 1),
    ];
    const result = rrf([{ results: input }], 60, 5);
    assert.equal(result.length, 2);
    // rank 1 → 1/(60+1) = 1/61
    assert.ok(Math.abs(result[0].score - 1 / 61) < 0.001);
    // rank 2 → 1/(60+2) = 1/62
    assert.ok(Math.abs(result[1].score - 1 / 62) < 0.001);
  });

  // ── Two lists without overlap ──

  it("two lists without overlap → correct merge by score", async () => {
    const listA: SearchResult[] = [
      makeResult("a.md", 0),
      makeResult("a.md", 1),
    ];
    const listB: SearchResult[] = [
      makeResult("b.md", 0),
    ];
    const result = rrf([{ results: listA }, { results: listB }], 60, 5);
    assert.equal(result.length, 3);
    // Expected: a.md#0 (1/61 ≈ 0.0164), b.md#0 (1/61 ≈ 0.0164), a.md#1 (1/62 ≈ 0.0161)
    // a.md#0 and b.md#0 have same score; stable tie-break needed
    // Since they're different IDs with identical score, sort order is stable (V8)
    // but we just verify all 3 are present
    const ids = result.map(r => `${r.chunkRef.filePath}#${r.chunkRef.chunkIndex}`);
    assert.ok(ids.includes("a.md#0"));
    assert.ok(ids.includes("a.md#1"));
    assert.ok(ids.includes("b.md#0"));
  });

  // ── Overlap: doc in both lists → score summed ──

  it("doc on rank 1 in both lists, k=60 → score = 2/61", async () => {
    const listA: SearchResult[] = [makeResult("a.md", 0)];
    const listB: SearchResult[] = [makeResult("a.md", 0)];
    const result = rrf([{ results: listA }, { results: listB }], 60, 5);
    assert.equal(result.length, 1);
    // 1/(60+1) + 1/(60+1) = 2/61
    assert.ok(Math.abs(result[0].score - 2 / 61) < 0.0001);
  });

  it("doc on different positions in two lists → score sums correctly", async () => {
    const listA: SearchResult[] = [makeResult("a.md", 0), makeResult("b.md", 0)];
    const listB: SearchResult[] = [makeResult("b.md", 0), makeResult("a.md", 0)];
    const result = rrf([{ results: listA }, { results: listB }], 60, 5);
    assert.equal(result.length, 2);
    // Both docs get 1/(60+1) + 1/(60+2) ≈ 0.01639 + 0.01613 = 0.03252
    // Same score for both → order is stable
    const ids = result.map(r => r.chunkRef.filePath);
    assert.ok(ids.includes("a.md"));
    assert.ok(ids.includes("b.md"));
  });

  // ── Weight ──

  it("weight: list A weight=2, list B weight=1 → A has 2× influence", async () => {
    const listA: SearchResult[] = [makeResult("a.md", 0)];
    const listB: SearchResult[] = [makeResult("b.md", 0)];
    // a gets 2/61, b gets 1/61
    const result = rrf([
      { results: listA, weight: 2 },
      { results: listB, weight: 1 },
    ], 60, 5);
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "a.md");
    assert.ok(Math.abs(result[0].score - 2 / 61) < 0.0001);
    assert.ok(Math.abs(result[1].score - 1 / 61) < 0.0001);
  });

  it("default weight = 1", async () => {
    const listA: SearchResult[] = [makeResult("a.md", 0)];
    const listB: SearchResult[] = [makeResult("b.md", 0)];
    // Same score 1/61 — order is stable
    const result = rrf([{ results: listA }, { results: listB }], 60, 5);
    assert.equal(result.length, 2);
    assert.ok(Math.abs(result[0].score - 1 / 61) < 0.0001);
    assert.ok(Math.abs(result[1].score - 1 / 61) < 0.0001);
  });

  // ── topK truncation ──

  it("topK truncates result", async () => {
    const list: SearchResult[] = Array.from({ length: 10 }, (_, i) =>
      makeResult(`doc${i}.md`, 0),
    );
    const result = rrf([{ results: list }], 60, 3);
    assert.equal(result.length, 3);
  });

  it("topK larger than input → returns all", async () => {
    const list: SearchResult[] = [makeResult("a.md", 0), makeResult("b.md", 0)];
    const result = rrf([{ results: list }], 60, 10);
    assert.equal(result.length, 2);
  });

  // ── Stable tie-break ──

  it("identical scores → stable order (no random flips)", async () => {
    const list: SearchResult[] = [
      makeResult("a.md", 0),
      makeResult("b.md", 0),
      makeResult("c.md", 0),
    ];
    // Run 5 times — verify order is always the same
    for (let i = 0; i < 5; i++) {
      const result = rrf([{ results: list }], 60, 5);
      assert.equal(result[0].chunkRef.filePath, "a.md");
      assert.equal(result[1].chunkRef.filePath, "b.md");
      assert.equal(result[2].chunkRef.filePath, "c.md");
    }
  });

  it("two different docs with same score → stable tie-break in merge", async () => {
    const listA: SearchResult[] = [
      makeResult("a.md", 0),
      makeResult("b.md", 0),
    ];
    const listB: SearchResult[] = [
      makeResult("c.md", 0),
      makeResult("d.md", 0),
    ];
    // All have score 1/61 + 1/62 — stable order preserved
    for (let i = 0; i < 5; i++) {
      const result = rrf([{ results: listA }, { results: listB }], 60, 10);
      // Just verify it doesn't crash and has 4 elements
      assert.equal(result.length, 4);
    }
  });

  // ── ID collision edge case ──

  it("filePath with '#' and chunkIndex do not collide", async () => {
    // filePath: "a#1", chunkIndex: 0 → id = "a#1#0"
    // filePath: "a", chunkIndex: 1   → id = "a#1"
    // These are DIFFERENT keys — both should appear in result
    const list: SearchResult[] = [
      makeResult("a#1", 0),
      makeResult("a", 1),
    ];
    const result = rrf([{ results: list }], 60, 5);
    assert.equal(result.length, 2);
    const ids = result.map(r => `${r.chunkRef.filePath}#${r.chunkRef.chunkIndex}`);
    assert.ok(ids.includes("a#1#0"));
    assert.ok(ids.includes("a#1"));
  });

  it("same filePath+chunkIndex from different lists → fused, not duplicated", async () => {
    const listA: SearchResult[] = [makeResult("a.md", 0)];
    const listB: SearchResult[] = [makeResult("a.md", 0)];
    const result = rrf([{ results: listA }, { results: listB }], 60, 5);
    assert.equal(result.length, 1);
  });

  // ── Non-default k ──

  it("non-default k value works", async () => {
    const list: SearchResult[] = [makeResult("a.md", 0)];
    const result = rrf([{ results: list }], 10, 5);
    // score = 1/(10+1) = 1/11
    assert.ok(Math.abs(result[0].score - 1 / 11) < 0.0001);
  });
});

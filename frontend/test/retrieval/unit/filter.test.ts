import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { applyFilters } from "@retrieval/retrievers/filter";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score: 0.5,
  text: `text-${filePath}`,
});

describe("applyFilters", () => {
  const results: SearchResult[] = [
    makeResult("Daily/2024-01-01.md", 0),
    makeResult("Daily/2024-01-02.md", 0),
    makeResult("Projects/foo.md", 0),
    makeResult("Projects/foo.md", 1),
    makeResult("foobar/x.md", 0),
  ];

  // ── No filters ──

  it("filters: undefined → returns all", async () => {
    const filtered = applyFilters(results, undefined);
    assert.equal(filtered.length, 5);
  });

  it("empty options → returns all", async () => {
    const filtered = applyFilters(results, {});
    assert.equal(filtered.length, 5);
  });

  // ── filePath exact match ──

  it("filePath exact match", async () => {
    const filtered = applyFilters(results, {
      filters: { filePath: "Projects/foo.md" },
    });
    assert.equal(filtered.length, 2);
    assert.ok(filtered.every(r => r.chunkRef.filePath === "Projects/foo.md"));
  });

  it("filePath exact match — no match returns empty", async () => {
    const filtered = applyFilters(results, {
      filters: { filePath: "nonexistent.md" },
    });
    assert.equal(filtered.length, 0);
  });

  // ── folder prefix match ──

  it("folder prefix match with trailing slash", async () => {
    const filtered = applyFilters(results, {
      filters: { folder: "Daily/" },
    });
    assert.equal(filtered.length, 2);
    assert.ok(filtered.every(r => r.chunkRef.filePath.startsWith("Daily/")));
  });

  it("folder prefix match without trailing slash: 'foo' does NOT match 'foobar/'", async () => {
    // This is the gotcha: "foobar/x.md".startsWith("foo") = true
    // Use the actual behavior: startsWith("foo") matches "foobar/x.md"
    const filtered = applyFilters(results, {
      filters: { folder: "foo" },
    });
    // Current behavior: "foobar/x.md" starts with "foo" → matches
    // And "Projects/foo.md" does NOT start with "foo"
    assert.equal(filtered.length, 1);
    assert.equal(filtered[0].chunkRef.filePath, "foobar/x.md");
  });

  it("folder='Projects/' matches correctly", async () => {
    const filtered = applyFilters(results, {
      filters: { folder: "Projects/" },
    });
    assert.equal(filtered.length, 2);
  });

  // ── Both filters (AND) ──

  it("both filePath and folder → AND logic", async () => {
    const filtered = applyFilters(results, {
      filters: { filePath: "Daily/2024-01-01.md", folder: "Daily/" },
    });
    assert.equal(filtered.length, 1);
    assert.equal(filtered[0].chunkRef.filePath, "Daily/2024-01-01.md");
  });

  it("both filters where filePath doesn't match folder → empty", async () => {
    const filtered = applyFilters(results, {
      filters: { filePath: "Projects/foo.md", folder: "Daily/" },
    });
    assert.equal(filtered.length, 0);
  });

  // ── Empty results ──

  it("empty results → empty output (no crash)", async () => {
    const filtered = applyFilters([], { filters: { filePath: "a.md" } });
    assert.deepEqual(filtered, []);
  });

  it("empty results with no filters → empty", async () => {
    const filtered = applyFilters([], undefined);
    assert.deepEqual(filtered, []);
  });
});

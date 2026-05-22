// src/test/refinery/unit/SimilarityMatcher.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { SimilarityMatcher } from "@refinery/dedupe/SimilarityMatcher";
import { REFINERY_CONSTANTS } from "@refinery/constants";
import type { ChunkWithHash } from "@refinery/types/chunk-decision";
import type { SearchResult } from "@retrieval/types/search";

const mockRetrieval = (results: Partial<SearchResult>[] = []) => ({
  query: async (_text: string, _opts?: any): Promise<SearchResult[]> =>
    results.map(r => ({
      chunkRef: r.chunkRef ?? { filePath: "out/test.md", chunkIndex: 0 },
      score: r.score ?? 0.5,
      text: r.text ?? "",
    })),
}) as any;

const mkChunk = (text = "x".repeat(50)): ChunkWithHash => ({
  hash: "h-" + Math.random().toString(36).slice(2, 8),
  embeddingText: text,
  generationText: text,
  text,
  index: 0,
  sourcePath: "inbox/test.md",
});

describe("SimilarityMatcher.classify", () => {
  it("returns 'keep' when no results", async () => {
    const m = new SimilarityMatcher(mockRetrieval([]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "keep");
  });

  it("returns 'reject' at NEAR_DUP_THRESHOLD (0.98)", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.98 }]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "reject");
  });

  it("returns 'reject' above NEAR_DUP_THRESHOLD (0.99)", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.99 }]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "reject");
  });

  it("returns 'merge' between MERGE (0.85) and NEAR_DUP (0.98)", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.90 }]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "merge");
  });

  it("returns 'keep' below MERGE_SIMILARITY_THRESHOLD", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.80 }]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "keep");
  });

  it("returns 'merge' at exactly MERGE_SIMILARITY_THRESHOLD (0.85)", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.85 }]));
    const d = await m.classify(mkChunk());
    // score >= threshold → merge (threshold is 0.85)
    assert.strictEqual(d.action, "merge");
  });

  it("skips chunks shorter than MIN_CHUNK_LENGTH_FOR_MERGE", async () => {
    let called = false;
    const retrieval = {
      query: async () => { called = true; return [{ score: 0.99, chunkRef: { filePath: "f" }, text: "" }]; },
    } as any;
    const m = new SimilarityMatcher(retrieval);
    const shortChunk = mkChunk("short");
    shortChunk.embeddingText = "short";
    await m.classify(shortChunk);
    assert.strictEqual(called, false);
  });

  it("includes targetPath in merge decisions", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{
      score: 0.90,
      chunkRef: { filePath: "output/existing.md", chunkIndex: 0 },
    }]));
    const d = await m.classify(mkChunk());
    assert.strictEqual(d.action, "merge");
    if (d.action === "merge") {
      assert.strictEqual(d.targetPath, "output/existing.md");
    }
  });

  it("handles null/empty chunk text gracefully", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.99 }]));
    const chunk = mkChunk("");
    chunk.embeddingText = "";
    const d = await m.classify(chunk);
    // Short text → no query → keep
    assert.strictEqual(d.action, "keep");
  });

  it("throws when retrieval fails (propagates errors)", async () => {
    const retrieval = {
      query: async () => { throw new Error("retrieval down"); },
    } as any;
    const m = new SimilarityMatcher(retrieval);
    await assert.rejects(
      () => m.classify(mkChunk()),
      /retrieval down/,
    );
  });
});

describe("SimilarityMatcher.findBestMatch", () => {
  it("returns null below threshold", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{ score: 0.50 }]));
    const result = await (m as any).findBestMatch(mkChunk());
    assert.strictEqual(result, null);
  });

  it("returns match above threshold", async () => {
    const m = new SimilarityMatcher(mockRetrieval([{
      score: 0.90,
      chunkRef: { filePath: "out/file.md", chunkIndex: 0 },
      text: "matched text",
    }]));
    const result = await (m as any).findBestMatch(mkChunk());
    assert.notStrictEqual(result, null);
    assert.strictEqual(result.filePath, "out/file.md");
    assert.strictEqual(result.score, 0.90);
  });

  it("picks highest-scored result (retrieval returns sorted)", async () => {
    const m = new SimilarityMatcher(mockRetrieval([
      { score: 0.95, chunkRef: { filePath: "b.md", chunkIndex: 0 } },
      { score: 0.90, chunkRef: { filePath: "c.md", chunkIndex: 0 } },
      { score: 0.88, chunkRef: { filePath: "a.md", chunkIndex: 0 } },
    ]));
    const result = await (m as any).findBestMatch(mkChunk());
    assert.strictEqual(result.filePath, "b.md");
  });
});

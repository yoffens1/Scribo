// src/test/refinery/unit/DeduplicationStage.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { DeduplicationStage } from "@refinery/stages/04-DeduplicationStage";
import { mkChunk } from "../helpers/mkChunk";
import { nullLogger } from "../helpers/nullLogger";

describe("DeduplicationStage", () => {
  it("partitions chunks correctly (keep/merge/reject)", async () => {
    let callCount = 0;
    const retrieval = {
      query: async (text: string) => {
        callCount++;
        if (text.includes("dup")) return [{ score: 0.99, chunkRef: { filePath: "out/dup.md", chunkIndex: 0 }, text: "" }];
        if (text.includes("similar")) return [{ score: 0.90, chunkRef: { filePath: "out/similar.md", chunkIndex: 0 }, text: "" }];
        return [];
      },
    } as any;

    const ctx = { retrieval, logger: nullLogger() } as any;

    const chunks = [
      mkChunk("dup chunk content here long enough for similarity search"),
      mkChunk("similar chunk content here long enough for search"),
      mkChunk("unique chunk content here long enough for lookup"),
    ];

    const result = await new DeduplicationStage().run(chunks, ctx);

    assert.strictEqual(result.remaining.length, 1, "only unique chunk should remain");
    assert.strictEqual(result.decisions.length, 3);
    const actions = result.decisions.map(d => d.action).sort();
    assert.deepStrictEqual(actions, ["keep", "merge", "reject"]);
  });

  it("all chunks kept when no duplicates found", async () => {
    const retrieval = {
      query: async () => [],
    } as any;
    const ctx = { retrieval, logger: nullLogger() } as any;

    const chunks = [
      mkChunk("content one long enough for similarity"),
      mkChunk("content two long enough for similarity"),
    ];

    const result = await new DeduplicationStage().run(chunks, ctx);
    assert.strictEqual(result.remaining.length, 2);
    assert.ok(result.decisions.every(d => d.action === "keep"));
  });

  it("handles empty chunks array", async () => {
    const retrieval = { query: async () => [] } as any;
    const ctx = { retrieval, logger: nullLogger() } as any;
    const result = await new DeduplicationStage().run([], ctx);
    assert.strictEqual(result.decisions.length, 0);
    assert.strictEqual(result.remaining.length, 0);
  });

  it("merge decisions have targetPath set", async () => {
    const retrieval = {
      query: async () => [{ score: 0.90, chunkRef: { filePath: "out/existing.md", chunkIndex: 0 }, text: "match" }],
    } as any;
    const ctx = { retrieval, logger: nullLogger() } as any;

    const chunks = [mkChunk("similar content here long enough for merging into existing")];
    const result = await new DeduplicationStage().run(chunks, ctx);

    const mergeDecisions = result.decisions.filter(d => d.action === "merge");
    assert.strictEqual(mergeDecisions.length, 1);
    assert.strictEqual((mergeDecisions[0] as any).targetPath, "out/existing.md");
  });

  it("short chunks are kept (below MIN_CHUNK_LENGTH)", async () => {
    let queried = false;
    const retrieval = {
      query: async () => { queried = true; return []; },
    } as any;
    const ctx = { retrieval, logger: nullLogger() } as any;

    const shortChunk = mkChunk("short");
    shortChunk.text = "short";
    const result = await new DeduplicationStage().run([shortChunk], ctx);
    assert.strictEqual(result.decisions.length, 1);
    assert.strictEqual(result.decisions[0].action, "keep");
    assert.strictEqual(queried, false, "should not query retrieval for short chunks");
  });
});

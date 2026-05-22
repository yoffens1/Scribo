// src/test/refinery/unit/TaxonomyStage.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { TaxonomyStage } from "@refinery/stages/07-TaxonomyStage";
import { mkChunk } from "../helpers/mkChunk";
import { nullLogger } from "../helpers/nullLogger";

const mkChunks = (count: number) =>
  Array.from({ length: count }, (_, i) => mkChunk(`chunk ${i} with enough content for taxonomy generation purposes`));

describe("TaxonomyStage", () => {
  it("does not batch when under threshold", async () => {
    let calls = 0;
    const ctx = {
      llm: {
        generateMessages: async () => { calls++; return { text: '{"roots":[],"rationale":"test"}' }; },
      } as any,
      logger: nullLogger(),
    } as any;

    await new TaxonomyStage().run(mkChunks(10), ctx);
    assert.strictEqual(calls, 1, "should make exactly 1 LLM call for 10 chunks");
  });

  it("batches large chunk sets over MAX_CHUNKS_PER_TAXONOMY_CALL (50)", async () => {
    let calls = 0;
    const ctx = {
      llm: {
        generateMessages: async () => { calls++; return { text: '{"roots":[],"rationale":"test"}' }; },
      } as any,
      logger: nullLogger(),
    } as any;

    await new TaxonomyStage().run(mkChunks(120), ctx);
    assert.strictEqual(calls, 3, "120 chunks → 50+50+20 = 3 LLM calls");
  });

  it("returns merged taxonomy from batches", async () => {
    let batch = 0;
    const ctx = {
      llm: {
        generateMessages: async () => {
          batch++;
          return {
            text: JSON.stringify({
              roots: [{ name: `topic-${batch}`, description: "", children: [], assignedChunks: [] }],
              rationale: `batch-${batch}`,
            }),
          };
        },
      } as any,
      logger: nullLogger(),
    } as any;

    const result = await new TaxonomyStage().run(mkChunks(60), ctx);
    assert.strictEqual(result.roots.length, 2);
    assert.ok(result.rationale.includes("batch-1"));
    assert.ok(result.rationale.includes("batch-2"));
  });

  it("throws on invalid LLM JSON", async () => {
    const ctx = {
      llm: {
        generateMessages: async () => ({ text: "not json at all" }),
      } as any,
      logger: nullLogger(),
    } as any;

    await assert.rejects(
      () => new TaxonomyStage().run([mkChunk("test chunk long enough")], ctx),
      /TaxonomyGenerator.*valid JSON/,
    );
  });

  it("handles empty chunk array", async () => {
    const ctx = {
      llm: { generateMessages: async () => ({ text: "{}" }) } as any,
      logger: nullLogger(),
    } as any;

    const result = await new TaxonomyStage().run([], ctx);
    assert.strictEqual(result.roots.length, 0);
    assert.strictEqual(result.rationale, "no chunks to organize");
  });

  it("single batch (exactly at threshold) makes one call", async () => {
    let calls = 0;
    const ctx = {
      llm: {
        generateMessages: async () => { calls++; return { text: '{"roots":[],"rationale":"test"}' }; },
      } as any,
      logger: nullLogger(),
    } as any;

    await new TaxonomyStage().run(mkChunks(50), ctx);
    assert.strictEqual(calls, 1);
  });
});

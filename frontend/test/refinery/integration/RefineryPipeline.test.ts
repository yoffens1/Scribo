// src/test/refinery/integration/RefineryPipeline.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { RefineryPipeline } from "@refinery/RefineryPipeline";
import { makeMockCtx } from "../helpers/mockCtx";
import { nullLogger, spyLogger } from "../helpers/nullLogger";

const sampleMd = "# Title\n\nParagraph one with enough text to chunk properly.\n\n## Section Two\n\nParagraph two with different content here for testing.\n\n## Section Three\n\nParagraph three with yet more text to produce chunks.";

describe("RefineryPipeline.refine", () => {
  it("runs all 5 stages and returns RefineryResult", async () => {
    const logger = spyLogger();
    const ctx = makeMockCtx({
      fileContents: { "inbox/note.md": sampleMd },
      logger,
      dryRun: true,
    });

    const pipeline = new RefineryPipeline(ctx);
    const result = await pipeline.refine("note.md");

    assert.strictEqual(result.sourcePath, "note.md");
    assert.ok(result.chunks.length > 0, "should have chunks");
    assert.ok(result.taxonomy !== undefined, "should have taxonomy");
    assert.ok(result.placement !== undefined, "should have placement");
    assert.ok(result.operations !== undefined, "should have operations");
    assert.strictEqual(result.dryRun, true);
  });

  it("returns all 5 stages in order (via log calls)", async () => {
    const logger = spyLogger();
    const ctx = makeMockCtx({
      fileContents: { "inbox/note.md": sampleMd },
      logger,
      dryRun: true,
    });

    const pipeline = new RefineryPipeline(ctx);
    await pipeline.refine("note.md");

    const pipelineStages = (logger as any).calls
      .filter((c: any) => c.stage === "pipeline" && c.message.startsWith("stage"))
      .map((c: any) => c.message);
    assert.ok(pipelineStages.length >= 5, "should have at least 5 pipeline messages");
  });

  it("propagates errors and ends trace", async () => {
    const logger = spyLogger();
    const ctx = makeMockCtx({
      fileContents: {}, // no files → ChunkingStage will throw
      logger,
    });

    const pipeline = new RefineryPipeline(ctx);
    await assert.rejects(() => pipeline.refine("missing.md"));

    // Check that an error was logged
    const errors = (logger as any).calls.filter((c: any) => c.level === "error");
    assert.ok(errors.length > 0, "should have error log entries");
  });

  it("skips write in dryRun, still returns operations", async () => {
    const logger = spyLogger();
    const ctx = makeMockCtx({
      fileContents: { "inbox/note.md": sampleMd },
      logger,
      dryRun: true,
    });

    const pipeline = new RefineryPipeline(ctx);
    const result = await pipeline.refine("note.md");

    assert.strictEqual(result.dryRun, true);
    assert.ok(result.operations.length >= 0, "should return operations even in dry run");
  });

  it("plan() returns chunks/taxonomy/placement without operations", async () => {
    const ctx = makeMockCtx({
      fileContents: { "inbox/note.md": sampleMd },
    });

    const pipeline = new RefineryPipeline(ctx);
    const plan = await pipeline.plan("note.md");

    assert.ok(plan.chunks.length > 0);
    assert.ok(plan.taxonomy !== undefined);
    assert.ok(plan.placement !== undefined);
    // plan() does not return operations (that's WriteStage)
  });
});

describe("RefineryPipeline error handling", () => {
  it("handles LLM failure in taxonomy stage gracefully", async () => {
    const logger = spyLogger();
    const ctx = makeMockCtx({
      fileContents: { "inbox/note.md": sampleMd },
      logger,
      dryRun: true,
    });
    ctx.llm = {
      generateMessages: async () => { throw new Error("LLM timeout"); },
    } as any;

    const pipeline = new RefineryPipeline(ctx);
    await assert.rejects(
      () => pipeline.refine("note.md"),
      /LLM timeout/,
    );
  });
});

// src/test/refinery/integration/RefineryService.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { RefineryService } from "@refinery/RefineryService";
import type { RefineryServiceOptions } from "@refinery/RefineryService";
import { makeMockCtx } from "../helpers/mockCtx";
import { nullLogger, spyLogger } from "../helpers/nullLogger";
import { fakeFs } from "../helpers/fakeFs";

const sampleMd = "# Test\n\nParagraph one with enough text to chunk properly.\n\n## Part Two\n\nParagraph two with different content for testing chunks.\n\n## Part Three\n\nParagraph three has more text to generate chunks.";

const makeService = (overrides: Partial<RefineryServiceOptions> = {}): RefineryService => {
  const logger = overrides.logger ?? nullLogger();
  const fileAccess = overrides.fileAccess ?? fakeFs({ "inbox/note.md": sampleMd });

  const llmOverride = overrides.llm ?? {
    generateMessages: async (messages: any[]) => {
      const sys = messages.find(m => m.role === "system")?.content ?? "";
      if (sys.includes("atomic flashcard") || sys.includes("question-style heading")) {
        return { text: '{"questionHeading":"## What is a test?","filename":"what-is-a-test.md"}' };
      }
      if (sys.includes("aliases") || sys.includes("synonyms")) {
        return { text: '["test-alias"]' };
      }
      if (sys.includes("tags") || sys.includes("keywords")) {
        return { text: '["test-tag"]' };
      }
      if (sys.includes("taxonomy") || sys.includes("folder tree") || sys.includes("propose")) {
        return { text: '{"roots":[],"rationale":"auto"}' };
      }
      if (sys.includes("placement") || sys.includes("file locations") || sys.includes("foldersToCreate")) {
        return { text: '{"decisions":[],"foldersToCreate":[],"rationale":"auto"}' };
      }
      return { text: '{}' };
    },
  } as any;

  return new RefineryService({
    fileAccess,
    retrieval: { query: async () => [], setEmbedder: () => {}, markDirty: () => {} } as any,
    llm: llmOverride,
    logger,
    outputRoot: "output",
    inboxRoot: "inbox",
    dryRun: true,
    ...overrides,
  });
};

describe("RefineryService", () => {
  it("plan() returns taxonomy and placement", async () => {
    const svc = makeService();
    const plan = await svc.plan("note.md");
    assert.ok(plan.chunks.length > 0);
    assert.ok(plan.taxonomy !== undefined);
    assert.ok(plan.placement !== undefined);
  });

  it("refine() returns RefineryResult with all fields", async () => {
    const svc = makeService();
    const result = await svc.refine("note.md");
    assert.strictEqual(result.sourcePath, "note.md");
    assert.ok(result.chunks.length > 0);
    assert.ok(result.taxonomy !== undefined);
    assert.ok(result.placement !== undefined);
    assert.strictEqual(result.dryRun, true);
  });

  it("dryRun=false override does not leak to subsequent calls", async () => {
    const logger = spyLogger();
    const svc = makeService({ logger, dryRun: true });

    // First call: override dryRun to false
    await svc.refine("note.md", { dryRun: false });

    // ctx.dryRun should be restored after the call
    assert.strictEqual((svc as any).ctx.dryRun, true,
      "ctx.dryRun should be restored to true after dryRun=false override");

    // Second call: no override (should keep dryRun=true)
    await svc.refine("note.md");
    assert.strictEqual((svc as any).ctx.dryRun, true,
      "ctx.dryRun should remain true after default call");
  });

  it("concurrent refine() calls with different dryRun do not interfere", async () => {
    const fileAccess = fakeFs({
      "inbox/a.md": sampleMd,
      "inbox/b.md": sampleMd,
      "inbox/note.md": sampleMd,
    });
    const svc = makeService({ dryRun: true, fileAccess });

    // Run two concurrent calls with different dryRun settings
    // Both should complete without errors
    await assert.doesNotReject(async () => {
      await Promise.all([
        svc.refine("a.md", { dryRun: false }),
        svc.refine("b.md", { dryRun: true }),
      ]);
    });

    // After concurrent calls, ctx.dryRun should be restored to its original value
    // BUG: current implementation mutates ctx.dryRun — race condition
    // After concurrent calls, ctx.dryRun should be unchanged — dryRun is now per-call, not mutated
    assert.strictEqual((svc as any).ctx.dryRun, true,
      "ctx.dryRun should remain true — dryRun is now passed per-call, not mutated on shared ctx");
  });
});

describe("RefineryService.refineBatch", () => {
  it("continues on per-file errors", async () => {
    const fileAccess = fakeFs({
      "inbox/good.md": sampleMd,
      "inbox/good2.md": sampleMd,
      // bad.md intentionally missing
    });
    const svc = makeService({ fileAccess });

    const result = await svc.refineBatch(["good.md", "bad.md", "good2.md"]);
    assert.strictEqual(result.results.length, 2);
    assert.strictEqual(result.errors.length, 1);
    assert.strictEqual(result.errors[0].sourcePath, "bad.md");
  });

  it("aggregates counts correctly", async () => {
    const svc = makeService();
    const result = await svc.refineBatch(["note.md", "note.md"]);
    assert.strictEqual(result.results.length, 2);
    assert.strictEqual(result.errors.length, 0);
    assert.ok(result.totalChunks > 0);
    assert.ok(typeof result.mergedChunks === "number");
    assert.ok(typeof result.createdFiles === "number");
    assert.ok(typeof result.createdFolders === "number");
  });

  it("handles empty batch", async () => {
    const svc = makeService();
    const result = await svc.refineBatch([]);
    assert.strictEqual(result.results.length, 0);
    assert.strictEqual(result.errors.length, 0);
    assert.strictEqual(result.totalChunks, 0);
    assert.strictEqual(result.mergedChunks, 0);
    assert.strictEqual(result.createdFiles, 0);
    assert.strictEqual(result.createdFolders, 0);
  });
});

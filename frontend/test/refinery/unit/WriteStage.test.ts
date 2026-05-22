// src/test/refinery/unit/WriteStage.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { WriteStage } from "@refinery/stages/09-WriteStage";
import type { ChunkWithHash } from "@refinery/types/chunk-decision";
import type { PlacementPlan } from "@refinery/placement/types/placement";
import type { WriteInput } from "@refinery/stages/09-WriteStage";

const chunk: ChunkWithHash = {
  hash: "h1",
  embeddingText: "body content",
  generationText: "body content",
  text: "body content",
  index: 0,
  sourcePath: "inbox/source.md",
};

const chunk2: ChunkWithHash = {
  hash: "h2",
  embeddingText: "second body",
  generationText: "second body",
  text: "second body",
  index: 1,
  sourcePath: "inbox/source2.md",
};

const ctx = {
  outputRoot: "out",
  inboxRoot: "inbox",
  fileAccess: {} as any,
  retrieval: {} as any,
  llm: {} as any,
  logger: { log: () => {} } as any,
  dryRun: true,
};

const buildOps = (plan: PlacementPlan, chunks: ChunkWithHash[]) => {
  const stage = new WriteStage();
  const input: WriteInput = { plan, chunks };
  return (stage as any).buildOperations(input, ctx);
};

describe("WriteStage.buildOperations", () => {
  it("emits create_folder ops for parent dirs of files", async () => {
    // Folders are derived from actual file paths, not from LLM's foldersToCreate
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "notes/ai/ml.md", action: "create", reason: "" }],
        foldersToCreate: ["notes", "notes/ai"], // LLM still sends these, but we ignore them
        rationale: "",
      },
      [chunk],
    );
    // create_folder for "out/notes/ai" (parent of ml.md), then create_file
    assert.strictEqual(ops[0].type, "create_folder");
    assert.strictEqual(ops[1].type, "create_file");
    assert.strictEqual(ops.length, 2);
  });

  it("skips decisions with no matching chunk", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "missing", outputPath: "x.md", action: "create", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [],
    );
    assert.strictEqual(ops.length, 0);
  });

  it("maps 'create' to create_file op", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "file.md", action: "create", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    assert.strictEqual(ops.length, 1);
    assert.strictEqual(ops[0].type, "create_file");
  });

  it("maps 'merge' to merge_chunk op", async () => {
    const ops = buildOps(
      {
        decisions: [{
          chunkHash: "h1", outputPath: "existing.md", action: "merge",
          existingTarget: "existing.md", reason: "",
        }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    // merge at root: 1 op
    assert.strictEqual(ops.length, 1);
    assert.strictEqual(ops[0].type, "merge_chunk");
  });

  it("maps 'rename' to move_file + merge_chunk ops", async () => {
    const ops = buildOps(
      {
        decisions: [{
          chunkHash: "h1", outputPath: "sub/new-name.md", action: "rename",
          existingTarget: "out/old-name.md", reason: "",
        }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    // create_folder for "sub" derived from the output path, then move + merge
    assert.strictEqual(ops.length, 3);
    assert.strictEqual(ops[0].type, "create_folder");
    assert.strictEqual(ops[1].type, "move_file");
    assert.strictEqual(ops[2].type, "merge_chunk");
  });

  it("maps 'nest' to create_folder + merge_chunk ops", async () => {
    const ops = buildOps(
      {
        decisions: [{
          chunkHash: "h1", outputPath: "deep/nest/file.md", action: "nest", reason: "",
        }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    assert.strictEqual(ops.length, 2);
    assert.strictEqual(ops[0].type, "create_folder");
    assert.strictEqual(ops[1].type, "merge_chunk");
  });

  it("prefixes paths with outputRoot for create_file", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "f.md", action: "create", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    assert.strictEqual((ops[0] as any).path, "out/f.md");
  });

  it("prefixes folder paths with outputRoot", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "notes/ai/file.md", action: "create", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    assert.ok(ops.length >= 2, "should have folder + file ops");
    assert.strictEqual((ops[0] as any).path, "out/notes/ai");
  });

  it("create_file includes chunk content", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "file.md", action: "create", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    // Content now includes frontmatter (---\n---\n) + text
    assert.ok((ops[0] as any).content.includes("body content"));
  });

  it("merge_chunk includes sourceFile and chunkText", async () => {
    const ops = buildOps(
      {
        decisions: [{ chunkHash: "h1", outputPath: "target.md", action: "merge", reason: "" }],
        foldersToCreate: [],
        rationale: "",
      },
      [chunk],
    );
    assert.strictEqual((ops[0] as any).sourceFile, "inbox/source.md");
    // Content now includes frontmatter prefix
    assert.ok((ops[0] as any).chunkText.includes("body content"));
  });

  it("handles multiple decisions of mixed actions", async () => {
    const plan: PlacementPlan = {
      decisions: [
        { chunkHash: "h1", outputPath: "a.md", action: "create", reason: "" },
        { chunkHash: "h2", outputPath: "b.md", action: "create", reason: "" },
      ],
      foldersToCreate: [],
      rationale: "",
    };
    const ops = buildOps(plan, [chunk, chunk2]);
    // No subfolders → just 2 files
    assert.strictEqual(ops.length, 2);
    assert.strictEqual(ops[0].type, "create_file");
    assert.strictEqual(ops[1].type, "create_file");
  });
});

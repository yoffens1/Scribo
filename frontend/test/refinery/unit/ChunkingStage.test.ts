// src/test/refinery/unit/ChunkingStage.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ChunkingStage } from "@refinery/stages/01-ChunkingStage";
import { makeMockCtx } from "../helpers/mockCtx";

const sampleMd = "# Title\n\nParagraph one with enough text to chunk.\n\n## Section Two\n\nParagraph two with different content here.\n\n## Section Three\n\nParagraph three with yet more text.";

describe("ChunkingStage", () => {
  const ctx = makeMockCtx({
    fileContents: { "inbox/note.md": sampleMd },
  });

  it("produces chunks with stable hashes", async () => {
    const stage = new ChunkingStage();
    const chunks1 = await stage.run("note.md", ctx);
    const chunks2 = await stage.run("note.md", ctx);
    assert.strictEqual(chunks1.length, chunks2.length);
    for (let i = 0; i < chunks1.length; i++) {
      assert.strictEqual(chunks1[i].hash, chunks2[i].hash, `hash mismatch at index ${i}`);
    }
  });

  it("each chunk has index, sourcePath", async () => {
    const stage = new ChunkingStage();
    const chunks = await stage.run("note.md", ctx);
    assert.ok(chunks.length > 0, "should produce at least one chunk");
    for (let i = 0; i < chunks.length; i++) {
      assert.strictEqual(chunks[i].index, i);
      assert.strictEqual(chunks[i].sourcePath, "note.md");
    }
  });

  it("each chunk has non-empty embeddingText, generationText, text and hash", async () => {
    const stage = new ChunkingStage();
    const chunks = await stage.run("note.md", ctx);
    for (const c of chunks) {
      assert.ok(c.embeddingText.length > 0, "embeddingText should not be empty");
      assert.ok(c.generationText.length > 0, "generationText should not be empty");
      assert.ok(c.text.length > 0, "text (backward compat) should not be empty");
      assert.ok(c.hash.length > 0, "chunk hash should not be empty");
    }
  });

  it("embeddingText and generationText are paired (same count)", async () => {
    // The structural split guarantees equal counts for both representations
    const stage = new ChunkingStage();
    const chunks = await stage.run("note.md", ctx);
    // Each chunk has both fields populated — paired by construction
    for (const c of chunks) {
      assert.ok(c.embeddingText.length > 0 && c.generationText.length > 0,
        `chunk ${c.index}: both texts should be non-empty`);
    }
  });

  it("returns empty array for empty file", async () => {
    const emptyCtx = makeMockCtx({
      fileContents: { "inbox/empty.md": "" },
    });
    const stage = new ChunkingStage();
    const chunks = await stage.run("empty.md", emptyCtx);
    // Chunker treats empty content as no chunks
    assert.strictEqual(chunks.length, 0);
  });

  it("respects inboxRoot in path construction", async () => {
    const customCtx = makeMockCtx({
      inboxRoot: "custom-inbox",
      fileContents: { "custom-inbox/note.md": sampleMd },
    });
    const stage = new ChunkingStage();
    // Should not throw — path is "custom-inbox/note.md"
    const chunks = await stage.run("note.md", customCtx);
    assert.ok(chunks.length > 0);
  });

  it("throws when file not found", async () => {
    const stage = new ChunkingStage();
    await assert.rejects(
      () => stage.run("nonexistent.md", ctx),
      /ENOENT/,
    );
  });
});

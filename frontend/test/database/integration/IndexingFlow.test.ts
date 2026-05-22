// src/test/database/integration/IndexingFlow.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { VectorDatabase } from "@database/Database";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "int-indexing";

class MockEmbedder {
  model = "mock-model";
  dim = 4;
  private chunked: Array<{ text: string; embedding: Float32Array }> = [];
  setChunked(c: Array<{ text: string; embedding: Float32Array }>) { this.chunked = c; }
  getDimensions() { return this.dim; }
  embedChunked() { return Promise.resolve(this.chunked); }
}

function makeEmb(v: number[]): Float32Array { return new Float32Array(v); }

describe("Integration: Indexing Flow", () => {
  let db: VectorDatabase;
  let embedder: MockEmbedder;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    embedder = new MockEmbedder();
  });

  afterEach(async () => {
    try { await db.close(); } catch {}
  });

  it("full pipeline: init → index → retrieve → rename → delete → reopen", async () => {
    // Init
    await db.initialize();

    // Index a file
    embedder.setChunked([
      { text: "hello", embedding: makeEmb([1, 2, 3, 4]) },
      { text: "world", embedding: makeEmb([5, 6, 7, 8]) },
    ]);
    await db.addMdFile("note.md", "hello world", embedder as any);

    // Retrieve
    const chunks = await db.getFileChunks("note.md");
    assert.equal(chunks.length, 2);
    assert.equal(chunks[0].chunkText, "hello");
    assert.equal(chunks[0].embedding![0], 1);

    // Rename
    await db.renameFile("note.md", "renamed.md");
    assert.equal((await db.getFileChunks("note.md")).length, 0);
    assert.equal((await db.getFileChunks("renamed.md")).length, 2);

    // Index another
    embedder.setChunked([{ text: "extra", embedding: makeEmb([9, 9, 9, 9]) }]);
    await db.addMdFile("extra.md", "extra", embedder as any);
    assert.equal((await db.getAllChunks()).length, 3);

    // Soft delete
    await db.softDeleteFile("extra.md");
    assert.equal((await db.getAllChunks(false)).length, 2);
    assert.equal((await db.getAllChunks(true)).length, 3);

    // Restore
    await db.restoreFile("extra.md");
    assert.equal((await db.getAllChunks(false)).length, 3);

    // Hard delete
    await db.hardDeleteFile("extra.md");
    assert.equal((await db.getAllChunks(true)).length, 2);
  });

  it("unchanged file is not re-indexed", async () => {
    await db.initialize();

    embedder.setChunked([{ text: "a", embedding: makeEmb([1, 2, 3, 4]) }]);
    await db.addMdFile("stable.md", "same content", embedder as any);

    const count1 = (await db.getAllChunks()).length;

    // Re-index with same content — should skip
    embedder.setChunked([{ text: "should not be used", embedding: makeEmb([9, 9, 9, 9]) }]);
    await db.addMdFile("stable.md", "same content", embedder as any);

    const count2 = (await db.getAllChunks()).length;
    assert.equal(count1, count2, "no new chunks should be created");

    // Change content — should re-index
    embedder.setChunked([{ text: "new", embedding: makeEmb([2, 2, 2, 2]) }]);
    await db.addMdFile("stable.md", "different content", embedder as any);

    const count3 = (await db.getAllChunks()).length;
    assert.equal(count3, 1, "old chunks replaced with new ones");
  });
});

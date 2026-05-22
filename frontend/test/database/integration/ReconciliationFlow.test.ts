// src/test/database/integration/ReconciliationFlow.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { VectorDatabase } from "@database/Database";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "int-recon";

class MockEmbedder {
  model = "test";
  dim = 4;
  private chunked: Array<{ text: string; embedding: Float32Array }> = [];
  setChunked(c: Array<{ text: string; embedding: Float32Array }>) { this.chunked = c; }
  getDimensions() { return this.dim; }
  embedChunked() { return Promise.resolve(this.chunked); }
}

function makeEmb(v: number[]): Float32Array { return new Float32Array(v); }

describe("Integration: Reconciliation Flow", () => {
  let db: VectorDatabase;
  let embedder: MockEmbedder;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    embedder = new MockEmbedder();
    await db.initialize();
  });

  afterEach(async () => {
    try { await db.close(); } catch {}
  });

  it("detects missing vault files and soft-deletes them", async () => {
    // Index a file
    embedder.setChunked([{ text: "x", embedding: makeEmb([1, 2, 3, 4]) }]);
    await db.addMdFile("present.md", "content", embedder as any);

    // reconcile with empty vault
    await db.reconcile(
      embedder as any,
      async () => [],
      async () => { throw new Error("should not read"); },
    );

    const chunks = await db.getFileChunks("present.md");
    assert.equal(chunks.length, 0, "chunks should be hidden (soft-deleted)");
  });

  it("detects new vault files and indexes them", async () => {
    embedder.setChunked([{ text: "new", embedding: makeEmb([1, 2, 3, 4]) }]);
    await db.reconcile(
      embedder as any,
      async () => ["new.md"],
      async (p) => p === "new.md" ? "new content" : "",
    );

    const chunks = await db.getFileChunks("new.md");
    assert.equal(chunks.length, 1);
  });

  it("skips unchanged files via mtime optimization", async () => {
    embedder.setChunked([{ text: "stable", embedding: makeEmb([1, 2, 3, 4]) }]);
    await db.addMdFile("stable.md", "stable content", embedder as any, 5000);

    let readCalled = false;
    await db.reconcile(
      embedder as any,
      async () => ["stable.md"],
      async () => { readCalled = true; return "should not be read"; },
      async () => 5000, // same mtime
    );

    assert.equal(readCalled, false, "readFile should not be called on mtime match");
  });

  it("reindexes when mtime changed", async () => {
    embedder.setChunked([{ text: "old", embedding: makeEmb([1, 1, 1, 1]) }]);
    await db.addMdFile("mod.md", "v1", embedder as any, 1000);

    let readCalled = false;
    embedder.setChunked([{ text: "updated", embedding: makeEmb([2, 2, 2, 2]) }]);
    await db.reconcile(
      embedder as any,
      async () => ["mod.md"],
      async () => { readCalled = true; return "v2"; },
      async () => 2000, // different mtime
    );

    assert.equal(readCalled, true, "readFile should be called when mtime differs");
    const chunks = await db.getFileChunks("mod.md");
    assert.equal(chunks[0].chunkText, "updated");
  });
});

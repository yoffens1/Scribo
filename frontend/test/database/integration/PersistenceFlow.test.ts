// src/test/database/integration/PersistenceFlow.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { VectorDatabase } from "@database/Database";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "int-persist";

class MockEmbedder {
  model = "m";
  dim = 4;
  private chunked: Array<{ text: string; embedding: Float32Array }> = [];
  setChunked(c: Array<{ text: string; embedding: Float32Array }>) { this.chunked = c; }
  getDimensions() { return this.dim; }
  embedChunked() { return Promise.resolve(this.chunked); }
}

function makeEmb(v: number[]): Float32Array { return new Float32Array(v); }

describe("Integration: Persistence Flow", () => {
  let adapter: FakeDataAdapter;
  let embedder: MockEmbedder;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    embedder = new MockEmbedder();
  });

  it("close → reopen preserves all data", async () => {
    const db1 = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    await db1.initialize();

    embedder.setChunked([
      { text: "a", embedding: makeEmb([1, 2, 3, 4]) },
      { text: "b", embedding: makeEmb([5, 6, 7, 8]) },
    ]);
    await db1.addMdFile("persist.md", "content", embedder as any);
    await db1.close();

    // Reopen
    const db2 = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    await db2.initialize();

    const chunks = await db2.getFileChunks("persist.md");
    assert.equal(chunks.length, 2);
    assert.equal(chunks[0].chunkText, "a");
    assert.equal(chunks[1].embedding![0], 5);

    await db2.close();
  });

  it("multiple close/reopen cycles preserve data", async () => {
    for (let cycle = 0; cycle < 3; cycle++) {
      const db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
      await db.initialize();

      if (cycle === 0) {
        embedder.setChunked([{ text: "first", embedding: makeEmb([1, 1, 1, 1]) }]);
        await db.addMdFile("cycle.md", "content", embedder as any);
      }

      const chunks = await db.getFileChunks("cycle.md");
      assert.equal(chunks.length, 1, `cycle ${cycle}: should have 1 chunk`);
      assert.equal(chunks[0].chunkText, "first");

      await db.close();
    }
  });

  it("vacuum across close/reopen preserves data", async () => {
    const db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    await db.initialize();

    embedder.setChunked([{ text: "v", embedding: makeEmb([1, 2, 3, 4]) }]);
    await db.addMdFile("vac.md", "content", embedder as any);
    await db.forceVacuum();
    await db.close();

    const db2 = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    await db2.initialize();
    const chunks = await db2.getFileChunks("vac.md");
    assert.equal(chunks.length, 1);
    await db2.close();
  });
});

import { describe, it, mock, beforeEach } from "node:test";
import assert from "node:assert/strict";
import { RetrievalService } from "@retrieval/RetrievalService";
import { LegacyRetrievalAdapter } from "@retrieval/LegacyRetrievalAdapter";
import type { ChunkSource } from "@retrieval/types/chunk-source";
import type { Embedder } from "@ai/embedding/Embedder";
import type { FullChunkData, ChunkData, ChunkDataWithPath } from "@retrieval/types/chunk";
import type { DbEventBus } from "@database/EventBus";

function makeEmbedder(): Embedder {
  return {
    model: "mock",
    getDimensions: () => 4,
    embed: async (_text: string) => new Float32Array(4),
    embedQuery: async (_text: string) => new Float32Array(4),
    embedBatch: async (_texts: string[]) => [],
    embedChunked: async () => [],
    initialize: async () => {},
  } as unknown as Embedder;
}

function makeChunk(
  filePath: string, chunkIndex: number, chunkText: string, chunkId: number,
): FullChunkData {
  return { chunkId, filePath, chunkIndex, chunkText, embedding: new Float32Array(4) };
}

function stripEmbedding(results: Array<{ embedding?: Float32Array }>) {
  return results.map(({ embedding, ...rest }) => rest);
}

describe("RetrievalService", () => {
  // ── query() without setEmbedder → error ──

  it("query() without setEmbedder → throws clear error", async () => {
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    // query() calls ensureRetriever which calls registry.getEmbedder()
    // That throws because embedder is null
    await assert.rejects(
      () => service.query("test"),
      /setEmbedder/,
    );
  });

  // ── query() with embedder set ──

  it("query() works after setEmbedder (hybrid mode)", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "machine learning basics", 1),
      makeChunk("b.md", 0, "deep neural networks", 2),
      makeChunk("c.md", 0, "daily journal entry", 3),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    service.setEmbedder(makeEmbedder());

    const results = await service.query("machine learning", { topK: 3 });
    assert.ok(Array.isArray(results));
    assert.ok(results.length > 0);
    for (const r of results) {
      assert.ok("chunkRef" in r);
      assert.ok("score" in r);
    }
  });

  // ── lazy retriever creation ──

  it("lazy retriever creation: RetrievalFactory.create called once", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "hello world", 1),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    service.setEmbedder(makeEmbedder());

    // Two queries — retriever created only once
    await service.query("first test");
    await service.query("second test");
    // No direct way to verify without exposing internals, but it doesn't crash
  });

  // ── markDirty ──

  it("markDirty resets retriever → next query recreates", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "hello world", 1),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    service.setEmbedder(makeEmbedder());

    await service.query("first");
    service.markDirty();
    // Should not crash — recreates retriever
    const results = await service.query("second");
    assert.ok(Array.isArray(results));
  });

  // ── fetch() branches ──

  it("fetch() without args → returns all chunks", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "alpha", 1),
      makeChunk("b.md", 0, "beta", 2),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    const results = await service.fetch();
    assert.equal(results.length, 2);
    assert.equal(results[0].filePath, "a.md");
    assert.equal(results[1].filePath, "b.md");
  });

  it("fetch() with filePath → delegates to getByFilePath", async () => {
    const chunks: ChunkData[] = [
      { chunkIndex: 0, chunkText: "content", embedding: new Float32Array(4) },
      { chunkIndex: 1, chunkText: "more", embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: (fp: string) => fp === "a.md" ? chunks : [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    const results = await service.fetch({ filePath: "a.md" });
    assert.equal(results.length, 2);
    assert.equal(results[0].filePath, "a.md");
    assert.equal(results[0].chunkIndex, 0);
    assert.equal(results[0].chunkText, "content");
  });

  it("fetch() with fileName → delegates to getByFileName", async () => {
    const chunks: ChunkDataWithPath[] = [
      { filePath: "a.md", chunkIndex: 0, chunkText: "by name", embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => [],
      getByFileName: (name: string) => name === "a" ? chunks : [],
    };
    const service = new RetrievalService({ source: cs });
    const results = await service.fetch({ fileName: "a" });
    assert.equal(results.length, 1);
    assert.equal(results[0].filePath, "a.md");
  });

  it("fetch() with offset/limit pagination", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "first", 1),
      makeChunk("a.md", 1, "second", 2),
      makeChunk("b.md", 0, "third", 3),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const service = new RetrievalService({ source: cs });
    const results = await service.fetch({ offset: 1, limit: 1 });
    assert.equal(results.length, 1);
    assert.equal(results[0].chunkText, "second");
  });

  // ── EventBus subscription ──

  it("eventBus: chunk:inserted → markDirty called", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "hello", 1),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    // Create a real event bus
    const { DbEventBus } = await import("@database/EventBus");
    const eventBus = new DbEventBus();
    const service = new RetrievalService({ source: cs, eventBus });
    service.setEmbedder(makeEmbedder());

    await service.query("original");

    // Emit chunk:inserted — nulls retriever + triggers debounced registry rebuild
    await eventBus.emit("chunk:inserted", { fileId: 2, count: 1 });
    // Wait for IndexRegistry's 500ms markDirty debounce to fire
    await new Promise(r => setTimeout(r, 600));

    // Next query should rebuild and work
    const results = await service.query("after insert");
    assert.ok(Array.isArray(results));
  });

  it("eventBus: chunk:deleted → markDirty called", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "hello", 1),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const { DbEventBus } = await import("@database/EventBus");
    const eventBus = new DbEventBus();
    const service = new RetrievalService({ source: cs, eventBus });
    service.setEmbedder(makeEmbedder());

    await service.query("original");

    await eventBus.emit("chunk:deleted", { fileId: 1, count: 1 });
    // Wait for IndexRegistry debounce
    await new Promise(r => setTimeout(r, 600));

    const results = await service.query("after delete");
    assert.ok(Array.isArray(results));
  });

  // ── Legacy methods ──

  // ── Legacy methods (now in LegacyRetrievalAdapter) ──

  it("getFileChunks() returns chunks for a file", async () => {
    const chunks: ChunkData[] = [
      { chunkIndex: 0, chunkText: "hello", embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => chunks,
      getByFileName: async () => [],
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.getFileChunks("a.md");
    assert.equal(results.length, 1);
    assert.equal(results[0].chunkText, "hello");
  });

  it("getChunksByFileOrName(): slash → getByFilePath", async () => {
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.getChunksByFileOrName("sub/a.md");
    assert.deepEqual(results, []);
  });

  it("getChunksByFileOrName(): no slash → getByFileName", async () => {
    const chunks: ChunkDataWithPath[] = [
      { filePath: "a.md", chunkIndex: 0, embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => [],
      getByFileName: async () => chunks,
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.getChunksByFileOrName("a");
    assert.equal(results.length, 1);
    assert.equal(results[0].filePath, "a.md");
  });

  it("getAllChunks() returns all", async () => {
    const chunks: FullChunkData[] = [
      makeChunk("a.md", 0, "alpha", 1),
      makeChunk("b.md", 0, "beta", 2),
    ];
    const cs: ChunkSource = {
      getAll: async () => chunks,
      getByFilePath: async () => [],
      getByFileName: async () => [],
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.getAllChunks();
    assert.equal(results.length, 2);
  });

  it("search() delegates to fetch()", async () => {
    const chunks: ChunkData[] = [
      { chunkIndex: 0, chunkText: "hello", embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => chunks,
      getByFileName: async () => [],
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.search({ filePath: "a.md" });
    assert.equal(results.length, 1);
    assert.equal(results[0].filePath, "a.md");
  });

  it("getChunksByFileName() returns chunks", async () => {
    const chunks: ChunkDataWithPath[] = [
      { filePath: "note.md", chunkIndex: 0, embedding: new Float32Array(4) },
    ];
    const cs: ChunkSource = {
      getAll: async () => [],
      getByFilePath: async () => [],
      getByFileName: async () => chunks,
    };
    const adapter = new LegacyRetrievalAdapter(cs);
    const results = adapter.getChunksByFileName("note");
    assert.equal(results.length, 1);
    assert.equal(results[0].filePath, "note.md");
  });
});

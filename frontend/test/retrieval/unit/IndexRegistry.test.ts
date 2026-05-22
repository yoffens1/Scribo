import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { IndexRegistry } from "@retrieval/IndexRegistry";
import type { ChunkSource } from "@retrieval/types/chunk-source";
import type { Embedder } from "@ai/embedding/Embedder";
import type { FullChunkData } from "@retrieval/types/chunk";

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

type ChunkSourceMock = ChunkSource & {
  getAll: ReturnType<typeof mock.fn>;
  getByFilePath: ReturnType<typeof mock.fn>;
  getByFileName: ReturnType<typeof mock.fn>;
};

function makeChunkSource(chunks: FullChunkData[]): ChunkSourceMock {
  return {
    getAll: mock.fn(() => chunks),
    getByFilePath: mock.fn(() => []),
    getByFileName: mock.fn(() => []),
  } as unknown as ChunkSourceMock;
}

function makeChunk(filePath: string, chunkIndex: number, chunkText: string): FullChunkData {
  return {
    chunkId: chunkIndex + 1,
    filePath,
    chunkIndex,
    chunkText,
    embedding: new Float32Array(4),
  };
}

describe("IndexRegistry", () => {
  // ── Lazy build ──

  it("getBm25Index: not built until called", async () => {
    const chunks: FullChunkData[] = [makeChunk("a.md", 0, "hello world")];
    const cs = makeChunkSource(chunks);
    const registry = new IndexRegistry(cs, makeEmbedder());

    // getAll should NOT have been called yet
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 0);

    registry.getBm25Index();
    // Now it should have been called
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 1);
  });

  it("getSearchEngine: throws if setEmbedder not called", async () => {
    const cs = makeChunkSource([]);
    const registry = new IndexRegistry(cs, null);
    assert.throws(() => registry.getSearchEngine(), /setEmbedder/);
  });

  // ── Cache ──

  it("second call does not rebuild (cache)", async () => {
    const chunks: FullChunkData[] = [makeChunk("a.md", 0, "hello")];
    const cs = makeChunkSource(chunks);
    const registry = new IndexRegistry(cs, makeEmbedder());

    registry.getBm25Index();
    registry.getBm25Index();
    // getAll called exactly once
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 1);
  });

  // ── markDirty ──

  it("markDirty → next getBm25Index rebuilds", async () => {
    const chunks: FullChunkData[] = [makeChunk("a.md", 0, "hello")];
    const cs = makeChunkSource(chunks);
    const registry = new IndexRegistry(cs, makeEmbedder());

    registry.getBm25Index();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 1);

    registry.markDirtyImmediate();
    registry.getBm25Index();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 2);
  });

  // ── BM25 and vector independence ──

  it("markDirty marks both BM25 and vector, but getBm25Index doesn't rebuild vector", async () => {
    const chunks: FullChunkData[] = [makeChunk("a.md", 0, "hello")];
    const cs = makeChunkSource(chunks);
    const embedder = makeEmbedder();
    const registry = new IndexRegistry(cs, embedder);

    registry.markDirtyImmediate();
    // getBm25Index rebuilds BM25 only, not vector
    registry.getBm25Index();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 1);

    // getSearchEngine now rebuilds vector — second getAll call
    registry.getSearchEngine();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 2);
  });

  // ── model change via setEmbedder ──

  it("setEmbedder + markDirty → next getSearchEngine rebuilds", async () => {
    const chunks: FullChunkData[] = [makeChunk("a.md", 0, "hello")];
    const cs = makeChunkSource(chunks);
    const embedder1 = makeEmbedder();
    const registry = new IndexRegistry(cs, embedder1);

    registry.getSearchEngine();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 1);

    const embedder2 = makeEmbedder();
    registry.setEmbedder(embedder2);
    registry.markDirtyImmediate();
    registry.getSearchEngine();
    assert.equal((cs.getAll as ReturnType<typeof mock.fn>).mock.callCount(), 2);
  });

  it("getEmbedder returns the set embedder", async () => {
    const embedder = makeEmbedder();
    const registry = new IndexRegistry(makeChunkSource([]), embedder);
    assert.equal(registry.getEmbedder(), embedder);
  });

  it("getEmbedder throws when null", async () => {
    const registry = new IndexRegistry(makeChunkSource([]), null);
    assert.throws(() => registry.getEmbedder(), /setEmbedder/);
  });
});

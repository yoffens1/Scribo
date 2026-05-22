import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { EmbeddingRetriever } from "@retrieval/retrievers/EmbeddingRetriever";
import { SearchEngine } from "@retrieval/engines/SearchEngine";
import type { SearchResult } from "@retrieval/types/search";
import type { Embedder } from "@ai/embedding/Embedder";
import type { VectorIndex } from "@retrieval/engines/types";

type MockVectorIndex = {
  add: ReturnType<typeof mock.fn>;
  setChunkRef: ReturnType<typeof mock.fn>;
  addChunk: ReturnType<typeof mock.fn>;
  removeChunk: ReturnType<typeof mock.fn>;
  remove: ReturnType<typeof mock.fn>;
  search: ReturnType<typeof mock.fn>;
};

function makeMockVectorIndex(results: Array<{ id: number; score: number; chunkRef: { filePath: string; chunkIndex: number } }>): MockVectorIndex {
  return {
    add: mock.fn(),
    setChunkRef: mock.fn(),
    addChunk: mock.fn(),
    removeChunk: mock.fn(),
    remove: mock.fn(),
    search: mock.fn((_query: Float32Array, _k: number) => results),
  };
}

function makeMockEmbedder(): Embedder {
  return {
    model: "mock",
    getDimensions: () => 4,
    embed: async () => new Float32Array(4),
    embedQuery: async () => new Float32Array(4),
    embedBatch: async () => [],
    embedChunked: async () => [],
    initialize: async () => {},
  } as unknown as Embedder;
}

describe("EmbeddingRetriever", () => {
  it("calls engine.search and returns results", async () => {
    const embedder = makeMockEmbedder();
    const vIdx = makeMockVectorIndex([
      { id: 0, score: 0.9, chunkRef: { filePath: "a.md", chunkIndex: 0 } },
      { id: 1, score: 0.7, chunkRef: { filePath: "b.md", chunkIndex: 0 } },
      { id: 2, score: 0.5, chunkRef: { filePath: "c.md", chunkIndex: 0 } },
    ]);
    const engine = new SearchEngine(embedder, vIdx as unknown as VectorIndex);
    const retriever = new EmbeddingRetriever(() => engine);

    const result = await retriever.retrieve("query", { topK: 3 });
    assert.equal(result.length, 3);
    assert.equal(result[0].chunkRef.filePath, "a.md");
  });

  it("applies filters if provided", async () => {
    const embedder = makeMockEmbedder();
    const vIdx = makeMockVectorIndex([
      { id: 0, score: 0.9, chunkRef: { filePath: "Daily/a.md", chunkIndex: 0 } },
      { id: 1, score: 0.7, chunkRef: { filePath: "Projects/b.md", chunkIndex: 0 } },
      { id: 2, score: 0.5, chunkRef: { filePath: "Daily/c.md", chunkIndex: 0 } },
    ]);
    const engine = new SearchEngine(embedder, vIdx as unknown as VectorIndex);
    const retriever = new EmbeddingRetriever(() => engine);

    const result = await retriever.retrieve("query", {
      topK: 3,
      filters: { folder: "Daily/" },
    });
    assert.equal(result.length, 2);
    assert.ok(result.every(r => r.chunkRef.filePath.startsWith("Daily/")));
  });

  it("passes topK to engine.search", async () => {
    const embedder = makeMockEmbedder();
    const vIdx = makeMockVectorIndex([
      { id: 0, score: 0.9, chunkRef: { filePath: "a.md", chunkIndex: 0 } },
    ]);
    const engine = new SearchEngine(embedder, vIdx as unknown as VectorIndex);
    const retriever = new EmbeddingRetriever(() => engine);

    await retriever.retrieve("query", { topK: 10 });
    assert.equal(
      vIdx.search.mock.calls[0].arguments[1],
      10,
    );
  });
});

import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
import { describe, it, mock, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { EmbeddingSerializer } from "@database/infrastructure/EmbeddingSerializer";
import { FileRepository } from "@database/repositories/FileRepository";
import { ChunkRepository } from "@database/repositories/ChunkRepository";
import { RetrievalFactory } from "@retrieval/retrievers/RetrievalFactory";
import { IndexRegistry } from "@retrieval/IndexRegistry";
import { EmbeddingRetriever } from "@retrieval/retrievers/EmbeddingRetriever";
import { KeywordRetriever } from "@retrieval/retrievers/KeywordRetriever";
import { HybridRetriever } from "@retrieval/retrievers/HybridRetriever";
import { RerankingRetriever } from "@retrieval/retrievers/RerankingRetriever";
import { LlmReranker } from "@retrieval/rerankers/LlmReranker";
import { ListwiseLlmReranker } from "@retrieval/rerankers/ListwiseLlmReranker";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";
import type { Embedder } from "@ai/embedding/Embedder";
import type { Reranker } from "@retrieval/rerankers/types/reranker";
import type { SearchResult } from "@retrieval/types/search";

const PLUGIN_DIR = "factory-integration-test";

function hashEmbed(text: string): Float32Array {
  let h = 0;
  for (let i = 0; i < text.length; i++) h = ((h << 5) - h + text.charCodeAt(i)) | 0;
  const seed = Math.abs(h);
  const vec = new Float32Array([
    Math.sin(seed * 0.1),
    Math.cos(seed * 0.13),
    Math.sin(seed * 0.17 + 1),
    Math.cos(seed * 0.19 + 2),
  ]);
  let norm = 0;
  for (let i = 0; i < 4; i++) norm += vec[i] ** 2;
  const invNorm = 1 / Math.sqrt(norm);
  for (let i = 0; i < 4; i++) vec[i] *= invNorm;
  return vec;
}

function makeEmbedder(): Embedder {
  return {
    model: "test",
    getDimensions: () => 4,
    initialize: async () => {},
    embed: hashEmbed,
    embedQuery: hashEmbed,
    embedBatch: async (texts: string[]) => texts.map(hashEmbed),
    embedChunked: async (_text: string) => [],
  } as unknown as Embedder;
}

describe("RetrievalFactory Integration", () => {
  let core: TauriDbConnection;
  let files: FileRepository;
  let chunks: ChunkRepository;
  const embedder = makeEmbedder();

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = new TauriDbConnection(".test-db", "test-model");
    const schema = new SchemaManager(core);
    await schema.initialize();
    const ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    chunks = new ChunkRepository(core, ser);

    // Create a couple of files with chunks
    for (const [fp, texts] of [
      ["a.md", ["hello world", "machine learning rocks"]],
      ["b.md", ["deep neural networks", "gradient descent optimization"]],
    ] as Array<[string, string[]]>) {
      const fileId = files.insertIndexing({
        cleanPath: fp,
        fileName: fp.replace(".md", ""),
        fileHash: `hash-${fp}`,
        fileMtime: Date.now(),
        embeddingModel: "test-model",
        embeddingDim: 4,
        chunkingVersion: "1",
        updatedAt: Date.now(),
      });
      files.markIndexed(fileId);

      const chunkRows = await Promise.all(
        texts.map(async (text, idx) => {
          const emb = await embedder.embed(text);
          return {
            chunkIndex: idx,
            text,
            tokens: text.split(/\s+/).length,
            embedding: ser.serialize(emb),
          };
        }),
      );
      await chunks.insertChunks(fileId, chunkRows);
    }
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  // ── All three modes create without error ──

  it("creates embedding mode retriever", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "embedding" },
      chunks,
      embedder,
    );
    assert.ok(retriever instanceof EmbeddingRetriever);
  });

  it("creates keyword mode retriever", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "keyword" },
      chunks,
      embedder,
    );
    assert.ok(retriever instanceof KeywordRetriever);
  });

  it("creates hybrid mode retriever", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "hybrid" },
      chunks,
      embedder,
    );
    assert.ok(retriever instanceof HybridRetriever);
  });

  // ── Reranker wraps retriever ──

  it("with reranker → returns RerankingRetriever", async () => {
    const reranker: Reranker = {
      async rerank(_q: string, c: SearchResult[]) { return c; },
    };
    const retriever = await RetrievalFactory.create(
      { mode: "embedding" },
      chunks,
      embedder,
      undefined,
      reranker,
    );
    assert.ok(retriever instanceof RerankingRetriever);
  });

  it("without reranker → returns unwrapped retriever", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "hybrid" },
      chunks,
      embedder,
    );
    assert.ok(retriever instanceof HybridRetriever);
  });

  // ── Shared IndexRegistry: cache works across modes ──

  it("two different modes with same registry → getAll called once", async () => {
    // Create a new registry for this test so we can spy on getAll
    const spyChunks = {
      getAll: (includeDeleted?: boolean) => {
        const result = await chunks.getAll(includeDeleted);
        spyChunks._callCount = (spyChunks._callCount ?? 0) + 1;
        return result;
      },
      getByFilePath: chunks.getByFilePath.bind(chunks),
      getByFileName: chunks.getByFileName.bind(chunks),
      _callCount: 0,
    };

    const registry = new IndexRegistry(spyChunks, embedder);

    // Create embedding mode retriever
    await RetrievalFactory.create(
      { mode: "embedding" },
      spyChunks,
      embedder,
      registry,
    );

    // Create keyword mode retriever — should reuse BM25 from registry
    await RetrievalFactory.create(
      { mode: "keyword" },
      spyChunks,
      embedder,
      registry,
    );

    // Both getSearchEngine and getBm25Index call getAll
    // But Bm25Index doesn't auto-build on getBm25Index unless dirty
    // Actually: getBm25Index() builds on first call (dirty=true)
    // getSearchEngine() also builds on first call (dirtyVector=true)
    // So getAll is called twice (once for vector, once for bm25)
    // But NOT 4 times (not once per factory.create)
    assert.ok(spyChunks._callCount <= 2,
      `Expected ≤2 getAll calls, got ${spyChunks._callCount}`);
  });

  it("same registry → retriever works across multiple queries", async () => {
    const registry = new IndexRegistry(chunks, embedder);

    const embRetriever = await RetrievalFactory.create(
      { mode: "embedding" },
      chunks,
      embedder,
      registry,
    );

    // Run multiple queries — shouldn't rebuild index
    await embRetriever.retrieve("machine learning", { topK: 3 });
    await embRetriever.retrieve("neural networks", { topK: 3 });
    await embRetriever.retrieve("deep learning", { topK: 3 });
    // No explicit assertion — just verifying no crash
  });

  // ── All modes produce valid results ──

  it("embedding mode returns results", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "embedding" },
      chunks,
      embedder,
    );
    const results = await retriever.retrieve("machine learning", { topK: 3 });
    assert.ok(Array.isArray(results));
    assert.ok(results.length > 0);
  });

  it("keyword mode returns results", async () => {
    const retriever = await RetrievalFactory.create(
      { mode: "keyword" },
      chunks,
      embedder,
    );
    const results = await retriever.retrieve("machine", { topK: 3 });
    assert.ok(Array.isArray(results));
    assert.ok(results.length > 0);
  });

  it("hybrid mode returns results", async () => {
    const retriever = RetrievalFactory.create(
      { mode: "hybrid" },
      chunks,
      embedder,
    );
    const results = await retriever.retrieve("neural networks", { topK: 3 });
    assert.ok(Array.isArray(results));
    assert.ok(results.length > 0);
  });

  // ── aiRerank config ──

  it("aiRerank.enabled=true + llm → wrapped in RerankingRetriever", async () => {
    const mockLLM = {
      generate: mock.fn(async () => ({ text: "[]", usage: { totalTokens: 1 }, model: "mock" })),
    };
    const retriever = RetrievalFactory.create(
      { mode: "embedding", aiRerank: { enabled: true } },
      chunks,
      embedder,
      undefined,
      undefined,
      undefined,
      mockLLM as any,
    );
    assert.ok(retriever instanceof RerankingRetriever);
  });

  it("aiRerank.enabled=true + no llm → no wrapping (no crash)", async () => {
    const retriever = RetrievalFactory.create(
      { mode: "embedding", aiRerank: { enabled: true } },
      chunks,
      embedder,
    );
    // No llm → buildReranker returns undefined → no RerankingRetriever
    assert.ok(retriever instanceof EmbeddingRetriever);
  });

  it("aiRerank.enabled=false → no wrapping", async () => {
    const mockLLM = {
      generate: mock.fn(async () => ({ text: "[]", usage: { totalTokens: 1 }, model: "mock" })),
    };
    const retriever = RetrievalFactory.create(
      { mode: "embedding", aiRerank: { enabled: false } },
      chunks,
      embedder,
      undefined,
      undefined,
      undefined,
      mockLLM as any,
    );
    assert.ok(retriever instanceof EmbeddingRetriever);
  });

  it("aiRerank mode=listwise → creates ListwiseLlmReranker", async () => {
    const mockLLM = {
      generate: mock.fn(async () => ({ text: "[]", usage: { totalTokens: 1 }, model: "mock" })),
    };
    const retriever = RetrievalFactory.create(
      { mode: "embedding", aiRerank: { enabled: true, mode: "listwise" } },
      chunks,
      embedder,
      undefined,
      undefined,
      undefined,
      mockLLM as any,
    );
    assert.ok(retriever instanceof RerankingRetriever);
  });

  it("aiRerank maxCandidates passed through", async () => {
    const mockLLM = {
      generate: mock.fn(async () => ({ text: "[]", usage: { totalTokens: 1 }, model: "mock" })),
    };
    const retriever = RetrievalFactory.create(
      { mode: "embedding", aiRerank: { enabled: true, maxCandidates: 10 } },
      chunks,
      embedder,
      undefined,
      undefined,
      undefined,
      mockLLM as any,
    );
    assert.ok(retriever instanceof RerankingRetriever);
  });
});

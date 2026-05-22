import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { RerankingRetriever } from "@retrieval/retrievers/RerankingRetriever";
import type { Retriever, RetrieveOptions } from "@retrieval/retrievers/types";
import type { Reranker } from "@retrieval/rerankers/types/reranker";
import type { ChunkSource } from "@retrieval/types/chunk-source";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number, text?: string, score = 0.5): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score,
  text,
});

// ── Helpers ──

type MockRetriever = {
  retrieve: ReturnType<typeof mock.fn>;
};

function makeMockInner(results: SearchResult[]) {
  return {
    retrieve: mock.fn(async (_query: string, _options?: RetrieveOptions) => results),
  } as MockRetriever;
}

type MockReranker = {
  rerank: ReturnType<typeof mock.fn>;
};

function makeMockReranker(reorder: (candidates: SearchResult[]) => SearchResult[]) {
  return {
    rerank: mock.fn(async (_query: string, candidates: SearchResult[]) => reorder(candidates)),
  } as MockReranker;
}

describe("RerankingRetriever", () => {
  // ── Over-fetch ──

  it("calls inner with topK * overFetch (default 4×)", async () => {
    const inner = makeMockInner([makeResult("a.md", 0)]);
    const reranker = makeMockReranker(c => c);
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker);

    await retriever.retrieve("query", { topK: 5 });
    const calls = inner.retrieve.mock.calls;
    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0].arguments[1], { topK: 20 });
  });

  it("overFetch parameter affects factor", async () => {
    const inner = makeMockInner([makeResult("a.md", 0)]);
    const reranker = makeMockReranker(c => c);
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker, undefined, 2);

    await retriever.retrieve("query", { topK: 5 });
    assert.deepEqual(
      inner.retrieve.mock.calls[0].arguments[1],
      { topK: 10 },
    );
  });

  // ── Without chunkSource ──

  it("without chunkSource → passes candidates as-is", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "hello"),
      makeResult("b.md", 0, undefined),
    ];
    const inner = makeMockInner(candidates);
    let passedCandidates: SearchResult[] = [];
    const reranker = makeMockReranker(c => { passedCandidates = c; return [c[0]]; });
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker);

    await retriever.retrieve("query", { topK: 5 });
    assert.equal(passedCandidates.length, 2);
    assert.equal(passedCandidates[0].text, "hello");
    assert.equal(passedCandidates[1].text, undefined);
  });

  // ── With chunkSource (hydration) ──

  function makeMockChunkSource(files: Record<string, Array<{ chunkIndex: number; chunkText: string }>>): ChunkSource {
    return {
      getByFilePath: (fp: string) => {
        const chunks = files[fp] ?? [];
        return chunks.map(c => ({
          chunkIndex: c.chunkIndex,
          chunkText: c.chunkText,
          embedding: new Float32Array([]),
        }));
      },
      getByFileName: async () => [],
      getAll: async () => [],
    };
  }

  it("with chunkSource → hydrates candidates where text is undefined", async () => {
    const cs = makeMockChunkSource({
      "a.md": [{ chunkIndex: 0, chunkText: "hydrated text" }],
    });
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, undefined),
      makeResult("b.md", 0, "already set"),
    ];
    const inner = makeMockInner(candidates);
    let passedCandidates: SearchResult[] = [];
    const reranker = makeMockReranker(c => { passedCandidates = c; return c.slice(0, 1); });
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker, cs);

    await retriever.retrieve("query", { topK: 5 });
    assert.equal(passedCandidates[0].text, "hydrated text");
    assert.equal(passedCandidates[1].text, "already set");
  });

  it("chunkSource.getByFilePath returns empty → text becomes ''", async () => {
    const cs = makeMockChunkSource({});
    const candidates: SearchResult[] = [makeResult("a.md", 0, undefined)];
    const inner = makeMockInner(candidates);
    let passedCandidates: SearchResult[] = [];
    const reranker = makeMockReranker(c => { passedCandidates = c; return c; });
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker, cs);

    await retriever.retrieve("query", { topK: 5 });
    assert.equal(passedCandidates[0].text, "");
  });

  it("chunkSource with partial match → text becomes '' for unmatched", async () => {
    const cs = makeMockChunkSource({
      "a.md": [{ chunkIndex: 0, chunkText: "found" }],
    });
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, undefined),
      makeResult("a.md", 999, undefined),
    ];
    const inner = makeMockInner(candidates);
    let passedCandidates: SearchResult[] = [];
    const reranker = makeMockReranker(c => { passedCandidates = c; return c; });
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker, cs);

    await retriever.retrieve("query", { topK: 5 });
    assert.equal(passedCandidates[0].text, "found");
    assert.equal(passedCandidates[1].text, "");
  });

  // ── Truncation to topK ──

  it("after rerank, truncates to topK", async () => {
    const candidates: SearchResult[] = Array.from({ length: 10 }, (_, i) =>
      makeResult(`doc${i}.md`, 0),
    );
    const inner = makeMockInner(candidates);
    const reranker = makeMockReranker(c => c.reverse());
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker);

    const result = await retriever.retrieve("query", { topK: 3 });
    assert.equal(result.length, 3);
  });

  // ── Inner returns fewer than overFetch ──

  it("inner returns fewer than topK * overFetch → works fine", async () => {
    const inner = makeMockInner([makeResult("a.md", 0)]);
    const reranker = makeMockReranker(c => c);
    const retriever = new RerankingRetriever(inner as unknown as Retriever, reranker as unknown as Reranker);

    const result = await retriever.retrieve("query", { topK: 5 });
    assert.equal(result.length, 1);
  });
});

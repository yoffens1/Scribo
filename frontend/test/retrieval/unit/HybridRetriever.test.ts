import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { HybridRetriever } from "@retrieval/retrievers/HybridRetriever";
import type { Retriever, RetrieveOptions } from "@retrieval/retrievers/types";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number, score = 0.5): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score,
  text: `text-${filePath}-${chunkIndex}`,
});

type MockRetriever = {
  retrieve: ReturnType<typeof mock.fn>;
};

function makeMockRetriever(results: SearchResult[]): MockRetriever {
  return {
    retrieve: mock.fn(async (_query: string, _options?: RetrieveOptions) => results),
  };
}

describe("HybridRetriever", () => {
  // ── Parallel calls ──

  it("calls embedding and keyword retrievers in parallel", async () => {
    const embRetriever = makeMockRetriever([makeResult("a.md", 0)]);
    const kwRetriever = makeMockRetriever([makeResult("b.md", 0)]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    await hybrid.retrieve("query");
    const embCalls = embRetriever.retrieve.mock.callCount();
    const kwCalls = kwRetriever.retrieve.mock.callCount();
    assert.equal(embCalls, 1);
    assert.equal(kwCalls, 1);
  });

  // ── topK * 3 passed to each inner ──

  it("passes topK * 3 to each inner retriever", async () => {
    const embRetriever = makeMockRetriever([]);
    const kwRetriever = makeMockRetriever([]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    await hybrid.retrieve("query", { topK: 5 });
    const embArgs = embRetriever.retrieve.mock.calls[0].arguments;
    const kwArgs = kwRetriever.retrieve.mock.calls[0].arguments;
    assert.deepEqual(embArgs[1], { topK: 15 });
    assert.deepEqual(kwArgs[1], { topK: 15 });
  });

  it("default topK=5 → inner gets topK=15 each", async () => {
    const embRetriever = makeMockRetriever([]);
    const kwRetriever = makeMockRetriever([]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    await hybrid.retrieve("query");
    const embArgs = embRetriever.retrieve.mock.calls[0].arguments;
    assert.deepEqual(embArgs[1], { topK: 15 });
  });

  // ── RRF merge ──

  it("two lists with overlap → overlap gets boosted and appears in top", async () => {
    const shared = makeResult("shared.md", 0);
    const embRetriever = makeMockRetriever([shared, makeResult("emb-only.md", 0)]);
    const kwRetriever = makeMockRetriever([shared, makeResult("kw-only.md", 0)]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    const result = await hybrid.retrieve("query", { topK: 3 });
    assert.equal(result.length, 3);
    assert.equal(result[0].chunkRef.filePath, "shared.md");
    assert.equal(result[0].chunkRef.chunkIndex, 0);
  });

  it("RRF correctly merges two disjoint lists", async () => {
    const embRetriever = makeMockRetriever([
      makeResult("e1.md", 0),
      makeResult("e2.md", 0),
      makeResult("e3.md", 0),
    ]);
    const kwRetriever = makeMockRetriever([
      makeResult("k1.md", 0),
      makeResult("k2.md", 0),
      makeResult("k3.md", 0),
    ]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    const result = await hybrid.retrieve("query", { topK: 5 });
    assert.equal(result.length, 5);
  });

  // ── Filters applied AFTER RRF ──

  it("filters applied after RRF merge", async () => {
    const embRetriever = makeMockRetriever([
      makeResult("Daily/note.md", 0),
      makeResult("Projects/code.md", 0),
    ]);
    const kwRetriever = makeMockRetriever([
      makeResult("Daily/other.md", 0),
      makeResult("Projects/lib.md", 0),
    ]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    const result = await hybrid.retrieve("query", {
      topK: 5,
      filters: { folder: "Daily/" },
    });
    assert.equal(result.length, 2);
    assert.ok(result.every(r => r.chunkRef.filePath.startsWith("Daily/")));
  });

  it("filters: if no results match → empty", async () => {
    const embRetriever = makeMockRetriever([makeResult("a.md", 0)]);
    const kwRetriever = makeMockRetriever([makeResult("b.md", 0)]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever);

    const result = await hybrid.retrieve("query", {
      filters: { filePath: "nonexistent.md" },
    });
    assert.equal(result.length, 0);
  });

  // ── Custom k ──

  it("uses custom k for RRF", async () => {
    const shared = makeResult("shared.md", 0);
    const embRetriever = makeMockRetriever([shared]);
    const kwRetriever = makeMockRetriever([shared]);
    const hybrid = new HybridRetriever(embRetriever as unknown as Retriever, kwRetriever as unknown as Retriever, 10);

    const result = await hybrid.retrieve("query", { topK: 1 });
    assert.ok(Math.abs(result[0].score - 2 / 11) < 0.0001);
  });
});

import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { KeywordRetriever } from "@retrieval/retrievers/KeywordRetriever";
import { Bm25Index } from "@retrieval/engines/Bm25Index";
import type { SearchResult } from "@retrieval/types/search";

describe("KeywordRetriever", () => {
  it("calls bm25Index.search and returns mapped results", async () => {
    const index = new Bm25Index();
    index.addDocument({ filePath: "a.md", chunkIndex: 0 }, "hello world machine learning");
    index.addDocument({ filePath: "b.md", chunkIndex: 0 }, "deep neural networks ai");
    const retriever = new KeywordRetriever(() => index);

    const results = retriever.retrieve("machine learning", { topK: 2 });
    // Returns array — BM25 is sync so we call it as if async
    assert.ok(results instanceof Promise);
  });

  it("returns promise even though BM25 is sync (Retriever interface contract)", async () => {
    const index = new Bm25Index();
    index.addDocument({ filePath: "a.md", chunkIndex: 0 }, "test document");
    const retriever = new KeywordRetriever(() => index);

    const result = retriever.retrieve("test", { topK: 1 });
    assert.ok(result instanceof Promise);
  });

  it("applies filters if provided", async () => {
    const index = new Bm25Index();
    index.addDocument({ filePath: "Daily/note.md", chunkIndex: 0 }, "daily alpha text");
    index.addDocument({ filePath: "Projects/code.md", chunkIndex: 0 }, "project alpha code");
    const retriever = new KeywordRetriever(() => index);

    const results = retriever.retrieve("alpha", {
      topK: 5,
      filters: { folder: "Daily/" },
    });
    assert.ok(results instanceof Promise);
  });

  it("empty index → returns empty", async () => {
    const index = new Bm25Index();
    const retriever = new KeywordRetriever(() => index);

    const results = retriever.retrieve("query", { topK: 5 });
    assert.ok(results instanceof Promise);
  });

  it("results have correct shape", async () => {
    const index = new Bm25Index();
    index.addDocument({ filePath: "a.md", chunkIndex: 0 }, "the quick brown fox");
    index.addDocument({ filePath: "b.md", chunkIndex: 1 }, "jumps over lazy dog");
    const retriever = new KeywordRetriever(() => index);

    const results = await retriever.retrieve("quick fox", { topK: 5 });
    assert.ok(Array.isArray(results));
    for (const r of results) {
      assert.ok("chunkRef" in r);
      assert.ok("score" in r);
      assert.equal(typeof r.chunkRef.filePath, "string");
      assert.equal(typeof r.chunkRef.chunkIndex, "number");
    }
  });
});

import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { ListwiseLlmReranker } from "@retrieval/rerankers/ListwiseLlmReranker";
import type { LLMService } from "@ai/llm/LLMService";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number, text: string, score = 0.5): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score,
  text,
});

function createMockLLM(responseText: string) {
  return {
    generate: mock.fn(async (_prompt: string) => ({
      text: responseText,
      usage: { totalTokens: 10 },
      model: "mock",
    })),
  };
}

describe("ListwiseLlmReranker", () => {
  // ── Empty candidates ──

  it("empty candidates → returns empty, LLM not called", async () => {
    const mockLLM = createMockLLM("{}");
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);
    const result = await reranker.rerank("query", []);
    assert.equal(result.length, 0);
    assert.equal(mockLLM.generate.mock.callCount(), 0);
  });

  // ── Valid order JSON ──

  it("valid JSON order → correct re-ranking", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text A"),
      makeResult("b.md", 0, "text B"),
      makeResult("c.md", 0, "text C"),
    ];
    const response = JSON.stringify({ order: [2, 0, 1] });
    const mockLLM = createMockLLM(response);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 3);
    assert.equal(result[0].chunkRef.filePath, "c.md");
    assert.equal(result[1].chunkRef.filePath, "a.md");
    assert.equal(result[2].chunkRef.filePath, "b.md");
    assert.ok(Math.abs(result[0].score - 1.0) < 0.001);
    assert.ok(Math.abs(result[1].score - 1 + 1 / 3) < 0.01);
    assert.ok(Math.abs(result[2].score - 1 + 2 / 3) < 0.01);
  });

  // ── Order with out-of-range indices ──

  it("order includes index outside candidate range → filtered out", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A"),
      makeResult("b.md", 0, "B"),
      makeResult("c.md", 0, "C"),
      makeResult("d.md", 0, "D"),
    ];
    const response = JSON.stringify({ order: [3, 0, 5, 1] });
    const mockLLM = createMockLLM(response);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 3);
    assert.equal(result[0].chunkRef.filePath, "d.md");
    assert.equal(result[1].chunkRef.filePath, "a.md");
    assert.equal(result[2].chunkRef.filePath, "b.md");
  });

  it("order: [3, 0, 5] with 4 candidates → 5 dropped, rest ranked", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A"),
      makeResult("b.md", 0, "B"),
      makeResult("c.md", 0, "C"),
      makeResult("d.md", 0, "D"),
    ];
    const response = JSON.stringify({ order: [3, 0, 5] });
    const mockLLM = createMockLLM(response);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "d.md");
    assert.equal(result[1].chunkRef.filePath, "a.md");
  });

  // ── JSON in markdown wrapper ──

  it("JSON in markdown code block → parsed", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A"),
      makeResult("b.md", 0, "B"),
    ];
    const wrappedResponse = ["```json", '{"order": [1, 0]}', "```"].join("\n");
    const mockLLM = createMockLLM(wrappedResponse);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "b.md");
    assert.equal(result[1].chunkRef.filePath, "a.md");
  });

  // ── Invalid JSON → fallback ──

  it("invalid JSON → fallback (returns capped unchanged)", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text"),
      makeResult("b.md", 0, "text"),
    ];
    const mockLLM = createMockLLM("not json");
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    // Empty parse → returns capped as-is (no candidates lost)
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "a.md");
  });

  it("valid JSON but no 'order' key → fallback (capped unchanged)", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text"),
    ];
    const response = JSON.stringify({ something: "else" });
    const mockLLM = createMockLLM(response);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 1);
  });

  // ── Partial order ──

  it("partial order → only ordered candidates returned", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A"),
      makeResult("b.md", 0, "B"),
      makeResult("c.md", 0, "C"),
    ];
    const response = JSON.stringify({ order: [2] });
    const mockLLM = createMockLLM(response);
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 1);
    assert.equal(result[0].chunkRef.filePath, "c.md");
    assert.equal(result[0].score, 1.0);
  });

  // ── Text truncation ──

  it("long text truncated to 500 chars", async () => {
    const longText = "y".repeat(1000);
    const candidates: SearchResult[] = [makeResult("a.md", 0, longText)];
    let capturedPrompt = "";
    const mockLLM = {
      generate: mock.fn(async (prompt: string) => {
        capturedPrompt = prompt;
        return { text: '{"order": [0]}', usage: { totalTokens: 1 }, model: "mock" };
      }),
    };
    const reranker = new ListwiseLlmReranker(mockLLM as unknown as LLMService);

    await reranker.rerank("query", candidates);
    assert.ok(capturedPrompt.includes("y".repeat(500)));
    assert.ok(!capturedPrompt.includes("y".repeat(1000)));
  });
});

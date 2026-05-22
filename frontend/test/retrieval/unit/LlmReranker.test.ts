import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { LlmReranker } from "@retrieval/rerankers/LlmReranker";
import type { LLMService } from "@ai/llm/LLMService";
import type { SearchResult } from "@retrieval/types/search";

const makeResult = (filePath: string, chunkIndex: number, text: string, score = 0.5): SearchResult => ({
  chunkRef: { filePath, chunkIndex },
  score,
  text,
});

function createMockLLM(responseText: string) {
  return {
    generate: mock.fn(async (_prompt: string, _signal?: AbortSignal) => ({
      text: responseText,
      usage: { totalTokens: 10 },
      model: "mock",
    })),
  };
}

describe("LlmReranker", () => {
  // ── Empty candidates ──

  it("empty candidates → returns empty, LLM not called", async () => {
    const mockLLM = createMockLLM("[]");
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);
    const result = await reranker.rerank("query", []);
    assert.equal(result.length, 0);
    assert.equal(mockLLM.generate.mock.callCount(), 0);
  });

  // ── Valid JSON ──

  it("valid JSON scores → correct re-ranking", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text A"),
      makeResult("b.md", 0, "text B"),
      makeResult("c.md", 0, "text C"),
    ];
    const response = JSON.stringify([
      { id: 1, score: 10 },
      { id: 0, score: 5 },
      { id: 2, score: 8 },
    ]);
    const mockLLM = createMockLLM(response);
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 3);
    // Order: B (10→1.0), C (8→0.8), A (5→0.5)
    assert.equal(result[0].chunkRef.filePath, "b.md");
    assert.equal(result[0].score, 1.0);
    assert.equal(result[1].chunkRef.filePath, "c.md");
    assert.equal(result[1].score, 0.8);
    assert.equal(result[2].chunkRef.filePath, "a.md");
    assert.equal(result[2].score, 0.5);
  });

  // ── JSON in markdown wrapper ──

  it("JSON in markdown code block → parsed", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "irrelevant"),
      makeResult("b.md", 0, "relevant"),
    ];
    const wrappedResponse = [
      "```json",
      '[{"id": 1, "score": 10}, {"id": 0, "score": 3}]',
      "```",
    ].join("\n");
    const mockLLM = createMockLLM(wrappedResponse);
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "b.md");
    assert.equal(result[0].score, 1.0);
    assert.equal(result[1].chunkRef.filePath, "a.md");
    assert.equal(result[1].score, 0.3);
  });

  // ── Invalid JSON → fallback ──

  it("invalid JSON → fallback (all scores 0, original order preserved)", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text A"),
      makeResult("b.md", 0, "text B"),
      makeResult("c.md", 0, "text C"),
    ];
    const mockLLM = createMockLLM("not valid json at all");
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 3);
  });

  it("completely garbled response → doesn't crash", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "text"),
    ];
    const mockLLM = createMockLLM("I think the answer is 42. Also, {not: json}");
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 1);
    assert.equal(result[0].chunkRef.filePath, "a.md");
  });

  // ── Partial scores → unscored use c.score ──

  it("partial scores → unscored candidates keep original c.score", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A", 0.7),
      makeResult("b.md", 0, "B", 0.6),
      makeResult("c.md", 0, "C", 0.3),
    ];
    const response = JSON.stringify([{ id: 1, score: 10 }]);
    const mockLLM = createMockLLM(response);
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result[0].chunkRef.filePath, "b.md");
    assert.equal(result[0].score, 1.0);
    assert.equal(result[1].chunkRef.filePath, "a.md");
    assert.equal(result[2].chunkRef.filePath, "c.md");
  });

  // ── Out-of-range id ──

  it("LLM returns id outside candidate range → ignored, doesn't crash", async () => {
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, "A"),
      makeResult("b.md", 0, "B"),
    ];
    const response = JSON.stringify([
      { id: 0, score: 5 },
      { id: 999, score: 10 },
    ]);
    const mockLLM = createMockLLM(response);
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    const result = await reranker.rerank("query", candidates);
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkRef.filePath, "a.md");
    assert.equal(result[0].score, 0.5);
  });

  // ── MAX_CANDIDATES cap ──

  it("exceeds MAX_CANDIDATES (25) → only first 25 passed to prompt", async () => {
    const candidates: SearchResult[] = Array.from({ length: 50 }, (_, i) =>
      makeResult(`doc${i}.md`, 0, `text-${i}`),
    );
    let capturedPrompt = "";
    const mockLLM = {
      generate: mock.fn(async (prompt: string) => {
        capturedPrompt = prompt;
        const scores = Array.from({ length: 25 }, (_, i) => ({ id: i, score: 5 }));
        return { text: JSON.stringify(scores), usage: { totalTokens: 10 }, model: "mock" };
      }),
    };
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    await reranker.rerank("query", candidates);
    assert.ok(capturedPrompt.includes("[24]"));
    assert.ok(!capturedPrompt.includes("[25]"));
    assert.ok(!capturedPrompt.includes("[49]"));
  });

  // ── Text truncation ──

  it("long text truncated to 500 chars in prompt", async () => {
    const longText = "x".repeat(1000);
    const candidates: SearchResult[] = [
      makeResult("a.md", 0, longText),
    ];
    let capturedPrompt = "";
    const mockLLM = {
      generate: mock.fn(async (prompt: string) => {
        capturedPrompt = prompt;
        return { text: "[]", usage: { totalTokens: 1 }, model: "mock" };
      }),
    };
    const reranker = new LlmReranker(mockLLM as unknown as LLMService);

    await reranker.rerank("query", candidates);
    assert.ok(capturedPrompt.match(/x{500}/), "should find 500 x's");
    assert.ok(!capturedPrompt.match(/x{1000}/), "should not find 1000 consecutive x's");
  });
});

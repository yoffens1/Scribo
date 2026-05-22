// src/core/retrieval/rerankers/LlmReranker.ts
import type { Reranker } from "./types/reranker";
import type { SearchResult } from "../types/search";
import type { LLMService } from "@ai/llm/LLMService";
import { RetrievalLogger } from "../logging/RetrievalLogger";
import { buildRerankScoringPrompt } from "@ai/prompts";
import { extractJsonArray } from "../utils/jsonExtract";

/**
 * LLM-based reranker.
 *
 * Sends a numbered list of candidate passages to an LLM with a scoring
 * prompt. Returns candidates re-ranked by LLM-assigned relevance scores.
 *
 * Model choice: use a cheap model (gpt-4o-mini, llama-3.1-8b) — reranking
 * does not require reasoning.
 */
export class LlmReranker implements Reranker {
  private maxCandidates: number;

  constructor(private llm: LLMService, maxCandidates = 25, private logger?: RetrievalLogger) {
    this.maxCandidates = maxCandidates;
  }

  async rerank(
    query: string,
    candidates: SearchResult[],
  ): Promise<SearchResult[]> {
    if (candidates.length === 0) return candidates;

    const capped = candidates.slice(0, this.maxCandidates);

    const prompt = buildRerankScoringPrompt(query, candidates, this.maxCandidates);

    this.logger?.log("debug", "rerank.llm.prompt", "generated", {
      candidateCount: capped.length,
      promptLength: prompt.length,
    });

    const response = await this.llm.generate(prompt);
    const scores = this.parseScores(response.text);

    this.logger?.log("debug", "rerank.llm.response", "parsed", {
      rawPrefix: response.text.slice(0, 300),
      parsedScores: scores.slice(0, 10),
      scoredCount: scores.length,
    });

    // Map scores back to candidates, preserving original order for unscored
    const scoreMap = new Map<number, number>();
    for (const s of scores) scoreMap.set(s.id, s.score);

    return capped
      .map((c, i) => ({
        ...c,
        score: scoreMap.has(i) ? scoreMap.get(i)! / 10 : c.score,
      }))
      .sort((a, b) => b.score - a.score);
  }

  private parseScores(text: string): Array<{ id: number; score: number }> {
    const json = extractJsonArray(text);
    if (!json) {
      console.warn("[LlmReranker] no valid JSON array found in response", text.slice(0, 200));
      return [];
    }
    try {
      return JSON.parse(json);
    } catch {
      this.logger?.log("warn", "rerank.llm.parse", "failed to parse scores", { json: json.slice(0, 200) });
      return [];
    }
  }
}

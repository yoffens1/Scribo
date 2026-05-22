// src/core/retrieval/rerankers/ListwiseLlmReranker.ts
import type { Reranker } from "./types/reranker";
import type { SearchResult } from "../types/search";
import type { LLMService } from "@ai/llm/LLMService";
import { RetrievalLogger } from "../logging/RetrievalLogger";
import { buildRerankListwisePrompt } from "@ai/prompts";
import { extractJsonObject } from "../utils/jsonExtract";

/**
 * Listwise LLM reranker — asks the model to ORDER candidates by relevance
 * instead of scoring each independently (less noisy than 0-10 scoring).
 */
export class ListwiseLlmReranker implements Reranker {
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

    const prompt = buildRerankListwisePrompt(query, candidates, this.maxCandidates);

    this.logger?.log("debug", "rerank.listwise.prompt", "generated", {
      candidateCount: capped.length,
      promptLength: prompt.length,
    });

    const response = await this.llm.generate(prompt);
    const order = this.parseOrder(response.text);

    this.logger?.log("debug", "rerank.listwise.response", "parsed", {
      rawPrefix: response.text.slice(0, 300),
      order,
    });

    // Fallback: if LLM returned empty/garbled order, return capped as-is.
    // Otherwise all candidates would silently disappear.
    if (order.length === 0) {
      console.warn("[ListwiseLlmReranker] empty order, returning original capped");
      return capped;
    }

    return order
      .filter((i) => i < capped.length)
      .map((i, rank) => ({
        ...capped[i],
        score: 1 - rank / Math.max(order.length, 1),
      }));
  }

  private parseOrder(text: string): number[] {
    const json = extractJsonObject(text);
    if (!json) return [];
    try {
      const obj = JSON.parse(json);
      return Array.isArray(obj.order) ? obj.order : [];
    } catch {
      return [];
    }
  }
}

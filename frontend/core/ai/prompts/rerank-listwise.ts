// src/core/ai/prompts/rerank-listwise.ts
import type { SearchResult } from "@retrieval/types/search";

export function buildRerankListwisePrompt(query: string, candidates: SearchResult[], maxCandidates: number): string {
  const capped = candidates.slice(0, maxCandidates);
  const numbered = capped
    .map((c, i) => `[${i}] ${(c.text ?? "").slice(0, 500)}`)
    .join("\n\n");

  return [
    `Sort the following passages by relevance to the query.`,
    ``,
    `Query: ${query}`,
    ``,
    `Passages:`,
    numbered,
    ``,
    `Return ONLY a JSON object: {"order": [3, 0, 5, 1, ...]}. Passages not in output are assumed irrelevant.`,
  ].join("\n");
}

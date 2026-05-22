// src/core/ai/prompts/rerank-scoring.ts
import type { SearchResult } from "@retrieval/types/search";

export function buildRerankScoringPrompt(query: string, candidates: SearchResult[], maxCandidates: number): string {
  const capped = candidates.slice(0, maxCandidates);
  const numbered = capped
    .map((c, i) => `[${i}] ${(c.text ?? "").slice(0, 500)}`)
    .join("\n\n");

  return [
    `Rate relevance of each passage to the query on a scale 0–10.`,
    ``,
    `Query: ${query}`,
    ``,
    `Passages:`,
    numbered,
    ``,
    `Return ONLY a JSON array: [{"id": 0, "score": 7.5}, ...]. No explanation.`,
  ].join("\n");
}

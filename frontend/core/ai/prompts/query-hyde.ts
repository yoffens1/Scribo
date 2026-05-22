// src/core/ai/prompts/query-hyde.ts

export function buildHydePrompt(query: string, lang: string): string {
  return [
    `Write a concise, factual answer to the following query in ${lang}.`,
    `Be informative and specific — the answer will be used to find related documents.`,
    ``,
    `Query: ${query}`,
  ].join("\n");
}

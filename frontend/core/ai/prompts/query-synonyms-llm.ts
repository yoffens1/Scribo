// src/core/ai/prompts/query-synonyms-llm.ts

export function buildSynonymExpansionPrompt(original: string, maxSynonyms: number, lang: string): string {
  return [
    `Generate ${maxSynonyms} alternative search queries that express the same information need.`,
    `Use natural language in ${lang}.`,
    `Return ONLY a JSON object: {"synonyms": ["query1", "query2", ...]}.`,
    ``,
    `Original query: ${original}`,
  ].join("\n");
}

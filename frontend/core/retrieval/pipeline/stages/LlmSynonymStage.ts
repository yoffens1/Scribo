// src/core/retrieval/pipeline/stages/LlmSynonymStage.ts
import type { QueryStage, ExpandedQuery, QueryVariant } from "../types";
import type { LLMService } from "@ai/llm/LLMService";
import { buildSynonymExpansionPrompt } from "@ai/prompts";

/**
 * LLM-based synonym expansion stage.
 *
 * Asks the LLM to generate alternative search queries for the same intent.
 * More flexible than the static dictionary, but costs LLM tokens per query.
 *
 * Use when the user has many domain-specific terms not covered by the
 * static dictionary.
 */
export class LlmSynonymStage implements QueryStage {
  constructor(
    private llm: LLMService,
    private maxSynonyms = 3,
  ) {}

  async process(input: ExpandedQuery): Promise<ExpandedQuery> {
    const lang = input.vaultLang ?? input.detectedLang ?? "en";
    const prompt = buildSynonymExpansionPrompt(input.original, this.maxSynonyms, lang);

    try {
      const response = await this.llm.generate(prompt);
      const synonyms = this.parseSynonyms(response.text);
      if (synonyms.length === 0) return input;

      const existingTexts = new Set(input.variants.map(v => v.text.toLowerCase().trim()));
      const newVariants: QueryVariant[] = [];

      for (const syn of synonyms) {
        const lower = syn.toLowerCase().trim();
        // Skip duplicates
        if (existingTexts.has(lower)) continue;
        // Skip single-word synonyms with < 6 chars (mostly noise: "Core", "Ядро")
        if (syn.split(/\s+/).length === 1 && syn.length < 6) continue;
        // Skip if same as original
        if (lower === input.original.toLowerCase().trim()) continue;

        const wordCount = syn.split(/\s+/).length;
        const weight = wordCount === 1 ? 0.4 : 0.6;

        newVariants.push({
          text: syn,
          lang,
          source: "synonym",
          weight,
        });
      }

      if (newVariants.length === 0) return input;

      return {
        ...input,
        variants: [...input.variants, ...newVariants],
      };
    } catch (err) {
      console.warn("[LlmSynonymStage] expansion failed", err);
      return input;
    }
  }

  private parseSynonyms(text: string): string[] {
    try {
      const match = text.match(/\{[\s\S]*"synonyms"[\s\S]*\}/);
      if (!match) return [];
      const obj = JSON.parse(match[0]);
      return Array.isArray(obj.synonyms) ? obj.synonyms : [];
    } catch {
      return [];
    }
  }
}

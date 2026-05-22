// src/core/retrieval/pipeline/stages/SynonymExpansionStage.ts
import type { QueryStage, ExpandedQuery, QueryVariant } from "../types";

/**
 * Expands query variants with synonyms from a static dictionary.
 * Fast, free, no LLM calls — but coverage is limited to the dictionary.
 *
 * Dictionary format: lowercase term → array of synonyms.
 *
 * For LLM-based expansion, see LlmSynonymStage.
 */
export class SynonymExpansionStage implements QueryStage {
  constructor(private dict: Record<string, string[]>) {}

  async process(input: ExpandedQuery): Promise<ExpandedQuery> {
    const newVariants: QueryVariant[] = [];

    for (const v of input.variants) {
      const lower = v.text.toLowerCase();
      const synonyms = this.dict[lower];
      if (!synonyms || synonyms.length === 0) continue;

      for (const syn of synonyms) {
        // Avoid adding duplicates
        const already = input.variants.some(existing => existing.text === syn) ||
          newVariants.some(nv => nv.text === syn);
        if (already) continue;

        newVariants.push({
          text: syn,
          lang: v.lang,
          source: "synonym" as const,
          weight: 0.6,
        });
      }
    }

    if (newVariants.length === 0) return input;

    return {
      ...input,
      variants: [...input.variants, ...newVariants],
    };
  }
}

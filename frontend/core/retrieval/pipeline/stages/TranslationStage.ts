// src/core/retrieval/pipeline/stages/TranslationStage.ts
import type { QueryStage, ExpandedQuery } from "../types";
import type { Translator } from "@translation/Translator";
import type { VaultLanguageStats } from "../VaultLanguageStats";

/**
 * If the query language differs from the vault's dominant language,
 * adds a translated variant to the expanded query.
 *
 * Errors in translation are caught — the pipeline always continues
 * with the original query even if the translator fails.
 */
export class TranslationStage implements QueryStage {
  constructor(
    private translator: Translator,
    private vaultLangProvider: () => Promise<string>,
  ) {}

  async process(input: ExpandedQuery): Promise<ExpandedQuery> {
    const vaultLang = await this.vaultLangProvider();

    if (!input.detectedLang || input.detectedLang === vaultLang) {
      return { ...input, vaultLang };
    }

    try {
      const translated = await this.translator.translate(
        input.original,
        input.detectedLang,
        vaultLang,
      );

      return {
        ...input,
        vaultLang,
        variants: [
          ...input.variants,
          {
            text: translated,
            lang: vaultLang,
            source: "translated",
            weight: 1.0,
          },
        ],
      };
    } catch (err) {
      console.warn("[TranslationStage] translation failed, continuing with original", err);
      return { ...input, vaultLang };
    }
  }
}

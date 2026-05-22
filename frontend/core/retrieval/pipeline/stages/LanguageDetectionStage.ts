// src/core/retrieval/pipeline/stages/LanguageDetectionStage.ts
import type { QueryStage, ExpandedQuery, LanguageDetector } from "../types";
import { EldrLanguageDetector } from "../../utils/EldrLanguageDetector";

export class LanguageDetectionStage implements QueryStage {
  constructor(private detector: LanguageDetector = new EldrLanguageDetector()) {}

  async process(input: ExpandedQuery): Promise<ExpandedQuery> {
    const lang = await this.detector.detect(input.original);

    return {
      ...input,
      detectedLang: lang,
      variants: input.variants.map(v => ({
        ...v,
        lang: v.lang ?? lang,
      })),
    };
  }
}

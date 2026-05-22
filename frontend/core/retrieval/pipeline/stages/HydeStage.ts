// src/core/retrieval/pipeline/stages/HydeStage.ts
import type { QueryStage, ExpandedQuery, LanguageDetector } from "../types";
import { EldrLanguageDetector } from "../../utils/EldrLanguageDetector";
import type { LLMService } from "@ai/llm/LLMService";
import { buildHydePrompt } from "@ai/prompts";

const MIN_HYDE_LENGTH = 50;

/**
 * HyDE (Hypothetical Document Embeddings) stage.
 *
 * Generates a short factual answer to the query, then validates:
 * - Output must be in the expected language (via eldr detection)
 * - Output must be at least MIN_HYDE_LENGTH chars
 *
 * Small models (llama3.2:3b) often produce garbage — validation prevents
 * low-quality HyDE variants from polluting the pipeline.
 */
export class HydeStage implements QueryStage {
  constructor(
    private llm: LLMService,
    private detector: LanguageDetector = new EldrLanguageDetector(),
  ) {}

  async process(input: ExpandedQuery): Promise<ExpandedQuery> {
    const lang = input.vaultLang ?? input.detectedLang ?? "en";
    const prompt = buildHydePrompt(input.original, lang);

    try {
      const response = await this.llm.generate(prompt);
      const hydeText = response.text.trim();
      if (!hydeText || hydeText.length < MIN_HYDE_LENGTH) {
        console.warn(`[HydeStage] output too short (${hydeText.length} chars), skipping`);
        return input;
      }

      // Validate language
      const detected = await this.detector.detect(hydeText);
      if (detected !== lang) {
        console.warn(`[HydeStage] language mismatch: expected ${lang}, got ${detected}, skipping`);
        return input;
      }

      return {
        ...input,
        variants: [
          ...input.variants,
          {
            text: hydeText.slice(0, 1000),
            lang,
            source: "hyde",
            weight: 0.8,
          },
        ],
      };
    } catch (err) {
      console.warn("[HydeStage] LLM generation failed, continuing without HyDE", err);
      return input;
    }
  }
}

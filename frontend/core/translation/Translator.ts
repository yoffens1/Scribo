// src/core/translation/Translator.ts
import { getEldr } from "@retrieval/utils/eldr";
import { LLMService } from "@ai/llm/LLMService";
import { buildTranslatePrompt, buildTranslateStrictPrompt } from "@ai/prompts";

const LATIN_RATIO_THRESHOLD = 0.3;

export class Translator {
  private provider: LLMService;
  private targetLang: string;
  private sourceLang?: string;
  private cache = new Map<string, string>();
  private maxCacheSize = 200;
  

  constructor(provider: LLMService, targetLang: string, sourceLang?: string) {
    this.provider = provider;
    this.targetLang = targetLang;
    this.sourceLang = sourceLang;
  }

  async translate(text: string, sourceLang?: string, targetLang?: string): Promise<string> {
    const src = sourceLang ?? this.sourceLang ?? await this.detectLanguage(text);
    const tgt = targetLang ?? this.targetLang;
    const cacheKey = `${src}|${tgt}|${text}`;

    const cached = this.cache.get(cacheKey);
    if (cached !== undefined) return cached;

    // First attempt
    const result = await this.doTranslate(text, src, tgt);

    // Validate: check if output has too much Latin script (wrong direction/non-translated)
    if (this.latinRatio(result) > LATIN_RATIO_THRESHOLD && tgt === "ru") {
      // Retry with stricter prompt
      const strict = await this.doTranslateStrict(text, src, tgt);
      if (this.latinRatio(strict) < this.latinRatio(result)) {
        this.cacheSet(cacheKey, strict);
        return strict;
      }
    }

    this.cacheSet(cacheKey, result);
    return result;
  }

  private async doTranslate(text: string, src: string, tgt: string): Promise<string> {
    const prompt = buildTranslatePrompt(text, tgt);
    const response = await this.provider.generate(prompt);
    return this.cleanResponse(response.text);
  }

  private async doTranslateStrict(text: string, src: string, tgt: string): Promise<string> {
    const prompt = buildTranslateStrictPrompt(text, tgt);
    const response = await this.provider.generate(prompt);
    return this.cleanResponse(response.text);
  }

  private cleanResponse(text: string): string {
    return text
      .trim()
      // Remove leading/trailing quotes
      .replace(/^["'`«„]|["'`»“]$/g, "")
      // Remove common LLM prefix patterns
      .replace(/^(Translation|Перевод|Translated|Sure,?\s*here\s*(is|you go)|Here'?s?\s*(is|the|your)\s*(translation|answer))/i, "")
      .replace(/^[:：]\s*/u, "")
      .trim();
  }

  /** Fraction of Latin-script characters in the text. */
  private latinRatio(text: string): number {
    if (text.length === 0) return 0;
    let latin = 0;
    for (const ch of text) {
      if ((ch >= "a" && ch <= "z") || (ch >= "A" && ch <= "Z")) latin++;
    }
    return latin / text.length;
  }

  private cacheSet(key: string, val: string): void {
    if (this.cache.size >= this.maxCacheSize) {
      const first = this.cache.keys().next().value!;
      this.cache.delete(first);
    }
    this.cache.set(key, val);
  }

  private async detectLanguage(text: string): Promise<string> {
    const { eldr } = await getEldr();
    return eldr.detect(text).iso639_1;
  }
}

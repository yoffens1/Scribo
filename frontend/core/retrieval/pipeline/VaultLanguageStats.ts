// src/core/retrieval/pipeline/VaultLanguageStats.ts
import type { ChunkSource } from "../types/chunk-source";
import type { LanguageDetector } from "./types";
import { EldrLanguageDetector } from "../utils/EldrLanguageDetector";
import { RETRIEVAL_CONSTANTS } from "../constants";

export class VaultLanguageStats {
  private cached: { lang: string; computedAt: number } | null = null;

  constructor(
    private chunkSource: ChunkSource,
    private detector: LanguageDetector = new EldrLanguageDetector(),
  ) {}

  async getLanguage(): Promise<string> {
    if (this.cached && Date.now() - this.cached.computedAt < RETRIEVAL_CONSTANTS.VAULT_LANG_CACHE_TTL_MS) {
      return this.cached.lang;
    }

    const allChunks = await this.chunkSource.getAll(false);
    const sample = allChunks.slice(0, 50);
    if (sample.length === 0) {
      this.cached = { lang: "en", computedAt: Date.now() };
      return "en";
    }

    const counts = new Map<string, number>();
    for (const chunk of sample) {
      const text = (chunk.chunkText ?? "").trim();
      if (text.length < 10) continue;
      const lang = await this.detector.detect(text);
      counts.set(lang, (counts.get(lang) ?? 0) + 1);
    }

    let best = "en";
    let max = 0;
    for (const [l, c] of counts) {
      if (c > max) { max = c; best = l; }
    }

    this.cached = { lang: best, computedAt: Date.now() };
    return best;
  }

  invalidate(): void { this.cached = null; }
}

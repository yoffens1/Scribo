// src/core/retrieval/utils/eldr.ts

interface EldrModule {
  eldr: { detect(text: string): { iso639_1: string } };
}

let cached: EldrModule | null = null;

/** Lazy-load eldr once — shared across LanguageDetectionStage, HydeStage, VaultLanguageStats. */
export async function getEldr(): Promise<EldrModule> {
  if (!cached) {
    cached = await import("eldr") as unknown as EldrModule;
  }
  return cached;
}

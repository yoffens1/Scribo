// src/core/retrieval/pipeline/types/query-variant.ts

export interface QueryVariant {
  text: string;
  lang?: string;        // ISO-639-1 code, "en" / "ru" / etc.
  source: "original" | "translated" | "synonym" | "hyde";
  weight: number;       // 1.0 for original, 0.6-0.8 for expansions
}

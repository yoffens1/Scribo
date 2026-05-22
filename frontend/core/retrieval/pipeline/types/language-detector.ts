// src/core/retrieval/pipeline/types/language-detector.ts

export interface LanguageDetector {
  detect(text: string): Promise<string>;  // returns ISO-639-1
}

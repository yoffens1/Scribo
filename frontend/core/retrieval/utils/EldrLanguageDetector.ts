// src/core/retrieval/utils/EldrLanguageDetector.ts
import type { LanguageDetector } from "../pipeline/types/language-detector";
import { getEldr } from "./eldr";

/** Default implementation using eldr library. In tests, swap with a mock. */
export class EldrLanguageDetector implements LanguageDetector {
  async detect(text: string): Promise<string> {
    const { eldr } = await getEldr();
    return eldr.detect(text).iso639_1;
  }
}

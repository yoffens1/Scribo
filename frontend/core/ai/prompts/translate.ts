// src/core/ai/prompts/translate.ts

export function buildTranslatePrompt(text: string, tgt: string): string {
  return [
    `You are a translator. Translate the user text to ${tgt}.`,
    `Rules:`,
    `- Translate EVERY word. Do NOT leave any English words.`,
    `- Output ONLY the translated text. No quotes, no explanations, no prefix.`,
    `- Preserve question marks and punctuation.`,
    ``,
    `Text: ${text}`,
  ].join("\n");
}

export function buildTranslateStrictPrompt(text: string, tgt: string): string {
  return [
    `Translate to ${tgt}. Use ONLY ${tgt} language characters.`,
    `No English words allowed in output. No explanations.`,
    ``,
    `${text}`,
  ].join("\n");
}

// src/core/ai/prompts/refinery/enrich-aliases.ts
import type { Message } from "@ai/types/messages";

export const buildAliasesPrompt = (chunkText: string, heading: string): Message[] => [
  {
    role: "system",
    content: `Extract aliases / alternative names for the concept in this note.

Rules:
- Output a JSON array of strings: ["alias1", "alias2"].
- Include common synonyms, abbreviations, alternative phrasings.
- Include the name from the heading itself.
- Do NOT include the answer body — only aliases.
- Max 8 aliases.
- If none, output empty array [].

Output ONLY the JSON array, nothing else.`,
  },
  {
    role: "user",
    content: `Heading: ${heading}\n\nChunk:\n${chunkText.slice(0, 600)}\n\nAliases:`,
  },
];

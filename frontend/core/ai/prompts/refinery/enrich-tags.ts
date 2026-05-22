// src/core/ai/prompts/refinery/enrich-tags.ts
import type { Message } from "@ai/types/messages";

export const buildTagsPrompt = (chunkText: string, heading: string): Message[] => [
  {
    role: "system",
    content: `Extract tags / categories for this note.

Rules:
- Output a JSON array of strings: ["tag1", "tag2"].
- Use lowercase, hyphenated tags (e.g. "quantum-mechanics", "organic-chemistry").
- Max 6 tags.
- Be specific — prefer "aufbau-principle" over "chemistry".
- If chunk is a table, add "table" tag.

Output ONLY the JSON array, nothing else.`,
  },
  {
    role: "user",
    content: `Heading: ${heading}\n\nChunk:\n${chunkText.slice(0, 600)}\n\nTags:`,
  },
];

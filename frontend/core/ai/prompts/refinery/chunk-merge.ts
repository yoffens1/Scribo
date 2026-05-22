// src/core/refinery/prompts/chunk-merge.ts
import type { Message } from "@ai/types/messages";

/**
 * Build the system + user messages for semantic chunk merging.
 * LLM receives two similar chunks and produces a merged version.
 */
export const buildChunkMergePrompt = (
  existingChunk: string,
  newChunk: string,
): Message[] => [
  {
    role: "system",
    content: `You merge two similar note chunks into one coherent, non-redundant chunk.

Rules:
- Remove duplicate information — if both chunks say the same thing, keep the better version.
- Preserve ALL unique information from both chunks.
- Merge in a natural way — the result should read like a single well-written note.
- Keep the original structure (headings, lists) if present.
- Do NOT add new information or commentary.
- Output ONLY the merged chunk text, no markdown fences, no explanation.`,
  },
  {
    role: "user",
    content: `EXISTING CHUNK:\n${existingChunk}\n\nNEW CHUNK:\n${newChunk}\n\nMerged result:`,
  },
];

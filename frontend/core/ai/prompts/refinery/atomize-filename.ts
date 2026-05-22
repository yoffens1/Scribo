// src/core/ai/prompts/refinery/atomize-filename.ts
import type { Message } from "@ai/types/messages";

/**
 * Prompt for generating a filename from a chunk + heading.
 * Output example: "Atom.md"
 */
export const buildFilenamePrompt = (chunkText: string, heading: string): Message[] => [
  {
    role: "system",
    content: `You generate a clean filename for a note.

Rules:
- Use lowercase with hyphens (e.g. "atomic-structure.md").
- No special characters except hyphen and dot.
- Max 60 characters (excluding .md).
- If the chunk is a data table, suffix with "-table" (e.g. "particle-charges-table.md").
- Basing filename on heading + chunk content.
- Output ONLY the filename, nothing else.`,
  },
  {
    role: "user",
    content: `Heading: ${heading}\n\nChunk:\n${chunkText.slice(0, 500)}\n\nFilename:`,
  },
];

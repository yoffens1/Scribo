// src/core/refinery/prompts/taxonomy-generate.ts
import type { Message } from "@ai/types/messages";

/**
 * Build the system + user messages for taxonomy generation.
 * LLM sees all new (non-deduped) chunks and proposes an ideal folder tree.
 */
export const buildTaxonomyPrompt = (
  chunks: Array<{ hash: string; text: string; sourcePath: string }>,
  maxDepth: number,
): Message[] => {
  const chunkList = chunks
    .map(
      (c) =>
        `<chunk hash="${c.hash}" source="${c.sourcePath}">\n${c.text.slice(0, 500)}\n</chunk>`,
    )
    .join("\n\n");

  return [
    {
      role: "system",
      content: `You are a knowledge librarian organizing unstructured notes into a clean folder hierarchy. 

Your task: given a set of note chunks, propose an ideal folder tree (max depth ${maxDepth}) that organizes them by topic.

Rules:
- Use clear, concise English folder names in Title Case (e.g. "Network Security", "Machine Learning"). Do NOT use hyphens for spaces.
- Group related chunks under shared parent folders.
- Do NOT create folders named after individual source files — extract the topic.
- Each chunk should be placed in exactly one folder (no duplicates).
- Prefer breadth over depth — don't nest deeper than necessary.
- If a chunk could fit multiple folders, pick the most specific one.

Output format — valid JSON only, no markdown, no explanation:
{
  "roots": [
    {
      "name": "Folder Name",
      "description": "what this folder contains",
      "children": [...],
      "assignedChunks": ["hash1", "hash2"]
    }
  ],
  "rationale": "brief explanation of your organization choices"
}`,
    },
    {
      role: "user",
      content: `Here are the note chunks to organize:\n\n${chunkList}`,
    },
  ];
};

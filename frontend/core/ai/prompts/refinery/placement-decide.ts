// src/core/refinery/prompts/placement-decide.ts
import type { Message } from "@ai/types/messages";

/**
 * Build the system + user messages for placement decisions.
 * LLM sees the proposed taxonomy + existing output structure and decides
 * where to actually put chunks on disk.
 */
export const buildPlacementPrompt = (
  proposedTree: string,
  existingTree: string,
  chunks: Array<{ hash: string; text: string }>,
): Message[] => {
  const chunkList = chunks
    .map(
      (c) =>
        `<chunk hash="${c.hash}">\n${c.text.slice(0, 300)}\n</chunk>`,
    )
    .join("\n\n");

  return [
    {
      role: "system",
      content: `You are a filesystem organizer. Given:
1. A proposed folder tree (ideal organization for new chunks)
2. An existing folder tree (what's already on disk)
3. A set of chunks to place

Decide where each chunk should go, resolving conflicts between proposed and existing structure.

Rules:
- Prefer placing the chunk into an existing folder over creating a new duplicate folder.
- If a proposed folder matches an existing one by topic, use the existing name.
- If a proposed folder is a subtopic of an existing folder, nest it under the existing one.
- Only create new folders when no suitable existing folder exists.
- Always use the existing folder name when merging (don't rename what's already there).
- outputPath MUST end with .md (e.g. "atom/structure.md").
- action MUST be exactly one of: "create", "merge", "rename", "nest". No other values.

Output format — valid JSON only:
{
  "decisions": [
    {
      "chunkHash": "hash",
      "outputPath": "existing-folder/filename.md",
      "action": "create",
      "existingTarget": "path/to/existing.md",
      "reason": "why this decision"
    }
  ],
  "foldersToCreate": ["path/to/new-folder"],
  "rationale": "brief explanation"
}`,
    },
    {
      role: "user",
      content: `PROPOSED TREE:\n${proposedTree}\n\nEXISTING TREE:\n${existingTree}\n\nCHUNKS TO PLACE:\n${chunkList}`,
    },
  ];
};

// src/core/ai/prompts/refinery/atomize-heading.ts
import type { Message } from "@ai/types/messages";

/**
 * Prompt for generating a question-style heading for a chunk.
 * Output example: "## What is an atom?"
 */
export const buildQuestionHeadingPrompt = (chunkText: string, sourcePath: string): Message[] => [
  {
    role: "system",
    content: `You are a knowledge curator. Given a note chunk, write a SHORT question-style heading (like "## What is X?") that captures the single core concept in this chunk.

Rules:
- Extract exactly ONE concept per chunk.
- Heading must start with "## ".
- Be specific — prefer "## What is the Aufbau principle?" over "## What is chemistry?".
- If the chunk is a data table, use format "## Table: [what this table shows]".
- Do NOT include the answer — only the question heading.
- Output ONLY the heading line, no markdown fences, no explanation.`,
  },
  {
    role: "user",
    content: `Source file: ${sourcePath}\n\nChunk:\n${chunkText.slice(0, 800)}\n\nHeading:`,
  },
];

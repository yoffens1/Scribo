import type { Message } from "@ai/types/messages";

/**
 * Prompt for generating question-style heading and filename in a single JSON response.
 */
export const buildAtomizePrompt = (chunkText: string, sourcePath: string): Message[] => [
  {
    role: "system",
    content: `You are a knowledge curator. Given a note chunk, transform it into an atomic flashcard.
Your goal is to:
1. Formulate a SHORT question-style heading (like "## What is X?") that captures the single core concept in this chunk.
2. Determine an appropriate, clean Title Case filename in singular form (like "Virtual Private Network.md" or "Firewall.md") for the card. The filename must be a noun phrase, not a question.

If the chunk is a data table, use "## Table: [what this table shows]" and a matching filename like "Table of Elements.md".
If the chunk contains a cloze deletion (e.g., {{c1::...}}), formulate a suitable heading like "## Cloze: [concept]" and a matching filename like "Aufbau Principle Cloze.md".

You must respond with a JSON object of the following schema:
{
  "questionHeading": "## What is the Aufbau principle?",
  "filename": "Aufbau Principle.md"
}

Ensure the questionHeading starts with "## ".
Ensure the filename is in Title Case, ends with ".md", contains no invalid filesystem characters, and is a concise noun or concept name in singular form.

Do not include any Markdown wrapper, fences, or text outside the JSON object. Just return the raw JSON.`,
  },
  {
    role: "user",
    content: `Source file: ${sourcePath}\n\nChunk:\n${chunkText.slice(0, 1500)}`,
  },
];

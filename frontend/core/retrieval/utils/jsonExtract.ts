// src/core/retrieval/utils/jsonExtract.ts

/**
 * Extract a balanced JSON array from arbitrary text, starting from the first "[{".
 * Handles markdown-wrapped JSON, prefixed text, and multiple bracket levels.
 */
export function extractJsonArray(text: string): string | null {
  const start = text.indexOf("[{");
  if (start === -1) return null;
  return extractBalanced(text, start, "[]");
}

/**
 * Extract a balanced JSON object from arbitrary text, starting from the first "{".
 */
export function extractJsonObject(text: string): string | null {
  const start = text.indexOf("{");
  if (start === -1) return null;
  return extractBalanced(text, start, "{}");
}

function extractBalanced(text: string, start: number, brackets: string): string | null {
  const [open, close] = brackets;
  let depth = 0;
  let inString = false;
  let escape = false;

  for (let i = start; i < text.length; i++) {
    const ch = text[i];
    if (escape) { escape = false; continue; }
    if (ch === "\\") { escape = true; continue; }
    if (ch === '"') { inString = !inString; continue; }
    if (inString) continue;
    if (ch === open || ch === "[") depth++;
    else if (ch === close || ch === "]") {
      depth--;
      if (depth === 0) return text.slice(start, i + 1);
    }
  }
  return null;
}

import { countTokens } from "./countTokens";
import { splitByWords } from "./splitByWords";

/**
 * Split a single line by sentence boundaries, falling back to word split.
 * Ensures no trailing text is silently dropped.
 */
function splitBySentenceBoundaries(line: string, maxTokens: number): string[] {
  const sentenceMatch = line.match(/[^\.!\?]+[\.!\?]+|.+$/g);
  if (!sentenceMatch) return splitByWords(line, maxTokens);

  const parts: string[] = [];
  let current = "";
  for (const sentence of sentenceMatch) {
    const trimmed = sentence.trim();
    if (!trimmed) continue;
    const candidate = current ? current + " " + trimmed : trimmed;
    if (countTokens(candidate) <= maxTokens) {
      current = candidate;
    } else {
      if (current) parts.push(current.trim());
      if (countTokens(trimmed) > maxTokens) {
        parts.push(...splitByWords(trimmed, maxTokens));
        current = "";
      } else {
        current = trimmed;
      }
    }
  }
  if (current) parts.push(current.trim());
  return parts.length > 0 ? parts : splitByWords(line, maxTokens);
}

export { splitBySentenceBoundaries };

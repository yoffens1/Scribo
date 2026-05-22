import { countTokens } from "./countTokens";
import { splitBySentenceBoundaries } from "./splitBySentenceBoundaries";

/**
 * Split oversized text into chunks respecting paragraph (newline) boundaries.
 * Paragraphs are kept whole; only split a paragraph by sentences if it
 * individually exceeds maxTokens.
 */
export function splitOversizedParagraph(
  para: string,
  maxTokens: number,
): string[] {
  if (countTokens(para) <= maxTokens) return [para];

  const lines = para.split("\n");
  const chunks: string[] = [];
  let batch: string[] = [];
  let batchTokens = 0;

  const flush = () => {
    if (batch.length > 0) {
      chunks.push(batch.join("\n"));
      batch = [];
      batchTokens = 0;
    }
  };

  for (const line of lines) {
    const lt = countTokens(line);

    if (lt > maxTokens) {
      flush();
      chunks.push(...splitBySentenceBoundaries(line, maxTokens));
      continue;
    }

    const separatorTokens = batch.length > 0 ? 1 : 0;
    if (batchTokens + separatorTokens + lt > maxTokens) {
      flush();
    }

    batch.push(line);
    batchTokens += (batch.length > 1 ? 1 : 0) + lt;
  }

  flush();
  return chunks.length > 0 ? chunks : [para];
}

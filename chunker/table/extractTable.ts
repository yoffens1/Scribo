import { countTokens } from "../token/countTokens";

/**
 * Scan markdown text for pipe-delimited tables, extract them, and replace
 * with numbered placeholders ({{TABLE_0}}, {{TABLE_1}}, …).
 *
 * A table is detected when a line starts with `|` and contains another `|`,
 * followed by consecutive pipe‑lines. It must include a separator row
 * (e.g. `|---|` or `|:---:|`) to qualify as a real table.
 *
 * @returns The text with tables replaced by placeholders, plus the extracted
 *          table blocks with their token counts.
 */
export function extractTable(text: string): {
  replacedText: string;
  tables: { placeholder: string; content: string; tokens: number }[];
} {
  const tables: { placeholder: string; content: string; tokens: number }[] = [];
  const lines = text.split("\n");
  const newLines: string[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // A line that starts and contains '|' may begin a table block.
    if (line.trim().startsWith("|") && line.includes("|")) {
      const tableLines: string[] = [line];
      let j = i + 1;

      // Gather consecutive pipe‑lines.
      while (
        j < lines.length &&
        lines[j].trim().startsWith("|") &&
        lines[j].includes("|")
      ) {
        tableLines.push(lines[j]);
        j++;
      }

      // A valid table must have a separator row (|---|, |:---:|, etc.).
      const hasSeparator = tableLines.some((l) => /\|[-:\s]*---/.test(l));
      if (hasSeparator) {
        const placeholder = `{{TABLE_${tables.length}}}`;
        const tableContent = tableLines.join("\n");

        tables.push({
          placeholder,
          content: tableContent,
          tokens: countTokens(tableContent),
        });

        // Replace the whole table block with a placeholder.
        newLines.push(placeholder);
        i = j;
        continue;
      }
    }

    // Non‑table line — keep as‑is.
    newLines.push(line);
    i++;
  }

  return {
    replacedText: newLines.join("\n"),
    tables,
  };
}

/** Parse a single markdown table row (pipe‑delimited), ignoring leading /
 *  trailing pipes, returning an array of trimmed cell values. */
function parseTableRow(row: string): string[] {
  const cleaned = row.replace(/^\|/, "").replace(/\|$/, "");
  return cleaned.split("|").map((cell) => cell.trim());
}

/** True when the row consists only of pipes, spaces, hyphens and colons
 *  and contains at least three hyphens — a markdown table separator.
 *  Handles all alignment variants: |---|, |:---|, |---:|, |:---:|. */
function isSeparatorRow(row: string): boolean {
  const trimmed = row.trim();
  return /^[\|\s\-:]+$/.test(trimmed) && /:?-{3,}:?/.test(trimmed);
}

/**
 * Convert a raw markdown table block into an array of natural‑language
 * sentences, one per data row.
 *
 * Example:
 * ```
 * | Particle | Charge | Location  |
 * |----------|--------|-----------|
 * | Proton   | +1     | Nucleus   |
 * | Electron | -1     | Orbital   |
 * ```
 * →
 * ```
 * "1. Particle: Proton. Charge: +1. Location: Nucleus"
 * "2. Particle: Electron. Charge: -1. Location: Orbital"
 * ```
 *
 * Each output row is prefixed with its 1‑based index for positional
 * awareness in embedding / LLM contexts.
 */
export function linearizeTable(tableText: string): string[] {
  const lines = tableText.split("\n").filter((l) => l.trim().length > 0);
  if (lines.length < 2) return [tableText];

  // Locate the separator row (|---|).
  let headerLine = "";
  let separatorIndex = -1;
  for (let i = 0; i < lines.length; i++) {
    if (isSeparatorRow(lines[i])) {
      separatorIndex = i;
      break;
    }
  }

  if (separatorIndex === -1) return [tableText];

  // Header row is the line immediately before the separator.
  headerLine = lines[separatorIndex - 1] || "";
  const headers = parseTableRow(headerLine);

  // All lines after the separator are data rows.
  const dataRows = lines.slice(separatorIndex + 1);
  const result: string[] = [];

  for (const row of dataRows) {
    const cells = parseTableRow(row);
    if (cells.length === 0) continue;

    // Build sentence: "header: value. header: value. …"
    const parts: string[] = [];
    for (let i = 0; i < headers.length && i < cells.length; i++) {
      const header = headers[i];
      const value = cells[i];
      if (value === "" || value === undefined) continue;
      parts.push(`${header}: ${value}`);
    }
    if (parts.length > 0) {
      result.push(parts.join(". "));
    }
  }

  // Prepend 1‑based row index so the chunk knows its position in the table.
  const numbered = result.map((desc, idx) => `${idx + 1}. ${desc}`);
  return numbered.length > 0 ? numbered : [tableText];
}

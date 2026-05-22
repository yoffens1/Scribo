/**
 * Удаляет начальную нумерацию (1., 1), 2.1., IV. и т.п.) в каждой строке текста.
 */
export function removeListNumbering(text: string): string {
  return text
    .split("\n")
    .map((line) => {
      // If line is a Markdown heading, preserve the heading markers and remove numbering that follows.
      const headingMatch = line.match(/^(\s*#{1,6}\s+)(.*)$/);
      if (headingMatch) {
        const prefix = headingMatch[1]; // e.g., "## "
        let content = headingMatch[2]; // rest after heading
        // Remove leading numbering from content (e.g., "2. text" -> "text")
        content = content
          .replace(/^(\s*(?:\d+(?:\.\d+)*\s*(?:[\.\)\-:]\s*)?))/, "")
          .trim();
        return prefix + content;
      } else {
        // Non-heading line: remove numbering as before, but maybe keep list markers? Original regex tries to remove numbering at start of line.
        return line
          .replace(
            /^(\s*(?:[-\*]?\s*)?(?:\d+(?:\.\d+)*\s*(?:[\.\)\-:]\s*)?))/u,
            "",
          )
          .trim();
      }
    })
    .join("\n");
}

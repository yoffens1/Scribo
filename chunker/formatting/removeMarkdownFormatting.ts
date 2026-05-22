/**
 * Удаляет инлайн-форматирование Markdown, оставляя только текстовое содержимое.
 * Поддерживает:
 * - **bold** и __bold__
 * - *italic* и _italic_
 * - ==highlight==
 * - ~~strikethrough~~
 * - `inline code`
 */
export function removeMarkdownFormatting(text: string): string {
  return (
    text
      .replace(/\*\*(.+?)\*\*/g, "$1") // **bold**
      .replace(/__(.+?)__/g, "$1") // __bold__
      // Italic / underline – only when not part of a word (word boundary required)
      .replace(/(?<!\w)_([^_]+)_(?!\w)/g, "$1") // _italic_
      .replace(/(?<!\w)\*([^\*]+)\*(?!\w)/g, "$1") // *italic*
      .replace(/~~(.+?)~~/g, "$1") // ~~strikethrough~~
      .replace(/==(.+?)==/g, "$1") // ==highlight==
      .replace(/`(.+?)`/g, "$1")
  ); // `inline code`
}

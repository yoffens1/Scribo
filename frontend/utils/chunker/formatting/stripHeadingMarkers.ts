/**
 * Удаляет маркеры Markdown-заголовков (##, ### и т.д.) в начале строки,
 * оставляя только текст заголовка. Обрабатывает каждую строку отдельно.
 */
export function stripHeadingMarkers(text: string): string {
  return text
    .split("\n")
    .map((line) => line.replace(/^\s*#{1,6}\s+/, "").trim())
    .join("\n");
}

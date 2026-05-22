export function removeMarkdownLinks(text: string): string {
  // Сначала удаляем обычные ссылки [text](url)
  let cleaned = text.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1");
  // Вики‑ссылки: [[target|alias]] -> alias, [[target]] -> target
  cleaned = cleaned.replace(/\[\[([^\]]+)\]\]/g, (_, content) => {
    const parts = content.split("|");
    // Если есть алиас, берём его, иначе название страницы
    return parts.length > 1 ? parts[1] : parts[0];
  });
  return cleaned;
}

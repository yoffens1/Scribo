/**
 * Простейший парсер YAML, который понимает:
 * - ключи: значения (однострочные)
 * - списки через дефисы (- элемент)
 * - вложенность пока не поддерживается (достаточно для Obsidian frontmatter)
 */
function simpleYamlParse(yamlText: string): Record<string, any> {
  const lines = yamlText.split("\n");
  const result: Record<string, any> = {};
  let currentKey: string | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    // Ключ: значение
    if (trimmed.includes(":") && !trimmed.startsWith("-")) {
      const colonIndex = trimmed.indexOf(":");
      const key = trimmed.slice(0, colonIndex).trim();
      const value = trimmed.slice(colonIndex + 1).trim();
      result[key] = value;
      currentKey = key;
    }
    // Элемент списка (- value)
    else if (trimmed.startsWith("-") && currentKey) {
      const listItem = trimmed.slice(1).trim();
      if (!Array.isArray(result[currentKey])) {
        result[currentKey] = [];
      }
      result[currentKey].push(listItem);
    }
    // Дополнительные строки для списков (если есть вложенность, но не будем усложнять)
  }

  /**
   * Извлекает метаданные из frontmatter (--- ... ---) в начале Markdown-текста.
   * Возвращает объект с полями или null, если frontmatter не найден.
   */

  // Парсим значения: если строка пустая - удаляем, если похожа на число/булево - конвертируем
  for (const [key, val] of Object.entries(result)) {
    if (typeof val === "string") {
      if (val === "true" || val === "false") result[key] = val === "true";
      else if (!isNaN(Number(val)) && val !== "") result[key] = Number(val);
      else if (val === "") delete result[key];
    }
  }

  return result;
}

export function extractYamlFrontmatter(
  markdownContent: string,
): Record<string, any> | null {
  // Ищем первую строку с '---' в начале
  const frontmatterRegex = /^---\n([\s\S]*?)\n---/;
  const match = markdownContent.match(frontmatterRegex);
  if (!match) return null;

  const yamlText = match[1];
  return simpleYamlParse(yamlText);
}

// src/utils/chunker/splitByHeadings.ts

/**
 * Разбивает текст на секции по заголовкам указанного уровня.
 * Если `level` не указан, разбивает по любым заголовкам (от # до ######).
 * Игнорирует строки, начинающиеся с '#', если они являются частью таблицы (начинаются с '|').
 */
export function splitByHeadings(text: string, level?: number): string[] {
  const lines = text.split("\n");
  const sections: string[] = [];
  let current: string[] = [];

  // Строим регулярку для заголовка нужного уровня
  let headingRegex: RegExp;
  if (level !== undefined && level >= 1 && level <= 6) {
    headingRegex = new RegExp(`^#{${level}}\\s`);
  } else {
    headingRegex = /^#{1,6}\s/;
  }

  for (const line of lines) {
    if (headingRegex.test(line) && !line.trim().startsWith("|")) {
      if (current.length > 0) {
        sections.push(current.join("\n"));
      }
      current = [line];
    } else {
      current.push(line);
    }
  }
  if (current.length > 0) {
    sections.push(current.join("\n"));
  }
  return sections;
}

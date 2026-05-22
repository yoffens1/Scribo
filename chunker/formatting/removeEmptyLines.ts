/**
 * Удаляет пустые строки из текста чанка.
 * Заменяет два и более перевода строки на один.
 */
export function removeEmptyLines(text: string): string {
  return text.replace(/\n{2,}/g, "\n").trim();
}

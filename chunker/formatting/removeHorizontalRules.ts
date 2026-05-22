/**
 * Удаляет горизонтальные линии Markdown: строки, состоящие только из дефисов, звёздочек или подчёркиваний (с пробелами).
 * Например: ---, ***, ___
 */
export function removeHorizontalRules(text: string): string {
  return text.replace(/^\s*[-*_]{3,}\s*$/gm, "").trim();
}

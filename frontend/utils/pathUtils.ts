
/**
 * Нормализует путь к файлу/папке внутри vault Obsidian.
 * - убирает './' в начале
 * - заменяет '\' на '/'
 * - удаляет дублирующиеся слеши
 * - удаляет конечный слеш (кроме случая, когда путь — корень vault)
 */
export function normalizePath(filePath: string): string {
  let clean = filePath.replace(/^\.\//, "");
  clean = clean.replace(/\\/g, "/");
  clean = clean.replace(/([^:])\/{2,}/g, "$1/");
  if (clean.length > 1 && clean.endsWith("/")) {
    clean = clean.slice(0, -1);
  }
  return clean;
}

/**
 * Возвращает нормализованный путь для TFile.
 * (на практике TFile.path уже нормален, но лишняя проверка не помешает)
 */
export function getCleanPath(file: { path: string }): string {
  return normalizePath(file.path);
}

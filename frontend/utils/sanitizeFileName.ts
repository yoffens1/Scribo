/**
 * Sanitizes a model name so it can be safely used in a file name.
 * Replaces characters that are not alphanumeric, dash, or underscore with '_'.
 */
export function sanitizeFileName(name: string): string {
  return name.replace(/[^a-zA-Z0-9\-_]/g, "_");
}

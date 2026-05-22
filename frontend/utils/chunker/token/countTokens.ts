export function countTokens(text: string): number {
  if (!text) return 0;
  const tokens = text.match(/\w+|[^\w\s]|\s+/g) || [];
  return tokens.length;
}

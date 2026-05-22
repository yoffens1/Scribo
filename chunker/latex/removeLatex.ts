export function removeLatex(text: string): string {
  let cleaned = text.replace(/\$\$[\s\S]*?\$\$/g, "");
  cleaned = cleaned.replace(/\$[^$]+\$/g, "");
  return cleaned;
}

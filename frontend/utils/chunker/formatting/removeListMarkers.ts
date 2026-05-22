/**
 * Remove leading list markers like "- ", "* ", "+ " at the start of each line.
 * After this cleanup the text becomes pure content without markup hints.
 */
export function removeListMarkers(text: string): string {
  return text
    .split("\n")
    .map((line) => line.replace(/^(\s*)[\-\*\+]\s+/, "$1"))
    .join("\n");
}

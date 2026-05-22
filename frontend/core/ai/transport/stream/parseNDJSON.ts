// src/core/ai/transport/stream/parseNDJSON.ts

/**
 * Parses NDJSON (newline-delimited JSON) stream from a Response body.
 * Each line is a complete JSON object. Yields parsed JSON.
 */
export async function* parseNDJSONStream(
  resp: Response,
  provider: string,
): AsyncIterable<Record<string, unknown>> {
  if (!resp.ok || !resp.body) {
    const text = await resp.text().catch(() => "");
    throw new Error(`[${provider}] stream HTTP ${resp.status}: ${text.slice(0, 200)}`);
  }

  const reader = resp.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          yield JSON.parse(line);
        } catch (err) {
          console.warn(`[${provider}] NDJSON parse error:`, String(err));
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

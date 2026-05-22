// src/core/ai/transport/stream/parseSSE.ts

/**
 * Parses SSE (Server-Sent Events) stream from a Response body.
 * Yields parsed JSON payloads from `data:` lines.
 * Stops on `data: [DONE]`.
 */
export async function* parseSSEStream(
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
        const trimmed = line.trim();
        if (!trimmed || !trimmed.startsWith("data: ")) continue;
        const data = trimmed.slice(6);
        if (data === "[DONE]") return;
        try {
          yield JSON.parse(data);
        } catch (err) {
          console.warn(`[${provider}] SSE parse error:`, String(err));
        }
      }
    }
  } finally {
    reader.releaseLock();
  }
}

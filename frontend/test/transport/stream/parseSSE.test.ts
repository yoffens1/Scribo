import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseSSEStream } from "@ai/transport/stream/parseSSE";

function makeResponse(body: string, ok = true, status = 200): Response {
  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode(body));
      controller.close();
    },
  });
  return new Response(ok ? stream : null, { status });
}

async function collect<T>(iter: AsyncIterable<T>): Promise<T[]> {
  const items: T[] = [];
  for await (const item of iter) items.push(item);
  return items;
}

describe("parseSSEStream", () => {
  it("parses data lines", async () => {
    const r = await collect(parseSSEStream(makeResponse('data: {"a":1}\n\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }]);
  });
  it("stops on [DONE]", async () => {
    const r = await collect(parseSSEStream(makeResponse('data: {"a":1}\n\ndata: [DONE]\n\ndata: {"b":2}\n\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }]);
  });
  it("skips invalid JSON", async () => {
    const r = await collect(parseSSEStream(makeResponse('data: {"a":1}\n\ndata: bad\n\ndata: {"b":2}\n\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }, { b: 2 }]);
  });
  it("throws on non-ok", async () => {
    await assert.rejects(() => collect(parseSSEStream(makeResponse("", false, 500), "t")), /HTTP 500/);
  });
});

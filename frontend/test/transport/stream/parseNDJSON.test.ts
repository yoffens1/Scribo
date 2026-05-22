import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseNDJSONStream } from "@ai/transport/stream/parseNDJSON";

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

describe("parseNDJSONStream", () => {
  it("parses single line", async () => {
    const r = await collect(parseNDJSONStream(makeResponse('{"a":1}\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }]);
  });
  it("parses multiple lines", async () => {
    const r = await collect(parseNDJSONStream(makeResponse('{"a":1}\n{"b":2}\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }, { b: 2 }]);
  });
  it("skips empty lines", async () => {
    const r = await collect(parseNDJSONStream(makeResponse('\n{"a":1}\n\n{"b":2}\n\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }, { b: 2 }]);
  });
  it("continues on invalid JSON", async () => {
    const r = await collect(parseNDJSONStream(makeResponse('{"a":1}\nbad\n{"b":2}\n'), "t"));
    assert.deepEqual(r, [{ a: 1 }, { b: 2 }]);
  });
  it("throws on non-ok", async () => {
    await assert.rejects(() => collect(parseNDJSONStream(makeResponse("", false, 500), "t")), /HTTP 500/);
  });
});

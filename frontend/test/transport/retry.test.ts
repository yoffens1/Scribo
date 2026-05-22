import { describe, it, beforeEach, afterEach, mock } from "node:test";
import assert from "node:assert/strict";
import { HttpError, ValidationError, ParseError } from "@ai/transport/HttpError";
import { shouldRetry, withRetry } from "@ai/transport/retry";

describe("shouldRetry", () => {
  it("retries 429", () => assert.equal(shouldRetry(new HttpError("x", 429, "o")), true));
  it("retries 5xx", async () => {
    assert.equal(shouldRetry(new HttpError("x", 500, "o")), true);
    assert.equal(shouldRetry(new HttpError("x", 503, "o")), true);
  });
  it("retries status 0", () => assert.equal(shouldRetry(new HttpError("x", 0, "o")), true));
  it("not 400", () => assert.equal(shouldRetry(new HttpError("x", 400, "o")), false));
  it("not 401", () => assert.equal(shouldRetry(new HttpError("x", 401, "o")), false));
  it("not AbortError", async () => {
    assert.equal(shouldRetry(new DOMException("aborted", "AbortError")), false);
    assert.equal(shouldRetry(Object.assign(new Error("a"), { name: "AbortError" })), false);
  });
  it("not ValidationError", () => assert.equal(shouldRetry(new ValidationError("x", "o")), false));
  it("not ParseError", () => assert.equal(shouldRetry(new ParseError("x", "o")), false));
  it("retries unknown", () => assert.equal(shouldRetry(new Error("x")), true));
});

describe("withRetry", () => {
  let origSetTimeout: typeof setTimeout;
  let delays: number[] = [];

  beforeEach(async () => {
    delays = [];
    origSetTimeout = globalThis.setTimeout;
    // Mock setTimeout to record delay and call immediately
    globalThis.setTimeout = ((cb: () => void, ms: number) => {
      if (ms > 0) delays.push(ms);
      return origSetTimeout(cb, 0) as any;
    }) as any;
  });

  afterEach(async () => {
    globalThis.setTimeout = origSetTimeout;
  });

  it("returns on first success", async () => {
    const fn = mock.fn(() => Promise.resolve("ok"));
    const r = await withRetry(fn, "test", 3);
    assert.equal(r, "ok");
    assert.equal(fn.mock.callCount(), 1);
  });

  it("retries on transient errors and succeeds", async () => {
    let calls = 0;
    const fn = mock.fn(() => {
      calls++;
      if (calls <= 2) return Promise.reject(new HttpError("err", 500, "o"));
      return Promise.resolve("ok");
    });
    const r = await withRetry(fn, "test", 3);
    assert.equal(r, "ok");
    assert.equal(fn.mock.callCount(), 3);
  });

  it("throws after max retries", async () => {
    const fn = mock.fn(() => Promise.reject(new Error("err")));
    await assert.rejects(() => withRetry(fn, "test", 2));
    assert.equal(fn.mock.callCount(), 2);
  });

  it("throws immediately on non-retryable", async () => {
    const fn = mock.fn(() => Promise.reject(new ValidationError("bad", "o")));
    await assert.rejects(() => withRetry(fn, "test", 3));
    assert.equal(fn.mock.callCount(), 1);
  });

  it("exponential backoff delays", async () => {
    const fn = mock.fn(() => Promise.reject(new Error("err")));
    await assert.rejects(() => withRetry(fn, "test", 4));
    assert.equal(fn.mock.callCount(), 4);
    // Delays: 250, 500, 1000
    assert.equal(delays[0], 250);
    assert.equal(delays[1], 500);
    assert.equal(delays[2], 1000);
  });
});

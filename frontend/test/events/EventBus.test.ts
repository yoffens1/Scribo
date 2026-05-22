import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import { AiEventBus } from "@ai/events/EventBus";
import type { AiEvents } from "@ai/types/ai-events";

const makeRetry = () => ({
  provider: "o", label: "x", attempt: 1, maxRetries: 3, delayMs: 1, error: "e", timestamp: 1,
});

describe("EventBus", () => {
  it("on + emit", async () => {
    const bus = new AiEventBus<AiEvents>();
    const fn = mock.fn();
    bus.on("ai:retry", fn);
    bus.emit("ai:retry", makeRetry());
    assert.equal(fn.mock.callCount(), 1);
  });

  it("off unsubscribes", async () => {
    const bus = new AiEventBus<AiEvents>();
    const fn = mock.fn();
    bus.on("ai:retry", fn);
    bus.off("ai:retry", fn);
    bus.emit("ai:retry", makeRetry());
    assert.equal(fn.mock.callCount(), 0);
  });

  it("unsubscribe via returned function", async () => {
    const bus = new AiEventBus<AiEvents>();
    const fn = mock.fn();
    const unsub = bus.on("ai:retry", fn);
    unsub();
    bus.emit("ai:retry", makeRetry());
    assert.equal(fn.mock.callCount(), 0);
  });

  it("multiple listeners", async () => {
    const bus = new AiEventBus<AiEvents>();
    const a = mock.fn(), b = mock.fn();
    bus.on("ai:request:done", a);
    bus.on("ai:request:done", b);
    bus.emit("ai:request:done", { provider: "o", path: "/", latencyMs: 1, status: 200, timestamp: 1 });
    assert.equal(a.mock.callCount(), 1);
    assert.equal(b.mock.callCount(), 1);
  });

  it("async error isolation", async () => {
    const bus = new AiEventBus<AiEvents>();
    const bad = mock.fn(() => Promise.reject(new Error("boom")));
    const good = mock.fn();
    bus.on("ai:retry", bad);
    bus.on("ai:retry", good);
    await bus.emit("ai:retry", makeRetry());
    assert.equal(bad.mock.callCount(), 1);
    assert.equal(good.mock.callCount(), 1);
  });

  it("clear(type)", async () => {
    const bus = new AiEventBus<AiEvents>();
    const a = mock.fn(), b = mock.fn();
    bus.on("ai:retry", a);
    bus.on("ai:request:done", b);
    bus.clear("ai:retry");
    bus.emit("ai:retry", makeRetry());
    bus.emit("ai:request:done", { provider: "o", path: "/", latencyMs: 1, status: 200, timestamp: 1 });
    assert.equal(a.mock.callCount(), 0);
    assert.equal(b.mock.callCount(), 1);
  });

  it("clear() all", async () => {
    const bus = new AiEventBus<AiEvents>();
    const fn = mock.fn();
    bus.on("ai:retry", fn);
    bus.on("ai:request:done", fn);
    bus.clear();
    bus.emit("ai:retry", makeRetry());
    bus.emit("ai:request:done", { provider: "o", path: "/", latencyMs: 1, status: 200, timestamp: 1 });
    assert.equal(fn.mock.callCount(), 0);
  });

  it("emit no listeners doesn't throw", async () => {
    const bus = new AiEventBus<AiEvents>();
    assert.doesNotThrow(() => bus.emit("ai:retry", makeRetry()));
  });
});

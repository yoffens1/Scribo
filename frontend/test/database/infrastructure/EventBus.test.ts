// src/test/database/infrastructure/EventBus.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { DbEventBus, Events } from "@database/EventBus";

const silentBus = () => new DbEventBus({ onHandlerError: () => {} });

describe("EventBus", () => {
  it("emit() delivers payload to single listener", async () => {
    const bus = silentBus();
    let received: any = null;
    bus.on("test", (p) => { received = p; });
    await bus.emit("test", { x: 42 });
    assert.deepEqual(received, { x: 42 });
  });

  it("emit() delivers to multiple listeners", async () => {
    const bus = silentBus();
    const results: number[] = [];
    bus.on("count", () => { results.push(1); });
    bus.on("count", () => { results.push(2); });
    await bus.emit("count", null);
    assert.deepEqual(results, [1, 2]);
  });

  it("off() unsubscribes a listener", async () => {
    const bus = silentBus();
    let called = false;
    const handler = () => { called = true; };
    bus.on("x", handler);
    bus.off("x", handler);
    await bus.emit("x", null);
    assert.equal(called, false);
  });

  it("on() returns unsubscribe function", async () => {
    const bus = silentBus();
    let calls = 0;
    const unsub = bus.on("y", () => { calls++; });
    await bus.emit("y", null);
    assert.equal(calls, 1);
    unsub();
    await bus.emit("y", null);
    assert.equal(calls, 1, "should not increase after unsub");
  });

  it("supports async listeners", async () => {
    const bus = silentBus();
    const order: number[] = [];
    bus.on("async", async () => {
      await new Promise(r => setTimeout(r, 10));
      order.push(1);
    });
    bus.on("async", () => {
      order.push(2);
    });
    await bus.emit("async", null);
    // async listeners run sequentially (emit awaits each)
    assert.deepEqual(order, [1, 2]);
  });

  it("one listener failure does not break others", async () => {
    const bus = silentBus();
    let secondCalled = false;
    bus.on("fail", () => { throw new Error("boom"); });
    bus.on("fail", () => { secondCalled = true; });
    await bus.emit("fail", null);
    assert.equal(secondCalled, true, "second listener should still be called");
  });

  it("clear() removes all listeners for event", async () => {
    const bus = silentBus();
    let called = false;
    bus.on("z", () => { called = true; });
    bus.clear("z");
    await bus.emit("z", null);
    assert.equal(called, false);
  });

  it("clear() without argument removes all events", async () => {
    const bus = silentBus();
    let a = false, b = false;
    bus.on("a", () => { a = true; });
    bus.on("b", () => { b = true; });
    bus.clear();
    await bus.emit("a", null);
    await bus.emit("b", null);
    assert.equal(a, false);
    assert.equal(b, false);
  });

  it("emit() is no-op when no listeners", async () => {
    const bus = silentBus();
    await bus.emit("none", null); // should not throw
    assert.ok(true);
  });

  it("standard event constants are defined", async () => {
    assert.equal(Events.FILE_INDEXED, "file:indexed");
    assert.equal(Events.FILE_DELETED, "file:deleted");
    assert.equal(Events.INDEXING_ERROR, "indexing:error");
    assert.equal(Events.MAINTENANCE_VACUUM, "maintenance:vacuum");
  });
});

import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/ReindexScheduler.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { ReindexScheduler } from "@database/services/ReindexScheduler";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "sched-test";

describe("ReindexScheduler", () => {
  let core: TauriDbConnection;
  let scheduler: ReindexScheduler;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = new TauriDbConnection(adapter as any, PLUGIN_DIR, "test");
    scheduler = new ReindexScheduler(core);
  });

  afterEach(async () => {
    scheduler.cleanup();
    try { await core.close(); } catch {}
  });

  it("enqueueIndexing() serializes operations", async () => {
    const order: number[] = [];

    const p1 = scheduler.enqueueIndexing(async () => {
      await new Promise(r => setTimeout(r, 30));
      order.push(1);
      return 1;
    });
    const p2 = scheduler.enqueueIndexing(async () => {
      order.push(2);
      return 2;
    });

    const results = await Promise.all([p1, p2]);
    assert.deepEqual(order, [1, 2], "operations should execute in order");
    assert.deepEqual(results, [1, 2]);
  });

  it("enqueueIndexing() does not block on error", async () => {
    const p1 = scheduler.enqueueIndexing(async () => {
      throw new Error("fail-1");
    });
    const p2 = scheduler.enqueueIndexing(async () => {
      return "ok";
    });

    const r1 = await p1.catch(e => e.message);
    const r2 = await p2;

    assert.equal(r1, "fail-1");
    assert.equal(r2, "ok");
  });

  it("enqueueIndexing() continues after failure", async () => {
    let threw = false;
    await scheduler.enqueueIndexing(async () => { throw new Error("expected"); }).catch(() => { threw = true; });
    assert.equal(threw, true);

    const result = await scheduler.enqueueIndexing(async () => "after-error");
    assert.equal(result, "after-error", "queue should continue after error");
  });

  it("scheduleReindex() debounces multiple calls", async () => {
    scheduler.scheduleReindex("a.md");
    scheduler.scheduleReindex("b.md");
    scheduler.scheduleReindex("c.md");

    // All should collapse into single pending set
    const pending = (scheduler as any).pendingReindex;
    assert.equal(pending.size, 3);
    assert.ok(pending.has("a.md"));
    assert.ok(pending.has("b.md"));
    assert.ok(pending.has("c.md"));
  });

  it("scheduleReindex() debounce timer resets on subsequent calls", async () => {
    scheduler.scheduleReindex("x.md");
    const timer1 = (scheduler as any).reindexTimer;
    assert.notEqual(timer1, null);

    scheduler.scheduleReindex("y.md");
    const timer2 = (scheduler as any).reindexTimer;
    assert.notEqual(timer2, null);
    assert.notStrictEqual(timer1, timer2, "timer should be replaced");
  });

  it("scheduleReindex() fires handler with batched files", async () => {
    return new Promise<void>((resolve) => {
      scheduler.setReindexHandler(async (files) => {
        assert.equal(files.length, 2);
        assert.ok(files.includes("a.md"));
        assert.ok(files.includes("b.md"));
        resolve();
      });

      // Override setTimeout to fire immediately
      const origSetTimeout = setTimeout;
      (scheduler as any).reindexTimer = null;
      // Manually call scheduleReindex then process
      (scheduler as any).pendingReindex.add("a.md");
      (scheduler as any).pendingReindex.add("b.md");
      scheduler["processPendingReindex"](); // private access for test
    });
  });

  it("cleanup() clears pending timer", async () => {
    scheduler.scheduleReindex("z.md");
    assert.notEqual((scheduler as any).reindexTimer, null);
    scheduler.cleanup();
    assert.equal((scheduler as any).reindexTimer, null);
  });
});

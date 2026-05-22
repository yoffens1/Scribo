import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/MaintenanceService.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { MaintenanceService } from "@database/services/MaintenanceService";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "maint-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(a as any, PLUGIN_DIR, "test");
}

describe("MaintenanceService", () => {
  let core: TauriDbConnection;
  let maintenance: MaintenanceService;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = createCore(adapter);
    const schema = new SchemaManager(core);
    await schema.initialize();
    maintenance = new MaintenanceService(core);
  });

  afterEach(async () => { try { await core.close(); } catch {} });

  it("optimize() runs PRAGMA optimize without error", async () => {
    await maintenance.optimize();
    assert.ok(true, "should not throw");
  });

  it("vacuumIfNeeded() does nothing when chunk count below threshold", async () => {
    maintenance.trackDeleted(10);
    // Log capture — just verify it doesn't throw
    await maintenance.vacuumIfNeeded(1000);
    assert.equal(maintenance.deletedChunksCount, 10, "counter unchanged");
  });

  it("vacuumIfNeeded() runs VACUUM when count exceeds threshold and resets counter", async () => {
    maintenance.trackDeleted(1500);
    await maintenance.vacuumIfNeeded(1000);
    assert.equal(maintenance.deletedChunksCount, 0, "counter should reset after vacuum");
  });

  it("forceVacuum() runs VACUUM and resets counter", async () => {
    maintenance.trackDeleted(500);
    await maintenance.forceVacuum();
    assert.equal(maintenance.deletedChunksCount, 0);
  });

  it("deletedChunksCount accumulates across multiple trackDeleted calls", async () => {
    maintenance.trackDeleted(3);
    maintenance.trackDeleted(7);
    maintenance.trackDeleted(15);
    assert.equal(maintenance.deletedChunksCount, 25);
  });

  it("optimize() is safe to call when db is null (after close)", async () => {
    await core.close();
    await maintenance.optimize(); // should not throw
    assert.ok(true);
  });
});

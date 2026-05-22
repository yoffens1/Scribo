import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/EmbeddingPersistenceService.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { EmbeddingSerializer } from "@database/infrastructure/EmbeddingSerializer";
import { FileRepository } from "@database/repositories/FileRepository";
import { ChunkRepository } from "@database/repositories/ChunkRepository";
import { MaintenanceService } from "@database/services/MaintenanceService";
import { DbEventBus } from "@database/EventBus";
import { EmbeddingPersistenceService } from "@database/services/indexing/EmbeddingPersistenceService";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "eps-test";

class MockEmbedder {
  model = "mock-model";
  private chunked: Array<{ text: string; embedding: Float32Array }> = [];
  setChunked(c: Array<{ text: string; embedding: Float32Array }>) { this.chunked = c; }
  embedChunked() { return Promise.resolve(this.chunked); }
}

function makeEmbedding(v: number[]): Float32Array { return new Float32Array(v); }

describe("EmbeddingPersistenceService", () => {
  let core: TauriDbConnection;
  let files: FileRepository;
  let chunks: ChunkRepository;
  let maintenance: MaintenanceService;
  let eventBus: DbEventBus;
  let persistence: EmbeddingPersistenceService;
  let mockEmbedder: MockEmbedder;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = new TauriDbConnection(adapter as any, PLUGIN_DIR, "test");
    const schema = new SchemaManager(core);
    await schema.initialize();
    const ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    chunks = new ChunkRepository(core, ser);
    maintenance = new MaintenanceService(core);
    eventBus = new DbEventBus();
    persistence = new EmbeddingPersistenceService(core, files, chunks, maintenance, eventBus);
    mockEmbedder = new MockEmbedder();
  });

  afterEach(async () => { try { await core.close(); } catch {} });

  const persistParams = (overrides: Partial<any> = {}) => ({
    filePath: "file.md", cleanPath: "file.md", fileName: "file",
    fileHash: "h", fileMtime: null, embeddingModel: "m",
    embeddingDim: 4, chunkingVersion: "1", updatedAt: Date.now(),
    content: "test", embedder: mockEmbedder as any,
    ...overrides,
  });

  it("inserts chunks for new file", async () => {
    mockEmbedder.setChunked([
      { text: "a", embedding: makeEmbedding([1, 2, 3, 4]) },
      { text: "b", embedding: makeEmbedding([5, 6, 7, 8]) },
    ]);
    await persistence.persist(persistParams());

    const all = await core.db!.exec("SELECT COUNT(*) FROM chunks");
    assert.equal(all[0].values[0][0], 2);
  });

  it("removes old chunks on update", async () => {
    mockEmbedder.setChunked([{ text: "old", embedding: makeEmbedding([1, 1, 1, 1]) }]);
    await persistence.persist(persistParams());

    mockEmbedder.setChunked([
      { text: "new1", embedding: makeEmbedding([2, 2, 2, 2]) },
      { text: "new2", embedding: makeEmbedding([3, 3, 3, 3]) },
    ]);
    await persistence.persist(persistParams());

    const all = await core.db!.exec("SELECT COUNT(*) FROM chunks");
    assert.equal(all[0].values[0][0], 2, "only 2 new chunks, old 1 should be gone");
  });

  it("tracks deleted chunk count", async () => {
    mockEmbedder.setChunked([
      { text: "a", embedding: makeEmbedding([1]) },
      { text: "b", embedding: makeEmbedding([2]) },
      { text: "c", embedding: makeEmbedding([3]) },
    ]);
    await persistence.persist(persistParams());

    const before = maintenance.deletedChunksCount;

    mockEmbedder.setChunked([{ text: "new", embedding: makeEmbedding([4]) }]);
    await persistence.persist(persistParams());

    assert.equal(maintenance.deletedChunksCount, before + 3, "should track 3 deleted chunks");
  });

  it("sets file status to indexed after success", async () => {
    mockEmbedder.setChunked([{ text: "x", embedding: makeEmbedding([1]) }]);
    await persistence.persist(persistParams());

    const st = await core.db!.exec("SELECT status, indexed_at FROM files WHERE file_path='file.md'");
    assert.equal(st[0].values[0][0], "indexed");
    assert.ok(st[0].values[0][1] != null);
    assert.ok((st[0].values[0][1] as number) > 0);
  });

  it("emits file:indexed event on success", async () => {
    const events: any[] = [];
    eventBus.on("file:indexed", (p) => { events.push(p); });

    mockEmbedder.setChunked([{ text: "x", embedding: makeEmbedding([1]) }]);
    await persistence.persist(persistParams());

    assert.equal(events.length, 1);
    assert.equal(events[0].filePath, "file.md");
    assert.equal(events[0].chunkCount, 1);
  });

  it("emits indexing:error and marks failed on error", async () => {
    const errorEvents: any[] = [];
    eventBus.on("indexing:error", (p) => { errorEvents.push(p); });

    mockEmbedder.setChunked(Array.from({ length: 600 }, (_, i) => ({
      text: `c${i}`, embedding: makeEmbedding([i, 0, 0, 0]),
    })));

    let err: Error | null = null;
    try { await persistence.persist(persistParams()); } catch (e: any) { err = e; }

    assert.ok(err);
    assert.match(err!.message, /600 chunks/);
    assert.equal(errorEvents.length, 1);
    assert.equal(errorEvents[0].filePath, "file.md");
    assert.match(errorEvents[0].error, /600 chunks/);

    // markFailed should persist outside transaction
    const st = await core.db!.exec("SELECT status FROM files WHERE file_path='file.md'");
    assert.equal(st[0].values[0][0], "failed");
  });
});

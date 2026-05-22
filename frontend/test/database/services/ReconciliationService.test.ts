import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/ReconciliationService.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { EmbeddingSerializer } from "@database/infrastructure/EmbeddingSerializer";
import { FileRepository } from "@database/repositories/FileRepository";
import { ChunkRepository } from "@database/repositories/ChunkRepository";
import { MaintenanceService } from "@database/services/MaintenanceService";
import { ReindexScheduler } from "@database/services/ReindexScheduler";
import { DbEventBus } from "@database/EventBus";
import { IndexingService } from "@database/services/indexing/IndexingService";
import { ReconciliationService } from "@database/services/ReconciliationService";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "recon-test";

class MockEmbedder {
  model = "test-model";
  dim = 4;
  private chunked: Array<{ text: string; embedding: Float32Array }> = [];
  setChunked(c: Array<{ text: string; embedding: Float32Array }>) { this.chunked = c; }
  getDimensions() { return this.dim; }
  embedChunked() { return Promise.resolve(this.chunked); }
}

function makeEmbedding(v: number[]): Float32Array { return new Float32Array(v); }

describe("ReconciliationService", () => {
  let core: TauriDbConnection;
  let reconciliation: ReconciliationService;
  let mockEmbedder: MockEmbedder;
  let files: FileRepository;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = new TauriDbConnection(adapter as any, PLUGIN_DIR, "test");
    const schema = new SchemaManager(core);
    await schema.initialize();
    const ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    const chunks = new ChunkRepository(core, ser);
    const maintenance = new MaintenanceService(core);
    const scheduler = new ReindexScheduler(core);
    const eventBus = new DbEventBus();
    mockEmbedder = new MockEmbedder();
    const indexing = new IndexingService(core, files, chunks, maintenance, scheduler, eventBus);
    reconciliation = new ReconciliationService(core, files, indexing, maintenance, scheduler);
  });

  afterEach(async () => { try { await core.close(); } catch {} });

  it("reconcile() soft-deletes files missing from vault", async () => {
    // Add file to DB
    files.insertIndexing({
      cleanPath: "gone.md", fileName: "gone", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 4,
      chunkingVersion: "1", updatedAt: Date.now(),
    });

    // reconcile with empty vault
    await reconciliation.reconcile(
      mockEmbedder as any,
      async () => [],
      async () => { throw new Error("should not be called"); },
    );

    const info = files.getByPath("gone.md");
    assert.equal(info!.isDeleted, 1, "file should be soft-deleted");
  });

  it("reconcile() indexes new vault files not in DB", async () => {
    mockEmbedder.setChunked([{ text: "new file", embedding: makeEmbedding([1, 2, 3, 4]) }]);

    await reconciliation.reconcile(
      mockEmbedder as any,
      async () => ["new.md"],
      async (p) => p === "new.md" ? "content of new file" : "",
    );

    assert.ok(files.getByPath("new.md"), "new file should be indexed");
  });

  it("reconcile() reindexes changed files", async () => {
    // Pre-index a file with specific content
    mockEmbedder.setChunked([{ text: "v1", embedding: makeEmbedding([1, 1, 1, 1]) }]);
    const indexing = (reconciliation as any).indexing;
    await indexing.indexFile("change.md", "version 1", mockEmbedder as any);

    // Now reconcile with changed content
    mockEmbedder.setChunked([{ text: "v2", embedding: makeEmbedding([2, 2, 2, 2]) }]);
    await reconciliation.reconcile(
      mockEmbedder as any,
      async () => ["change.md"],
      async (p) => p === "change.md" ? "version 2" : "",
    );

    // Should have updated hash
    assert.ok(files.getByPath("change.md")!.fileHash !== null);
  });

  it("reconcile() skips unchanged file via mtime fast-path", async () => {
    // Index with mtime
    mockEmbedder.setChunked([{ text: "stable", embedding: makeEmbedding([1, 1, 1, 1]) }]);
    const indexing = (reconciliation as any).indexing;
    await indexing.indexFile("stable.md", "stable content", mockEmbedder as any, 1000);

    let readCalled = false;

    // reconcile with mtime callback returning same mtime
    await reconciliation.reconcile(
      mockEmbedder as any,
      async () => ["stable.md"],
      async () => { readCalled = true; return "different!"; },
      async () => 1000, // same mtime
    );

    // Mtime match should skip readFile
    assert.equal(readCalled, false, "readFile should not be called on mtime match");
  });

  it("reconcile() handles readFile throwing", async () => {
    files.insertIndexing({
      cleanPath: "ok.md", fileName: "ok", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 4,
      chunkingVersion: "1", updatedAt: Date.now(),
    });

    // Should not crash when readFile throws
    await reconciliation.reconcile(
      mockEmbedder as any,
      async () => ["bad.md"],
      async () => { throw new Error("disk error"); },
    );

    // bad.md should not be in DB (was never indexed)
    assert.equal(files.getByPath("bad.md"), null);
  });

  it("reindexAllFiles() forces reindex when force=true", async () => {
    mockEmbedder.setChunked([{ text: "forced", embedding: makeEmbedding([9, 9, 9, 9]) }]);

    await reconciliation.reindexAllFiles(
      mockEmbedder as any,
      async () => ["f.md"],
      async () => "content",
      true,
    );

    assert.ok(files.getByPath("f.md"), "should index with force");
  });
});

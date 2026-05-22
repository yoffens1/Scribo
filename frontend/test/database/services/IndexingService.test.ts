import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/IndexingService.test.ts
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
import { FileValidationService } from "@database/services/indexing/FileValidationService";
import { HashService } from "@database/services/indexing/HashService";
import { EmbeddingPersistenceService } from "@database/services/indexing/EmbeddingPersistenceService";
import { Embedder } from "@ai/embedding/Embedder";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";
import * as path from "path";

const PLUGIN_DIR = "indexing-test";

/** Mock embedder with controllable outputs. */
class MockEmbedder {
  model = "mock-model";
  dim = 4;

  private chunked: Array<{ text: string; embedding: Float32Array }> = [];

  getDimensions(): number { return this.dim; }

  /** Pre-set what embedChunked will return. */
  setChunked(chunks: Array<{ text: string; embedding: Float32Array }>): void {
    this.chunked = chunks;
  }

  embedChunked(_content: string): Promise<Array<{ text: string; embedding: Float32Array }>> {
    return Promise.resolve(this.chunked);
  }
}

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

function makeEmbedding(values: number[]): Float32Array {
  return new Float32Array(values);
}

describe("IndexingService", () => {
  let adapter: FakeDataAdapter;
  let core: TauriDbConnection;
  let indexing: IndexingService;
  let mockEmbedder: MockEmbedder;
  let files: FileRepository;
  let maintenance: MaintenanceService;
  let eventBus: DbEventBus;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    core = createCore(adapter);
    const schema = new SchemaManager(core);
    await schema.initialize();

    const ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    const chunks = new ChunkRepository(core, ser);
    maintenance = new MaintenanceService(core);
    const scheduler = new ReindexScheduler(core);
    eventBus = new DbEventBus();
    mockEmbedder = new MockEmbedder();
    indexing = new IndexingService(
      core, files, chunks, maintenance, scheduler, eventBus,
    );
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  // ── New file indexing ──

  it("indexFile() creates file record and chunks for new file", async () => {
    mockEmbedder.setChunked([
      { text: "chunk 1", embedding: makeEmbedding([1, 2, 3, 4]) },
      { text: "chunk 2", embedding: makeEmbedding([5, 6, 7, 8]) },
    ]);

    await indexing.indexFile("new.md", "content here", mockEmbedder as any);

    const info = files.getByPath("new.md");
    assert.ok(info, "file should exist");
    assert.equal(info!.model, "mock-model");

    // Chunks stored
    const all = await core.db!.exec("SELECT COUNT(*) FROM chunks");
    assert.equal(all[0].values[0][0], 2);

    // Status indexed
    const st = await core.db!.exec("SELECT status, indexed_at FROM files WHERE file_path='new.md'");
    assert.equal(st[0].values[0][0], "indexed");
    assert.ok(st[0].values[0][1] != null, "indexed_at should be set");
    assert.ok((st[0].values[0][1] as number) > 0, "indexed_at should be set");

    // event emitted
    const events: any[] = [];
    eventBus.on("file:indexed", (p) => { events.push(p); });    // Already emitted — verify by re-indexing another file
    mockEmbedder.setChunked([{ text: "c", embedding: makeEmbedding([1, 2, 3, 4]) }]);
    await indexing.indexFile("other.md", "y", mockEmbedder as any);
    assert.equal(events.length, 1);
    assert.equal(events[0].filePath, "other.md");
    assert.equal(events[0].chunkCount, 1);
  });

  // ── Update existing file ──

  it("indexFile() updates existing file and replaces chunks", async () => {
    mockEmbedder.setChunked([
      { text: "old chunk", embedding: makeEmbedding([1, 1, 1, 1]) },
    ]);
    await indexing.indexFile("update.md", "v1", mockEmbedder as any);

    const id1 = files.getByPath("update.md")!.fileId;
    assert.equal(await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [id1])[0].values[0][0], 1);

    // Update with new content
    mockEmbedder.setChunked([
      { text: "new a", embedding: makeEmbedding([2, 2, 2, 2]) },
      { text: "new b", embedding: makeEmbedding([3, 3, 3, 3]) },
    ]);
    await indexing.indexFile("update.md", "v2", mockEmbedder as any);

    // Same fileId, old chunks gone, new chunks present
    const id2 = files.getByPath("update.md")!.fileId;
    assert.equal(id1, id2);
    assert.equal(await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [id2])[0].values[0][0], 2);
  });

  // ── Skip unchanged ──

  it("indexFile() skips unchanged file (hash match)", async () => {
    mockEmbedder.setChunked([
      { text: "c", embedding: makeEmbedding([1, 2, 3, 4]) },
    ]);
    await indexing.indexFile("skip.md", "same content", mockEmbedder as any);

    const initialCount = await core.db!.exec("SELECT COUNT(*) FROM chunks")[0].values[0][0] as number;

    // Re-index same content
    await indexing.indexFile("skip.md", "same content", mockEmbedder as any);

    // No new chunks, no re-embed (mock wasn't called with new data)
    const finalCount = await core.db!.exec("SELECT COUNT(*) FROM chunks")[0].values[0][0] as number;
    assert.equal(finalCount, initialCount, "should not add chunks for unchanged file");
  });

  it("indexFile() skips unchanged via mtime match", async () => {
    mockEmbedder.setChunked([
      { text: "c", embedding: makeEmbedding([1, 2, 3, 4]) },
    ]);
    await indexing.indexFile("mtime.md", "content", mockEmbedder as any, 12345);

    const initialCount = await core.db!.exec("SELECT COUNT(*) FROM chunks")[0].values[0][0] as number;

    // Re-index with same mtime, same content
    mockEmbedder.setChunked([
      { text: "should not be used", embedding: makeEmbedding([9, 9, 9, 9]) },
    ]);
    await indexing.indexFile("mtime.md", "content", mockEmbedder as any, 12345);

    const finalCount = await core.db!.exec("SELECT COUNT(*) FROM chunks")[0].values[0][0] as number;
    assert.equal(finalCount, initialCount);
  });

  // ── Model drift triggers reindex ──

  it("indexFile() reindexes when embedding model changed", async () => {
    mockEmbedder.setChunked([
      { text: "c", embedding: makeEmbedding([1, 1, 1, 1]) },
    ]);
    await indexing.indexFile("drift.md", "content", mockEmbedder as any);

    // Change mock model name
    mockEmbedder.model = "new-model";
    mockEmbedder.setChunked([
      { text: "c", embedding: makeEmbedding([2, 2, 2, 2]) },
    ]);
    await indexing.indexFile("drift.md", "content", mockEmbedder as any);

    const info = files.getByPath("drift.md");
    assert.equal(info!.model, "new-model");
  });

  // ── Failed indexing marks failed ──

  it("indexFile() marks file as failed when too many chunks", async () => {
    // Create 501 chunks (exceeds MAX_CHUNKS_PER_FILE = 500)
    const chunked = Array.from({ length: 501 }, (_, i) => ({
      text: `chunk-${i}`,
      embedding: makeEmbedding([1, 2, 3, 4]),
    }));
    mockEmbedder.setChunked(chunked);

    let error: Error | null = null;
    try {
      await indexing.indexFile("toomany.md", "big content", mockEmbedder as any);
    } catch (e: any) {
      error = e;
    }

    assert.ok(error, "should throw on too many chunks");
    assert.match(error!.message, /501 chunks.*exceeding limit/);

    // markFailed creates the file row via upsert
    const row = await core.db!.exec("SELECT status, last_error FROM files WHERE file_path='toomany.md'");
    assert.equal(row[0].values[0][0], "failed");
  });

  // ── indexed_at updates ──

  it("indexFile() updates indexed_at on reindex", async () => {
    mockEmbedder.setChunked([{ text: "a", embedding: makeEmbedding([1, 2, 3, 4]) }]);
    await indexing.indexFile("idxat.md", "content", mockEmbedder as any);

    const first = await core.db!.exec("SELECT indexed_at FROM files WHERE file_path='idxat.md'")[0].values[0][0] as number;

    // Wait a tick then reindex
    await new Promise(r => setTimeout(r, 10));

    mockEmbedder.setChunked([{ text: "b", embedding: makeEmbedding([5, 6, 7, 8]) }]);
    // Change content to force reindex
    await indexing.indexFile("idxat.md", "new content", mockEmbedder as any);

    const second = await core.db!.exec("SELECT indexed_at FROM files WHERE file_path='idxat.md'")[0].values[0][0] as number;
    assert.ok(second > first, "indexed_at should update");
  });
});

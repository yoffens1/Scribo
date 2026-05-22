import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/repositories/FileRepository.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FileRepository } from "@database/repositories/FileRepository";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "file-repo-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

describe("FileRepository", () => {
  let adapter: FakeDataAdapter;
  let core: TauriDbConnection;
  let repo: FileRepository;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    core = createCore(adapter);
    const schema = new SchemaManager(core);
    await schema.initialize();
    repo = new FileRepository(core);
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  it("getByPath() returns null for non-existent file", async () => {
    assert.equal(await repo.getByPath("nonexistent.md"), null);
  });

  it("insertIndexing() creates a file row with status='indexing'", async () => {
    const id = await repo.insertIndexing({
      cleanPath: "test.md",
      fileName: "test",
      fileHash: "abc",
      fileMtime: 1000,
      embeddingModel: "test-model",
      embeddingDim: 1024,
      chunkingVersion: "1",
      updatedAt: 2000,
    });
    assert.ok(id > 0);

    const info = await repo.getByPath("test.md");
    assert.ok(info);
    assert.equal(info!.fileHash, "abc");
    assert.equal(info!.mtime, 1000);
    assert.equal(info!.model, "test-model");
    assert.equal(info!.chunkVersion, "1");
    assert.equal(info!.isDeleted, 0);

    // Verify status in DB
    const status = await core.db!.exec("SELECT status FROM files WHERE file_path='test.md'");
    assert.equal(status[0].values[0][0], "indexing");
  });

  it("updateIndexing() updates a file row", async () => {
    const id = await repo.insertIndexing({
      cleanPath: "update.md", fileName: "update", fileHash: "v1",
      fileMtime: 1, embeddingModel: "m1", embeddingDim: 512,
      chunkingVersion: "1", updatedAt: 100,
    });

    await repo.updateIndexing({
      fileHash: "v2", fileMtime: 2, embeddingModel: "m2",
      embeddingDim: 768, chunkingVersion: "2", updatedAt: 200,
      fileName: "updated", fileId: id,
    });

    const info = await repo.getByPath("update.md");
    assert.equal(info!.fileHash, "v2");
    assert.equal(info!.mtime, 2);
    assert.equal(info!.model, "m2");
    assert.equal(info!.chunkVersion, "2");
  });

  it("markIndexed() sets status to 'indexed' and clears last_error", async () => {
    const id = await repo.insertIndexing({
      cleanPath: "idx.md", fileName: "idx", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    // Manually set last_error first
    await core.db!.run("UPDATE files SET last_error = 'old error' WHERE file_id = ?", [id]);

    await repo.markIndexed(id);

    const row = await core.db!.exec("SELECT status, last_error, indexed_at FROM files WHERE file_id = ?", [id]);
    assert.equal(row[0].values[0][0], "indexed");
    assert.equal(row[0].values[0][1], null);
    assert.ok(row[0].values[0][2] != null, "indexed_at should be set");
    assert.ok((row[0].values[0][2] as number) > 0, "indexed_at should be set");
  });

  it("markFailed() sets status='failed' and last_error", async () => {
    const id = await repo.insertIndexing({
      cleanPath: "fail.md", fileName: "fail", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });

    await repo.markFailed("fail.md", "test error");

    const row = await core.db!.exec("SELECT status, last_error FROM files WHERE file_id = ?", [id]);
    assert.equal(row[0].values[0][0], "failed");
    assert.equal(row[0].values[0][1], "test error");
  });

  it("insertFailed() upserts a failed record", async () => {
    await repo.insertFailed({
      cleanPath: "fail2.md", fileName: "fail2", fileHash: "h2",
      fileMtime: 50, error: "too large", updatedAt: 500,
    });

    const info = await repo.getByPath("fail2.md");
    assert.ok(info);
    const row = await core.db!.exec("SELECT status, last_error FROM files WHERE file_path='fail2.md'");
    assert.equal(row[0].values[0][0], "failed");
    assert.equal(row[0].values[0][1], "too large");
  });

  it("softDelete() sets is_deleted = 1", async () => {
    await repo.insertIndexing({
      cleanPath: "sd.md", fileName: "sd", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });

    await repo.softDelete("sd.md", 999);

    const info = await repo.getByPath("sd.md");
    assert.equal(info!.isDeleted, 1);
  });

  it("restore() sets is_deleted = 0", async () => {
    await repo.insertIndexing({
      cleanPath: "rest.md", fileName: "rest", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    await repo.softDelete("rest.md", 100);

    await repo.restore("rest.md", 200);

    const info = await repo.getByPath("rest.md");
    assert.equal(info!.isDeleted, 0);
  });

  it("rename() updates file_path and file_name", async () => {
    await repo.insertIndexing({
      cleanPath: "old/name.md", fileName: "name", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });

    await repo.rename("old/name.md", "new/renamed.md", 300);

    assert.equal(await repo.getByPath("old/name.md"), null);
    const info = await repo.getByPath("new/renamed.md");
    assert.ok(info);
    assert.equal(await core.db!.exec("SELECT file_name FROM files WHERE file_path='new/renamed.md'")[0].values[0][0], "renamed.md");
  });

  it("exists() returns true/false correctly", async () => {
    assert.equal(await repo.exists("nope.md"), false);
    await repo.insertIndexing({
      cleanPath: "yep.md", fileName: "yep", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    assert.equal(await repo.exists("yep.md"), true);
  });

  it("hardDelete() removes the file row", async () => {
    await repo.insertIndexing({
      cleanPath: "gone.md", fileName: "gone", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });

    await repo.hardDelete("gone.md");

    assert.equal(await repo.getByPath("gone.md"), null);
  });

  it("countChunksForFile() returns 0 for file without chunks", async () => {
    await repo.insertIndexing({
      cleanPath: "empty.md", fileName: "empty", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    assert.equal(await repo.countChunksForFile("empty.md"), 0);
  });

  // ── Edge cases ──

  it("rename() does nothing when file does not exist (no-op UPDATE)", async () => {
    (repo as any).rename("nope.md", "nope2.md", 100);
    // Should not throw, and no file should appear
    assert.equal(await repo.getByPath("nope.md"), null);
    assert.equal(await repo.getByPath("nope2.md"), null);
  });

  it("hardDelete() cascades to chunks via foreign keys", async () => {
    const id = await repo.insertIndexing({
      cleanPath: "cascade.md", fileName: "cascade", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    // Insert a chunk manually
    await core.db!.run("PRAGMA foreign_keys = ON");
    await core.db!.run("CREATE TABLE IF NOT EXISTS chunks (chunk_id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE, chunk_index INTEGER NOT NULL, chunk_text TEXT, token_count INTEGER, embedding BLOB NOT NULL, metadata TEXT, UNIQUE(file_id, chunk_index))");
    const emb = new Float32Array([1, 2, 3]);
    const stmt = core.getStmt("INSERT INTO chunks (file_id, chunk_index, chunk_text, token_count, embedding) VALUES (?, 0, 'text', 10, ?)");
    stmt.bind([id, new Uint8Array(emb.buffer)]);
    stmt.step();
    stmt.reset();

    assert.equal(await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [id])[0].values[0][0], 1);

    await repo.hardDelete("cascade.md");

    assert.equal(await repo.getByPath("cascade.md"), null);
    assert.equal(await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [id])[0].values[0][0], 0, "chunks should cascade-delete");
  });

  it("getAllFiles() returns empty array for empty DB", async () => {
    // Need a fresh repo on empty DB
    assert.equal(await repo.getAllFiles().length, 0);
  });

  it("getFilesMap() returns empty Map for empty DB", async () => {
    assert.equal(await repo.getFilesMap().size, 0);
  });

  it("getAllFiles() returns all file rows", async () => {
    await repo.insertIndexing({
      cleanPath: "a.md", fileName: "a", fileHash: "h1",
      fileMtime: 1, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });
    await repo.insertIndexing({
      cleanPath: "b.md", fileName: "b", fileHash: "h2",
      fileMtime: 2, embeddingModel: "m2", embeddingDim: 512,
      chunkingVersion: "2", updatedAt: 20,
    });

    const all = await repo.getAllFiles();
    assert.equal(all.length, 2);
    assert.equal(all[0].filePath, "a.md");
    assert.equal(all[1].filePath, "b.md");
  });

  it("getFilesMap() returns correct Map", async () => {
    await repo.insertIndexing({
      cleanPath: "x.md", fileName: "x", fileHash: "h",
      fileMtime: 5, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: 10,
    });

    const map = await repo.getFilesMap();
    assert.equal(map.size, 1);
    const info = map.get("x.md")!;
    assert.equal(info.isDeleted, false);
    assert.equal(info.mtime, 5);
    assert.equal(info.model, "m");
    assert.equal(info.chunkVer, "1");
  });
});

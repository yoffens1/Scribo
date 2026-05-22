import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/FileValidationService.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FileRepository } from "@database/repositories/FileRepository";
import { FileValidationService } from "@database/services/indexing/FileValidationService";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "fvs-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

describe("FileValidationService", () => {
  let adapter: FakeDataAdapter;
  let core: TauriDbConnection;
  let files: FileRepository;
  let validation: FileValidationService;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    core = createCore(adapter);
    const schema = new SchemaManager(core);
    await schema.initialize();
    files = new FileRepository(core);
    validation = new FileValidationService(core, files);
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  // ── hasFileChanged() ──

  it("hasFileChanged() returns true when file does not exist", async () => {
    assert.equal(validation.hasFileChanged("new.md", "hash1"), true);
  });

  it("hasFileChanged() returns true when hash differs", async () => {
    files.insertIndexing({
      cleanPath: "file.md", fileName: "file", fileHash: "old-hash",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.hasFileChanged("file.md", "new-hash"), true);
  });

  it("hasFileChanged() returns true when model differs", async () => {
    files.insertIndexing({
      cleanPath: "file2.md", fileName: "file2", fileHash: "h",
      fileMtime: null, embeddingModel: "old-model", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.hasFileChanged("file2.md", "h", "new-model"), true);
  });

  it("hasFileChanged() returns true when chunk version differs", async () => {
    files.insertIndexing({
      cleanPath: "file3.md", fileName: "file3", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.hasFileChanged("file3.md", "h", undefined, "2"), true);
  });

  it("hasFileChanged() returns false when nothing changed", async () => {
    files.insertIndexing({
      cleanPath: "same.md", fileName: "same", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.hasFileChanged("same.md", "h", "m", "1"), false);
  });

  it("hasFileChanged() returns true for soft-deleted files", async () => {
    files.insertIndexing({
      cleanPath: "deleted.md", fileName: "deleted", fileHash: "h",
      fileMtime: null, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    files.softDelete("deleted.md", Date.now());
    assert.equal(validation.hasFileChanged("deleted.md", "h"), true);
  });

  // ── canSkipByMtime() ──

  it("canSkipByMtime() returns true when mtime, hash, model, version all match", async () => {
    files.insertIndexing({
      cleanPath: "mtime.md", fileName: "mtime", fileHash: "h",
      fileMtime: 5000, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.canSkipByMtime("mtime.md", "h", "m", "1", 5000), true);
  });

  it("canSkipByMtime() returns false when mtime differs", async () => {
    files.insertIndexing({
      cleanPath: "mtime2.md", fileName: "mtime2", fileHash: "h",
      fileMtime: 100, embeddingModel: "m", embeddingDim: 256,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    assert.equal(validation.canSkipByMtime("mtime2.md", "h", "m", "1", 200), false);
  });

  it("canSkipByMtime() returns false when file not in DB", async () => {
    assert.equal(validation.canSkipByMtime("nope.md", "h", "m", "1", 100), false);
  });

  // ── validateSize() ──

  it("validateSize() returns false for small files", async () => {
    const skipped = await validation.validateSize({
      filePath: "small.md",
      content: "tiny",
      cleanPath: "small.md",
      fileName: "small",
      fileHash: "h",
      fileMtime: null,
    });
    assert.equal(skipped, false);
  });

  it("validateSize() returns true and records failed for files exceeding MAX_FILE_SIZE", async () => {
    // MAX_FILE_SIZE = 5 MB
    const bigContent = "x".repeat(6 * 1024 * 1024); // 6 MB
    const skipped = await validation.validateSize({
      filePath: "huge.md",
      content: bigContent,
      cleanPath: "huge.md",
      fileName: "huge",
      fileHash: "h",
      fileMtime: null,
    });
    assert.equal(skipped, true);

    const row = await core.db!.exec("SELECT status, last_error FROM files WHERE file_path='huge.md'");
    assert.equal(row[0].values[0][0], "failed");
    assert.equal(row[0].values[0][1], "File too large");
  });

  // ── validateDimension() ──

  it("validateDimension() returns false for dim within limit", async () => {
    const skipped = await validation.validateDimension({
      filePath: "ok.md",
      embeddingDim: 1024,
      cleanPath: "ok.md",
      fileName: "ok",
      fileHash: "h",
      fileMtime: null,
    });
    assert.equal(skipped, false);
  });

  it("validateDimension() returns true and records failed for dim exceeding MAX_EMBEDDING_DIM", async () => {
    const skipped = await validation.validateDimension({
      filePath: "bigdim.md",
      embeddingDim: 5000,
      cleanPath: "bigdim.md",
      fileName: "bigdim",
      fileHash: "h",
      fileMtime: null,
    });
    assert.equal(skipped, true);

    const row = await core.db!.exec("SELECT status, last_error FROM files WHERE file_path='bigdim.md'");
    assert.equal(row[0].values[0][0], "failed");
    assert.equal(row[0].values[0][1], "Embedding dimension too large");
  });
});

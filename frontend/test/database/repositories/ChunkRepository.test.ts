import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/repositories/ChunkRepository.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { EmbeddingSerializer } from "@database/infrastructure/EmbeddingSerializer";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FileRepository } from "@database/repositories/FileRepository";
import { ChunkRepository } from "@database/repositories/ChunkRepository";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "chunk-repo-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

function makeEmbedding(values: number[]): Float32Array {
  return new Float32Array(values);
}

describe("ChunkRepository", () => {
  let adapter: FakeDataAdapter;
  let core: TauriDbConnection;
  let files: FileRepository;
  let chunks: ChunkRepository;
  let ser: EmbeddingSerializer;
  let fileId: number;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    core = createCore(adapter);
    const schema = new SchemaManager(core);
    await schema.initialize();
    ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    chunks = new ChunkRepository(core, ser);

    // Create a test file
    fileId = files.insertIndexing({
      cleanPath: "test.md", fileName: "test", fileHash: "h",
      fileMtime: 1, embeddingModel: "m", embeddingDim: 1024,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  // ── insertChunks() ──

  it("insertChunks() stores prepared rows", async () => {
    const rows = [
      { chunkIndex: 0, text: "hello", tokens: 5, embedding: ser.serialize(makeEmbedding([1, 2, 3, 4])) },
      { chunkIndex: 1, text: "world", tokens: 5, embedding: ser.serialize(makeEmbedding([5, 6, 7, 8])) },
    ];
    await chunks.insertChunks(fileId, rows);

    const count = await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [fileId]);
    assert.equal(count[0].values[0][0], 2);
  });

  it("insertChunks() preserves Float32Array values roundtrip", async () => {
    const original = makeEmbedding([1.0, 2.5, -3.0, 4.0]);
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "x", tokens: 3, embedding: ser.serialize(original) },
    ]);

    const result = await chunks.getByFilePath("test.md");
    assert.equal(result.length, 1);
    assert.equal(result[0].embedding!.length, 4);
    assert.equal(result[0].embedding![0], 1.0);
    assert.equal(result[0].embedding![3], 4.0);
  });

  it("insertChunks() stores token_count correctly", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "short", tokens: 100, embedding: ser.serialize(makeEmbedding([1])) },
    ]);

    const row = await core.db!.exec("SELECT token_count FROM chunks WHERE file_id = ?", [fileId]);
    assert.equal(row[0].values[0][0], 100);
  });

  it("insertChunks() handles empty rows array", async () => {
    await chunks.insertChunks(fileId, []);
    const count = await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [fileId]);
    assert.equal(count[0].values[0][0], 0);
  });

  // ── deleteByFileId() ──

  it("deleteByFileId() removes all chunks for file and returns count", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "a", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
      { chunkIndex: 1, text: "b", tokens: 1, embedding: ser.serialize(makeEmbedding([2])) },
      { chunkIndex: 2, text: "c", tokens: 1, embedding: ser.serialize(makeEmbedding([3])) },
    ]);

    const deleted = await chunks.deleteByFileId(fileId);
    assert.equal(deleted, 3);

    const remaining = await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [fileId]);
    assert.equal(remaining[0].values[0][0], 0);
  });

  it("deleteByFileId() returns 0 when no chunks", async () => {
    const deleted = await chunks.deleteByFileId(fileId);
    assert.equal(deleted, 0);
  });

  // ── getByFilePath() ──

  it("getByFilePath() returns chunks ordered by chunk_index", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 1, text: "second", tokens: 1, embedding: ser.serialize(makeEmbedding([2])) },
      { chunkIndex: 0, text: "first", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
    ]);

    const result = await chunks.getByFilePath("test.md");
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkText, "first");
    assert.equal(result[1].chunkText, "second");
  });

  it("getByFilePath() excludes soft-deleted files by default", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "visible", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
    ]);
    files.softDelete("test.md", Date.now());

    const result = await chunks.getByFilePath("test.md");
    assert.equal(result.length, 0);

    // includeDeleted = true should show them
    const result2 = await chunks.getByFilePath("test.md", true);
    assert.equal(result2.length, 1);
  });

  it("getByFilePath() returns empty array for non-existent file", async () => {
    assert.deepEqual(await chunks.getByFilePath("nope.md"), []);
  });

  // ── getByFileName() ──

  it("getByFileName() returns chunks with filePath", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "by name", tokens: 2, embedding: ser.serialize(makeEmbedding([9])) },
    ]);

    const result = await chunks.getByFileName("test");
    assert.equal(result.length, 1);
    assert.equal(result[0].filePath, "test.md");
    assert.equal(result[0].chunkText, "by name");
  });

  it("getByFileName() matches by name without extension", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "match", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
    ]);

    // The file was created with fileName="test", path="test.md"
    // getByFileName searches by file_name column (without extension)
    assert.equal(await chunks.getByFileName("test").length, 1);
    // Searching by full path name won't match (file_name is "test", not "test.md")
    assert.equal(await chunks.getByFileName("test.md").length, 0);
  });

  // ── getAll() ──

  it("getAll() returns all chunks across all files", async () => {
    const file2 = files.insertIndexing({
      cleanPath: "other.md", fileName: "other", fileHash: "h2",
      fileMtime: 2, embeddingModel: "m", embeddingDim: 1024,
      chunkingVersion: "1", updatedAt: Date.now(),
    });

    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "f1", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
    ]);
    await chunks.insertChunks(file2, [
      { chunkIndex: 0, text: "f2", tokens: 1, embedding: ser.serialize(makeEmbedding([2])) },
    ]);

    const all = await chunks.getAll();
    assert.equal(all.length, 2);
    assert.equal(all[0].filePath, "other.md"); // alphabetical
    assert.equal(all[1].filePath, "test.md");
  });

  it("getAll() includes chunkId", async () => {
    await chunks.insertChunks(fileId, [
      { chunkIndex: 0, text: "id check", tokens: 1, embedding: ser.serialize(makeEmbedding([1])) },
    ]);

    const all = await chunks.getAll();
    assert.equal(all.length, 1);
    assert.ok(all[0].chunkId > 0);
  });

  // ── Edge: too many chunks ──

  it("insertChunks() does NOT validate max chunks (validation moved to service layer)", async () => {
    // In the refactored code, max-chunks validation is in EmbeddingPersistenceService,
    // not in ChunkRepository. This test confirms the repo blindly inserts.
    const rows = Array.from({ length: 600 }, (_, i) => ({
      chunkIndex: i,
      text: `chunk-${i}`,
      tokens: 1,
      embedding: ser.serialize(makeEmbedding([i])),
    }));
    // Should not throw — repo doesn't validate
    await chunks.insertChunks(fileId, rows);
    const count = await core.db!.exec("SELECT COUNT(*) FROM chunks WHERE file_id = ?", [fileId]);
    assert.equal(count[0].values[0][0], 600);
  });
});

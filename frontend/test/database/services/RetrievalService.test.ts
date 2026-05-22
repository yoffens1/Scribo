import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/services/RetrievalService.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { EmbeddingSerializer } from "@database/infrastructure/EmbeddingSerializer";
import { FileRepository } from "@database/repositories/FileRepository";
import { ChunkRepository } from "@database/repositories/ChunkRepository";
import { LegacyRetrievalAdapter } from "@retrieval/LegacyRetrievalAdapter";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "retrieval-test";

function makeEmbedding(v: number[]): Float32Array { return new Float32Array(v); }

describe("RetrievalService", () => {
  let core: TauriDbConnection;
  let legacy: LegacyRetrievalAdapter;
  let files: FileRepository;
  let chunks: ChunkRepository;

  beforeEach(async () => {
    const adapter = new FakeDataAdapter();
    core = new TauriDbConnection(adapter as any, PLUGIN_DIR, "test");
    const schema = new SchemaManager(core);
    await schema.initialize();
    const ser = new EmbeddingSerializer();
    files = new FileRepository(core);
    chunks = new ChunkRepository(core, ser);
    legacy = new LegacyRetrievalAdapter(chunks);

    // Create test files with chunks
    const f1 = files.insertIndexing({
      cleanPath: "a.md", fileName: "a", fileHash: "h1",
      fileMtime: 1, embeddingModel: "m", embeddingDim: 4,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    files.markIndexed(f1);
    await chunks.insertChunks(f1, [
      { chunkIndex: 0, text: "alpha", tokens: 5, embedding: ser.serialize(makeEmbedding([1, 2, 3, 4])) },
      { chunkIndex: 1, text: "beta", tokens: 4, embedding: ser.serialize(makeEmbedding([5, 6, 7, 8])) },
    ]);

    const f2 = files.insertIndexing({
      cleanPath: "b.md", fileName: "b", fileHash: "h2",
      fileMtime: 2, embeddingModel: "m", embeddingDim: 4,
      chunkingVersion: "1", updatedAt: Date.now(),
    });
    files.markIndexed(f2);
    await chunks.insertChunks(f2, [
      { chunkIndex: 0, text: "gamma", tokens: 5, embedding: ser.serialize(makeEmbedding([9, 10, 11, 12])) },
    ]);
  });

  afterEach(async () => { try { await core.close(); } catch {} });

  it("getFileChunks() returns chunks for a file", async () => {
    const result = legacy.getFileChunks("a.md");
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkText, "alpha");
    assert.equal(result[1].chunkText, "beta");
  });

  it("getFileChunks() returns empty for non-existent file", async () => {
    const result = legacy.getFileChunks("nope.md");
    assert.deepEqual(result, []);
  });

  it("getChunksByFileName() returns chunks with filePath", async () => {
    const result = legacy.getChunksByFileName("b");
    assert.equal(result.length, 1);
    assert.equal(result[0].filePath, "b.md");
    assert.equal(result[0].chunkText, "gamma");
  });

  it("getAllChunks() returns all chunks ordered by file_path", async () => {
    const all = legacy.getAllChunks();
    assert.equal(all.length, 3);
    assert.equal(all[0].filePath, "a.md");
    assert.equal(all[2].filePath, "b.md");
  });

  it("includeDeleted=false excludes soft-deleted files", async () => {
    files.softDelete("a.md", Date.now());
    const all = legacy.getAllChunks(false);
    assert.equal(all.length, 1, "only b.md should be visible");
    assert.equal(all[0].filePath, "b.md");
  });

  it("includeDeleted=true includes soft-deleted files", async () => {
    files.softDelete("a.md", Date.now());
    const all = legacy.getAllChunks(true);
    assert.equal(all.length, 3, "all chunks should be visible");
  });

  it("getChunksByFileOrName() routes to file path when slash present", async () => {
    const result = legacy.getChunksByFileOrName("sub/a.md");
    // "sub/a.md" doesn't exist, returns empty
    assert.deepEqual(result, []);
  });

  it("getChunksByFileOrName() routes to file name when no slash", async () => {
    const result = legacy.getChunksByFileOrName("a");
    assert.equal(result.length, 2);
    assert.equal(result[0].chunkText, "alpha");
  });

  // ── Query API ──

  it("search() with filePath scope", async () => {
    const result = legacy.search({ filePath: "a.md" });
    assert.equal(result.length, 2);
    assert.equal(result[0].filePath, "a.md");
  });

  it("search() with fileName scope", async () => {
    const result = legacy.search({ fileName: "b" });
    assert.equal(result.length, 1);
    assert.equal(result[0].filePath, "b.md");
  });

  it("search() without scope returns all", async () => {
    const result = legacy.search({});
    assert.equal(result.length, 3);
  });

  it("search() with limit/offset pagination", async () => {
    const result = legacy.search({ limit: 1, offset: 1 });
    assert.equal(result.length, 1);
    assert.equal(result[0].chunkText, "beta");
  });

  it("search() returns uniform result shape (always has filePath)", async () => {
    const result = legacy.search({ filePath: "a.md" });
    for (const r of result) {
      assert.ok("filePath" in r);
      assert.ok("embedding" in r);
    }
  });
});

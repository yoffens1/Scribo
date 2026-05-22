import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/infrastructure/SchemaManager.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "schema-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

describe("SchemaManager", () => {
  let adapter: FakeDataAdapter;
  let core: TauriDbConnection;
  let schema: SchemaManager;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
    core = createCore(adapter);
    schema = new SchemaManager(core);
  });

  afterEach(async () => {
    try { await core.close(); } catch {}
  });

  it("initialize() creates all tables", async () => {
    await schema.initialize();

    const tables = await core.db!.exec(
      "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
    );
    const names = tables[0].values.map(r => r[0]);
    assert.ok(names.includes("meta"), "meta table should exist");
    assert.ok(names.includes("files"), "files table should exist");
    assert.ok(names.includes("chunks"), "chunks table should exist");
  });

  it("initialize() creates indexes", async () => {
    await schema.initialize();

    const indexes = await core.db!.exec(
      "SELECT name FROM sqlite_master WHERE type='index' ORDER BY name",
    );
    const names = indexes[0].values.map(r => r[0]);
    assert.ok(names.includes("idx_chunks_file_id"), "idx_chunks_file_id should exist");
    assert.ok(names.includes("idx_files_path"), "idx_files_path should exist");
  });

  it("initialize() sets schema_version to 2", async () => {
    await schema.initialize();

    const result = await core.db!.exec("SELECT value FROM meta WHERE key = 'schema_version'");
    assert.equal(result[0].values[0][0], "5");
  });

  it("initialize() applies migrations (v1→v2 adds columns)", async () => {
    await schema.initialize();

    const cols = await core.db!.exec("PRAGMA table_info(files)");
    const colNames = cols[0].values.map(r => r[1]);
    assert.ok(colNames.includes("is_deleted"), "is_deleted column missing");
    assert.ok(colNames.includes("embedding_model"), "embedding_model column missing");
    assert.ok(colNames.includes("chunking_version"), "chunking_version column missing");
    assert.ok(colNames.includes("embedding_dimension"), "embedding_dimension column missing");
    assert.ok(colNames.includes("file_mtime"), "file_mtime column missing");
    assert.ok(colNames.includes("status"), "status column missing");
    assert.ok(colNames.includes("last_error"), "last_error column missing");

    const chunkCols = await core.db!.exec("PRAGMA table_info(chunks)");
    const chunkColNames = chunkCols[0].values.map(r => r[1]);
    assert.ok(chunkColNames.includes("token_count"), "token_count column missing");
  });

  it("initialize() recovers interrupted indexing (status='indexing' → 'failed')", async () => {
    // Manually create core with full schema, insert 'indexing' rows, then re-init
    await core.initialize();
    await core.db!.run(`CREATE TABLE IF NOT EXISTS files (
      file_id INTEGER PRIMARY KEY AUTOINCREMENT,
      file_path TEXT NOT NULL UNIQUE,
      file_name TEXT NOT NULL,
      file_hash TEXT,
      file_mtime INTEGER,
      embedding_model TEXT DEFAULT 'unknown',
      embedding_dimension INTEGER,
      chunking_version TEXT DEFAULT '1',
      is_deleted INTEGER DEFAULT 0,
      status TEXT DEFAULT 'indexed',
      last_error TEXT,
      updated_at INTEGER,
      indexed_at INTEGER
    )`);
    await core.db!.run("INSERT INTO files (file_path, file_name, status) VALUES ('a.md', 'a', 'indexing')");
    await core.db!.run("INSERT INTO files (file_path, file_name, status) VALUES ('b.md', 'b', 'indexed')");
    core.markDirty();
    await core.flush();
    await core.close();

    // Re-initialize through SchemaManager
    const core2 = createCore(adapter);
    const schema2 = new SchemaManager(core2);
    await schema2.initialize();

    const result = core2.db!.exec("SELECT file_path, status, last_error FROM files ORDER BY file_path");
    assert.equal(result[0].values[0][0], "a.md");
    assert.equal(result[0].values[0][1], "failed");
    assert.equal(result[0].values[0][2], "Interrupted indexing");
    assert.equal(result[0].values[1][0], "b.md");
    assert.equal(result[0].values[1][1], "indexed");
    assert.equal(result[0].values[1][2], null);
    await core2.close();
  });

  it("initialize() enables foreign keys", async () => {
    await schema.initialize();

    const fk = await core.db!.exec("PRAGMA foreign_keys");
    assert.equal(fk[0].values[0][0], 1, "foreign_keys should be ON");
  });

  it("initialize() is idempotent — missing tables are re-created", async () => {
    await schema.initialize();
    // Drop chunks table — simulate inconsistency
    await core.db!.run("DROP TABLE chunks");

    // Re-initialize — should re-create via IF NOT EXISTS
    const schema2 = new SchemaManager(core);
    await schema2.initialize();

    const tables = await core.db!.exec("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name");
    const names = tables[0].values.map(r => r[0]);
    assert.ok(names.includes("chunks"), "chunks table should be re-created");
  });

  it("migration re-run is idempotent (double initialize)", async () => {
    await schema.initialize();
    // Second initialize should not throw or duplicate
    await schema.initialize();

    const result = await core.db!.exec("SELECT value FROM meta WHERE key = 'schema_version'");
    assert.equal(result[0].values[0][0], "5");

    // Columns should still exist (migration didn't break anything)
    const cols = await core.db!.exec("PRAGMA table_info(files)");
    const colNames = cols[0].values.map(r => r[1]);
    assert.ok(colNames.includes("is_deleted"));
  });
});

import { initMock } from "@test/testing/tauriMock";
// @ts-ignore
const _ = initMock;
// src/test/database/infrastructure/Migration.test.ts
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import { TauriDbConnection } from "@database/infrastructure/TauriDbConnection";
import { SchemaManager } from "@database/infrastructure/schema/SchemaManager";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";

const PLUGIN_DIR = "migration-test";

function createCore(adapter?: FakeDataAdapter) {
  const a = adapter ?? new FakeDataAdapter();
  return new TauriDbConnection(".test-db", "test-model");
}

describe("Migrations", () => {
  let adapter: FakeDataAdapter;

  beforeEach(async () => {
    adapter = new FakeDataAdapter();
  });

  afterEach(async () => {});

  it("v1 → v2: old DB without new columns is migrated successfully", async () => {
    // Simulate a v1 DB (schema_version = 1, no v2 columns)
    const core = createCore(adapter);
    await core.initialize();

    // Create v1 schema manually
    await core.db!.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    await core.db!.run("INSERT INTO meta VALUES ('schema_version', '1')");
    await core.db!.run("CREATE TABLE IF NOT EXISTS files (file_id INTEGER PRIMARY KEY AUTOINCREMENT, file_path TEXT NOT NULL UNIQUE, file_name TEXT NOT NULL, file_hash TEXT)");
    await core.db!.run("CREATE TABLE IF NOT EXISTS chunks (chunk_id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE, chunk_index INTEGER NOT NULL, chunk_text TEXT, embedding BLOB NOT NULL, metadata TEXT, UNIQUE(file_id, chunk_index))");
    // Insert v1 data
    await core.db!.run("INSERT INTO files (file_path, file_name, file_hash) VALUES ('note.md', 'note', 'abc123')");
    core.markDirty();
    await core.flush();
    await core.close();

    // Now migrate via SchemaManager
    const core2 = createCore(adapter);
    const schema = new SchemaManager(core2);
    await schema.initialize();

    // Verify schema_version is 2
    const ver = core2.db!.exec("SELECT value FROM meta WHERE key='schema_version'");
    assert.equal(ver[0].values[0][0], "5");

    // Verify v2 columns exist
    const cols = core2.db!.exec("PRAGMA table_info(files)");
    const colNames = cols[0].values.map(r => r[1]);
    assert.ok(colNames.includes("is_deleted"), "is_deleted missing");
    assert.ok(colNames.includes("embedding_model"), "embedding_model missing");
    assert.ok(colNames.includes("status"), "status missing");
    assert.ok(colNames.includes("last_error"), "last_error missing");
    assert.ok(colNames.includes("updated_at"), "updated_at missing");
    assert.ok(colNames.includes("indexed_at"), "indexed_at missing");

    // Verify old data survived
    const rows = core2.db!.exec("SELECT file_path, file_name, file_hash FROM files");
    assert.equal(rows[0].values[0][0], "note.md");
    assert.equal(rows[0].values[0][1], "note");
    assert.equal(rows[0].values[0][2], "abc123");

    // Verify new columns have defaults
    const defaults = core2.db!.exec("SELECT is_deleted, embedding_model, status FROM files WHERE file_path='note.md'");
    assert.equal(defaults[0].values[0][0], 0);
    assert.equal(defaults[0].values[0][1], "unknown");
    assert.equal(defaults[0].values[0][2], "indexed");

    await core2.close();
  });

  it("v1 → v2: columns can be used after migration", async () => {
    const core = createCore(adapter);
    await core.initialize();

    // v1 schema
    await core.db!.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    await core.db!.run("INSERT INTO meta VALUES ('schema_version', '1')");
    await core.db!.run("CREATE TABLE IF NOT EXISTS files (file_id INTEGER PRIMARY KEY AUTOINCREMENT, file_path TEXT NOT NULL UNIQUE, file_name TEXT NOT NULL, file_hash TEXT)");
    await core.db!.run("CREATE TABLE IF NOT EXISTS chunks (chunk_id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE, chunk_index INTEGER NOT NULL, chunk_text TEXT, embedding BLOB NOT NULL, metadata TEXT, UNIQUE(file_id, chunk_index))");
    core.markDirty();
    await core.flush();
    await core.close();

    // Migrate
    const core2 = createCore(adapter);
    const schema = new SchemaManager(core2);
    await schema.initialize();

    // Write data using new columns
    core2.db!.run("INSERT INTO files (file_path, file_name, file_hash, is_deleted, embedding_model, embedding_dimension, chunking_version, status, last_error, updated_at, indexed_at) VALUES ('test.md', 'test', 'hash1', 0, 'test-model', 1024, '2', 'indexed', NULL, 1000, 2000)");
    const row = core2.db!.exec("SELECT embedding_dimension, chunking_version, updated_at, indexed_at FROM files WHERE file_path='test.md'");
    assert.equal(row[0].values[0][0], 1024);
    assert.equal(row[0].values[0][1], "2");
    assert.equal(row[0].values[0][2], 1000);
    assert.equal(row[0].values[0][3], 2000);

    await core2.close();
  });

  it("migration is idempotent — running twice does not break", async () => {
    // First migration
    const core = createCore(adapter);
    await core.initialize();
    await core.db!.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    await core.db!.run("INSERT INTO meta VALUES ('schema_version', '1')");
    await core.db!.run("CREATE TABLE IF NOT EXISTS files (file_id INTEGER PRIMARY KEY AUTOINCREMENT, file_path TEXT NOT NULL UNIQUE, file_name TEXT NOT NULL, file_hash TEXT)");
    await core.db!.run("CREATE TABLE IF NOT EXISTS chunks (chunk_id INTEGER PRIMARY KEY AUTOINCREMENT, file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE, chunk_index INTEGER NOT NULL, chunk_text TEXT, embedding BLOB NOT NULL, metadata TEXT, UNIQUE(file_id, chunk_index))");
    core.markDirty();
    await core.flush();
    await core.close();

    // Migrate twice
    for (let i = 0; i < 2; i++) {
      const c = createCore(adapter);
      const s = new SchemaManager(c);
      await s.initialize();
      const cols = c.db!.exec("PRAGMA table_info(files)");
      const colNames = cols[0].values.map(r => r[1]);
      assert.ok(colNames.includes("is_deleted"), `run ${i}: is_deleted missing`);
      assert.ok(colNames.includes("status"), `run ${i}: status missing`);
      await c.close();
    }
  });
});

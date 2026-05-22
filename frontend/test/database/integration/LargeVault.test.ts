// src/test/database/integration/LargeVault.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { VectorDatabase } from "@database/Database";
import { FakeDataAdapter } from "@test/testing/FakeDataAdapter";
import { PATH_TO_VAULT } from "@settings";
import * as fs from "fs/promises";
import * as path from "path";

const PLUGIN_DIR = "int-large";

class MockEmbedder {
  model = "mock";
  dim = 4;
  getDimensions() { return this.dim; }
  async embedChunked(content: string): Promise<Array<{ text: string; embedding: Float32Array }>> {
    const chunks: Array<{ text: string; embedding: Float32Array }> = [];
    let pos = 0;
    while (pos < content.length) {
      const chunk = content.slice(pos, pos + 50);
      const emb = new Float32Array(4);
      for (let i = 0; i < 4; i++) emb[i] = (chunk.length + i * 7) % 100;
      chunks.push({ text: chunk, embedding: emb });
      pos += 50;
    }
    return chunks;
  }
}

async function collectMdFiles(dir: string): Promise<string[]> {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    if (entry.name.startsWith(".")) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectMdFiles(full)));
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      files.push(full);
    }
  }
  return files;
}

describe("Integration: Large Vault (real files)", () => {
  const vaultRoot = PATH_TO_VAULT.inputVault;

  it("indexes all real .md files from input vault", async () => {
    const mdFiles = await collectMdFiles(vaultRoot);
    assert.ok(mdFiles.length > 0, `vault should have .md files, found ${mdFiles.length}`);

    const adapter = new FakeDataAdapter();
    const db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    const embedder = new MockEmbedder();
    await db.initialize();

    const filePaths: string[] = [];
    const contents: string[] = [];
    for (const fullPath of mdFiles.slice(0, 300)) {
      const rel = path.relative(vaultRoot, fullPath);
      const content = await fs.readFile(fullPath, "utf-8");
      if (content.length > 0) {
        filePaths.push(rel);
        contents.push(content);
      }
    }

    await db.addMdFiles(filePaths, contents, embedder as any);

    const all = await db.getAllChunks();
    assert.ok(all.length > 0, "should produce chunks from real content");

    // Verify every file has at least one chunk
    for (const fp of filePaths.slice(0, 10)) {
      const chunks = await db.getFileChunks(fp);
      assert.ok(chunks.length > 0, `${fp} should have chunks`);
    }

    await db.close();
  });

  it("retrieves correct embeddings from real content", async () => {
    const mdFiles = await collectMdFiles(vaultRoot);
    assert.ok(mdFiles.length >= 2);

    const adapter = new FakeDataAdapter();
    const db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    const embedder = new MockEmbedder();
    await db.initialize();

    // Index 2 real files
    const f1 = path.relative(vaultRoot, mdFiles[0]);
    const c1 = await fs.readFile(mdFiles[0], "utf-8");
    const f2 = path.relative(vaultRoot, mdFiles[1]);
    const c2 = await fs.readFile(mdFiles[1], "utf-8");

    embedder.embedChunked = async (content: string) => {
      // Return deterministic embeddings based on file hash
      const emb = new Float32Array(4);
      for (let i = 0; i < 4; i++) emb[i] = content.length % (i + 37);
      return [{ text: content.slice(0, 50), embedding: emb }];
    };
    await db.addMdFile(f1, c1, embedder as any);
    await db.addMdFile(f2, c2, embedder as any);

    const c1Chunks = await db.getFileChunks(f1);
    const c2Chunks = await db.getFileChunks(f2);
    assert.ok(c1Chunks.length > 0);
    assert.ok(c2Chunks.length > 0);

    // Different files should have different embeddings
    assert.notDeepEqual(
      Array.from(c1Chunks[0].embedding!),
      Array.from(c2Chunks[0].embedding!),
      "different files produce different embeddings",
    );

    await db.close();
  });

  it("close/reopen preserves real file data", async () => {
    const mdFiles = await collectMdFiles(vaultRoot);
    assert.ok(mdFiles.length > 0);

    const adapter = new FakeDataAdapter();
    const db1 = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    const embedder = new MockEmbedder();
    await db1.initialize();

    const rel = path.relative(vaultRoot, mdFiles[0]);
    const content = await fs.readFile(mdFiles[0], "utf-8");
    embedder.embedChunked = async () => [
      { text: "chunk", embedding: new Float32Array([1, 2, 3, 4]) },
    ];
    await db1.addMdFile(rel, content, embedder as any);

    const count1 = (await db1.getAllChunks()).length;
    await db1.close();

    const db2 = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    await db2.initialize();
    const count2 = (await db2.getAllChunks()).length;
    assert.equal(count2, count1, "all chunks should survive close/reopen");

    const chunks = await db2.getFileChunks(rel);
    assert.equal(chunks.length, 1);
    assert.deepEqual(Array.from(chunks[0].embedding!), [1, 2, 3, 4]);
    await db2.close();
  });

  it("reconcile detects real vault changes", async () => {
    const mdFiles = await collectMdFiles(vaultRoot);
    if (mdFiles.length < 2) return; // skip if vault too small

    const adapter = new FakeDataAdapter();
    const db = new VectorDatabase(adapter as any, PLUGIN_DIR, "test");
    const embedder = new MockEmbedder();
    await db.initialize();

    // Index first file only
    const rel = path.relative(vaultRoot, mdFiles[0]);
    const content = await fs.readFile(mdFiles[0], "utf-8");
    embedder.embedChunked = async () => [
      { text: "one", embedding: new Float32Array([1, 1, 1, 1]) },
    ];
    await db.addMdFile(rel, content, embedder as any);

    // reconcile with full vault — should detect second file as new
    embedder.embedChunked = async () => [
      { text: "discovered", embedding: new Float32Array([9, 9, 9, 9]) },
    ];
    await db.reconcile(
      embedder as any,
      async () => mdFiles.slice(0, 10).map(f => path.relative(vaultRoot, f)),
      async (fp) => {
        const fullPath = path.join(vaultRoot, fp);
        return fs.readFile(fullPath, "utf-8");
      },
    );

    const totalChunks = (await db.getAllChunks()).length;
    assert.ok(totalChunks >= 2, `should have at least 2 chunks from reconciled files, got ${totalChunks}`);
    await db.close();
  });
});

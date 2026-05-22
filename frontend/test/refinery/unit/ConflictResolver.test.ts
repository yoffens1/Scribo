// src/test/refinery/unit/ConflictResolver.test.ts
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ConflictResolver } from "@refinery/placement/ConflictResolver";
import { fakeFs } from "../helpers/fakeFs";

// Access private method via bracket notation
const callIncrementPath = (r: ConflictResolver, path: string, n: number): string =>
  (r as any).incrementPath(path, n);

describe("ConflictResolver.incrementPath", () => {
  const r = new ConflictResolver(fakeFs());

  it("adds suffix before extension", async () => {
    assert.strictEqual(callIncrementPath(r, "notes/foo.md", 1), "notes/foo-1.md");
  });

  it("handles no extension", async () => {
    assert.strictEqual(callIncrementPath(r, "notes/foo", 2), "notes/foo-2");
  });

  it("handles dots in folder name correctly (FIXED)", async () => {
    const result = callIncrementPath(r, "a.b/foo", 1);
    assert.strictEqual(result, "a.b/foo-1");
  });

  it("handles dotfile correctly", async () => {
    const result = callIncrementPath(r, "foo/.gitignore", 1);
    // .gitignore starts with dot — no extension, suffix at end
    assert.strictEqual(result, "foo/.gitignore-1");
  });

  it("handles multiple extensions", async () => {
    assert.strictEqual(callIncrementPath(r, "file.tar.gz", 1), "file.tar-1.gz");
  });

  it("handles n=0 (no-op case)", async () => {
    assert.strictEqual(callIncrementPath(r, "foo.md", 0), "foo-0.md");
  });
});

describe("ConflictResolver.resolve", () => {
  it("renames second decision targeting same path", async () => {
    const fs = fakeFs({});
    const r = new ConflictResolver(fs);
    const out = await r.resolve([
      { chunkHash: "a", outputPath: "foo.md", action: "create", reason: "" },
      { chunkHash: "b", outputPath: "foo.md", action: "create", reason: "" },
    ]);
    assert.strictEqual(out[0].outputPath, "foo.md");
    assert.strictEqual(out[1].outputPath, "foo-1.md");
  });

  it("skips existing files on disk", async () => {
    const fs = fakeFs({ "foo.md": "x", "foo-1.md": "y" });
    const r = new ConflictResolver(fs);
    const out = await r.resolve([
      { chunkHash: "a", outputPath: "foo.md", action: "create", reason: "" },
    ]);
    // foo.md exists, foo-1.md exists → foo-2.md
    assert.strictEqual(out[0].outputPath, "foo-2.md");
  });

  it("no rename when no conflicts", async () => {
    const fs = fakeFs({});
    const r = new ConflictResolver(fs);
    const out = await r.resolve([
      { chunkHash: "a", outputPath: "a.md", action: "create", reason: "" },
      { chunkHash: "b", outputPath: "b.md", action: "create", reason: "" },
    ]);
    assert.strictEqual(out[0].outputPath, "a.md");
    assert.strictEqual(out[1].outputPath, "b.md");
  });

  it("does not rename merge actions (FIXED)", async () => {
    const fs = fakeFs({ "foo.md": "existing content" });
    const r = new ConflictResolver(fs);
    const out = await r.resolve([
      { chunkHash: "a", outputPath: "foo.md", action: "merge", existingTarget: "foo.md", reason: "" },
    ]);
    // Merge actions intentionally target existing paths — should NOT be renamed
    assert.strictEqual(out[0].outputPath, "foo.md");
  });

  it("resolves triple conflict correctly", async () => {
    const fs = fakeFs({});
    const r = new ConflictResolver(fs);
    const out = await r.resolve([
      { chunkHash: "a", outputPath: "x.md", action: "create", reason: "" },
      { chunkHash: "b", outputPath: "x.md", action: "create", reason: "" },
      { chunkHash: "c", outputPath: "x.md", action: "create", reason: "" },
    ]);
    // New resolver: first wins, second gets -1, third gets -1 from base path increment
    assert.strictEqual(out[0].outputPath, "x.md");
    assert.ok(out[1].outputPath !== out[0].outputPath);
    assert.ok(out[2].outputPath !== out[0].outputPath);
    assert.ok(out[2].outputPath !== out[1].outputPath);
    // All paths are unique
    assert.strictEqual(new Set(out.map(d => d.outputPath)).size, 3);
  });
});

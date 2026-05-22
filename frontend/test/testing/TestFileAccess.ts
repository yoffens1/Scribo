// test/utils/TestFileAccess.ts
import { readFile, writeFile, access, mkdir, readdir, rename, unlink } from "fs/promises";
import { join } from "path";

export class TestFileAccess {
  constructor(private root: string) {}

  async readBinary(normalizedPath: string): Promise<ArrayBuffer> {
    const full = join(this.root, normalizedPath);
    const buf = await readFile(full);
    return buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
  }

  async readText(normalizedPath: string): Promise<string> {
    const full = join(this.root, normalizedPath);
    return readFile(full, "utf-8");
  }

  async writeText(normalizedPath: string, content: string): Promise<void> {
    const full = join(this.root, normalizedPath);
    await mkdir(join(full, ".."), { recursive: true });
    await writeFile(full, content, "utf-8");
  }

  async exists(normalizedPath: string): Promise<boolean> {
    const full = join(this.root, normalizedPath);
    try { await access(full); return true; } catch { return false; }
  }

  async list(normalizedPath: string): Promise<Array<{ name: string; isDir: boolean }>> {
    const full = join(this.root, normalizedPath);
    try {
      const entries = await readdir(full, { withFileTypes: true });
      return entries.map(e => ({ name: e.name, isDir: e.isDirectory() }));
    } catch { return []; }
  }

  async rename(from: string, to: string): Promise<void> {
    await rename(join(this.root, from), join(this.root, to));
  }

  async delete(normalizedPath: string): Promise<void> {
    const full = join(this.root, normalizedPath);
    try { await unlink(full); } catch { /* ignore missing */ }
  }
}

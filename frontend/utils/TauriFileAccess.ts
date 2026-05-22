import { IFileAccess } from "./_types";
import { readFile, readTextFile, writeTextFile, exists, readDir, rename, remove, mkdir } from "@tauri-apps/plugin-fs";
import { BaseDirectory } from "@tauri-apps/plugin-fs";
import { resolve, dirname } from "@tauri-apps/api/path";

export class TauriFileAccess implements IFileAccess {
  private baseDir = { baseDir: BaseDirectory.AppData };

  async readBinary(normalizedPath: string): Promise<ArrayBuffer> {
    const bytes = await readFile(normalizedPath, this.baseDir);
    return bytes.buffer;
  }

  async readText(normalizedPath: string): Promise<string> {
    return await readTextFile(normalizedPath, this.baseDir);
  }

  async writeText(normalizedPath: string, content: string): Promise<void> {
    // ensure parent dir exists
    try {
      const parent = await dirname(normalizedPath);
      if (parent && parent !== ".") {
        await mkdir(parent, { ...this.baseDir, recursive: true });
      }
    } catch (e) {}
    
    await writeTextFile(normalizedPath, content, this.baseDir);
  }

  async exists(normalizedPath: string): Promise<boolean> {
    return await exists(normalizedPath, this.baseDir);
  }

  async list(normalizedPath: string): Promise<Array<{ name: string; isDir: boolean }>> {
    const entries = await readDir(normalizedPath, this.baseDir);
    return entries.map(e => ({ name: e.name, isDir: e.isDirectory }));
  }

  async rename(from: string, to: string): Promise<void> {
    // ensure parent dir exists
    try {
      const parent = await dirname(to);
      if (parent && parent !== ".") {
        await mkdir(parent, { ...this.baseDir, recursive: true });
      }
    } catch (e) {}

    await rename(from, to, { oldPathBaseDir: BaseDirectory.AppData, newPathBaseDir: BaseDirectory.AppData });
  }

  async delete(normalizedPath: string): Promise<void> {
    const isDir = await this.exists(normalizedPath) && (await readDir(normalizedPath, this.baseDir).catch(() => null)) !== null;
    await remove(normalizedPath, { ...this.baseDir, recursive: isDir });
  }
}

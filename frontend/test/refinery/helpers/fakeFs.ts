// src/test/refinery/helpers/fakeFs.ts
import type { IFileAccess } from "@utils/_types";

export interface FakeFs extends IFileAccess {
  files: Map<string, string>;
  writeCalls: number;
  deleteCalls: number;
  renameCalls: Array<{ from: string; to: string }>;
}

export const fakeFs = (initial: Record<string, string> = {}): FakeFs => {
  const files = new Map(Object.entries(initial));

  return {
    files,
    writeCalls: 0,
    deleteCalls: 0,
    renameCalls: [],

    async readText(path) {
      if (!files.has(path)) throw new Error(`ENOENT: ${path}`);
      return files.get(path)!;
    },
    async readBinary(_path) {
      throw new Error("readBinary not implemented in fakeFs");
    },
    async writeText(path, content) {
      files.set(path, content);
      this.writeCalls++;
    },
    async exists(path) {
      return files.has(path);
    },
    async list(path) {
      const prefix = path.endsWith("/") ? path : path ? path + "/" : "";
      const entries: Array<{ name: string; isDir: boolean }> = [];
      const seen = new Set<string>();
      for (const key of files.keys()) {
        if (!key.startsWith(prefix)) continue;
        const rest = key.slice(prefix.length);
        const firstSlash = rest.indexOf("/");
        if (firstSlash === -1) {
          if (!seen.has(rest)) {
            seen.add(rest);
            entries.push({ name: rest, isDir: false });
          }
        } else {
          const dirName = rest.slice(0, firstSlash);
          if (!seen.has(dirName)) {
            seen.add(dirName);
            entries.push({ name: dirName, isDir: true });
          }
        }
      }
      return entries;
    },
    async rename(from, to) {
      const content = files.get(from);
      if (!content) throw new Error(`ENOENT: ${from}`);
      files.delete(from);
      files.set(to, content);
      this.renameCalls.push({ from, to });
    },
    async delete(path) {
      files.delete(path);
      this.deleteCalls++;
    },
  };
};

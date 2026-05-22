// src/core/filesearch/FuzzySearch.ts
import { FuzzyIndex } from "./FuzzyIndex";
import type { FileEntry, SearchResult } from "./types";

/**
 * Utility fuzzy search over vault file/folder names.
 *
 * NOT part of the RAG retrieval pipeline — separate data source (Obsidian
 * Vault API, not SQLite chunks), separate algorithm (fuzzy/Levenshtein,
 * not cosine/BM25), separate lifecycle (index rebuilt on vault events).
 *
 * Usage:
 *   const fs = new FuzzySearch(() => collectEntries(app));
 *   fs.rebuild();                        // on vault init
 *   fs.onFileCreated(entry);             // on vault 'create' event
 *   fs.onFileDeleted((e) => e.path===p); // on vault 'delete' event
 *   fs.onFileRenamed(oldPath, newEntry); // on vault 'rename' event
 *   const results = fs.search("wiresh"); // → [{ entry, score }]
 */
export class FuzzySearch {
  private index = new FuzzyIndex();
  private collectEntries: () => FileEntry[];

  constructor(collectEntries: () => FileEntry[]) {
    this.collectEntries = collectEntries;
  }

  /** Full rebuild from vault (call on plugin load). */
  rebuild(): void {
    this.index.rebuild(this.collectEntries());
  }

  /** File/folder created. */
  onFileCreated(entry: FileEntry): void {
    this.index.add(entry);
  }

  /** File/folder deleted. Pass predicate matching the removed entry. */
  onFileDeleted(predicate: (entry: FileEntry) => boolean): void {
    this.index.remove(predicate);
  }

  /** File/folder renamed. Remove old entry by path, add new one. */
  onFileRenamed(oldPath: string, newEntry: FileEntry): void {
    this.index.remove((e) => e.path === oldPath);
    this.index.add(newEntry);
  }

  /** Fuzzy search — returns up to `limit` results sorted by score desc. */
  search(query: string, limit = 20): SearchResult[] {
    return this.index.search(query, limit);
  }

  get size(): number {
    return this.index.size;
  }
}

/**
 * Collect FileEntry[] from Obsidian vault API.
 * Caller wires this to app.vault.getFiles() + app.vault.getAllLoadedFiles().
 */
export function collectVaultEntries(
  files: Array<{ path: string; name: string }>,
): FileEntry[] {
  return files.map((f) => {
    const name = f.name || f.path.split("/").pop() || f.path;
    const dotIdx = name.lastIndexOf(".");
    return {
      path: f.path,
      name,
      nameNoExt: dotIdx > 0 ? name.slice(0, dotIdx) : name,
      isFolder: !name.includes("."),
    };
  });
}

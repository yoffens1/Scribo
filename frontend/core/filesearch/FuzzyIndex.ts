// src/core/filesearch/FuzzyIndex.ts
import Fuse from "fuse.js";
import type { FileEntry, SearchResult } from "./types";

/**
 * Wraps a Fuse.js v7 index over FileEntry records.
 * Supports incremental add/remove and full rebuild (setCollection).
 */
export class FuzzyIndex {
  private fuse: Fuse<FileEntry>;

  constructor() {
    this.fuse = new Fuse<FileEntry>([], {
      keys: [
        { name: "name", weight: 0.5 },
        { name: "nameNoExt", weight: 0.35 },
        { name: "path", weight: 0.15 },
      ],
      threshold: 0.4,
      distance: 100,
      includeScore: true,
      minMatchCharLength: 1,
      ignoreLocation: true,
      useExtendedSearch: true,
    });
  }

  /** Replace the entire index (vault init or full rescan). */
  rebuild(entries: FileEntry[]): void {
    this.fuse.setCollection(entries);
  }

  /** Add a single entry (file created). */
  add(entry: FileEntry): void {
    this.fuse.add(entry);
  }

  /** Remove entries matching a predicate (file deleted/renamed). */
  remove(predicate: (entry: FileEntry) => boolean): void {
    this.fuse.remove(predicate);
  }

  /** Fuzzy search — returns up to `limit` results. */
  search(query: string, limit = 20): SearchResult[] {
    if (!query.trim()) return [];

    const results = this.fuse.search(query.trim(), { limit });

    return results.map((r) => ({
      entry: r.item,
      // Fuse score: 0 = perfect match, 1 = no match. Invert.
      score: r.score !== undefined ? 1 - r.score : 0,
      matchedFields: r.matches
        ? [...new Set(r.matches.map((m) => m.key ?? "").filter(Boolean))]
        : [],
    }));
  }

  get size(): number {
    return this.fuse.getIndex().size();
  }
}

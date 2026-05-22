// src/core/filesearch/types.ts

export interface FileEntry {
  /** Full normalized path: "conp/API.md" */
  path: string;
  /** Basename with extension: "API.md" */
  name: string;
  /** Basename without extension: "API" */
  nameNoExt: string;
  /** True for folders (TFolder), false for files (TFile) */
  isFolder: boolean;
}

export interface SearchResult {
  entry: FileEntry;
  /** 0–1, higher = better match (1 = exact) */
  score: number;
  /** Which fields matched */
  matchedFields: string[];
}

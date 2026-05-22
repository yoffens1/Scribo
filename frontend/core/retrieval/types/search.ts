// src/core/retrieval/types/search.ts
import type { ChunkRef } from "./chunk";

/** Single search result from text-based retrieval (semantic/keyword/hybrid). */
export interface SearchResult {
  chunkRef: ChunkRef;
  score: number;
  /** May be empty for keyword-only retrievers — caller fills from DB. */
  text?: string;
}

export interface SearchQuery {
  text: string;
  topK?: number;
  filters?: {
    filePath?: string;
    folder?: string;
  };
}

// src/core/retrieval/retrievers/types/fetch.ts

/** File-scoped chunk fetch (not text search — different operation). */
export interface FetchQuery {
  filePath?: string;
  fileName?: string;
  includeDeleted?: boolean;
  limit?: number;
  offset?: number;
}

/** Discriminated union for locating chunks — replaces fragile heuristic. */
export type ChunkQuery =
  | { by: "filePath"; value: string }
  | { by: "fileName"; value: string };

export interface FetchResult {
  chunkId?: number;
  filePath: string;
  chunkIndex: number;
  chunkText?: string;
  tokenCount?: number;
  embedding: Float32Array;
}

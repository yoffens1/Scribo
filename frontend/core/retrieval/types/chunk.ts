// src/core/retrieval/types/chunk.ts

/** Natural key: filePath + chunkIndex. Used in search results, engine entries. */
export interface ChunkRef {
  filePath: string;
  chunkIndex: number;
}

/** Surrogate key (database integer PK). */
export type ChunkId = number;

/**
 * Chunk data from the database (ChunkSource port return types).
 * Structurally compatible with @database/models/types chunk records
 * — no mapping needed, TypeScript structural typing handles it.
 *
 * embedding is optional — UI preview / debug views may fetch chunks
 * without embeddings to save memory.
 */
export interface ChunkData {
  chunkIndex: number;
  chunkText?: string;
  embedding?: Float32Array;
  tokenCount?: number;
}

export interface ChunkDataWithPath extends ChunkData {
  filePath: string;
}

export interface FullChunkData extends ChunkDataWithPath {
  chunkId: ChunkId;
}

/** Internal engine entry (array index + ChunkRef). */
export interface ChunkEntry {
  id: number;
  chunkRef: ChunkRef;
}

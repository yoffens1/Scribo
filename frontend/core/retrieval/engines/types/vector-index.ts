// src/core/retrieval/engines/types/vector-index.ts
import type { ChunkRef } from "../../types/chunk";

export interface VectorIndex {
  add(id: number, vector: Float32Array): void;
  /** Set chunkRef for entry id — called once on build. */
  setChunkRef(id: number, chunkRef: ChunkRef): void;
  /** Incremental add/update by chunkRef — returns assigned ID. */
  addChunk(chunkRef: ChunkRef, vector: Float32Array): number;
  /** Incremental remove by chunkRef (idempotent). */
  removeChunk(chunkRef: ChunkRef): void;
  /** Set entry vector to null; search() skips it. Idempotent. */
  remove(id: number): void;
  search(query: Float32Array, k: number): Array<{ id: number; score: number; chunkRef: ChunkRef }>;
}

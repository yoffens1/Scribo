// src/core/retrieval/types/chunk-source.ts
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "./chunk";

/**
 * Port (hexagonal architecture) — defines what retrieval layer needs from storage.
 * Implemented by database/repositories/ChunkRepository.
 * All methods are synchronous (sql.js is in-memory); callers ensure DB is open.
 *
 * Types are structurally compatible with @database/models/types —
 * no runtime mapping needed; TypeScript structural typing handles it.
 */
export interface ChunkSource {
  getAll(includeDeleted?: boolean): Promise<FullChunkData[]>;
  getByFilePath(filePath: string, includeDeleted?: boolean): Promise<ChunkData[]>;
  getByFileName(name: string, includeDeleted?: boolean): Promise<ChunkDataWithPath[]>;
}

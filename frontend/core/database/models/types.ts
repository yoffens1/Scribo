// src/core/database/models/types.ts

export interface ChunkRecord {
  chunkIndex: number;
  chunkText?: string;
  embedding: Float32Array;
  tokenCount?: number;
}

export interface ChunkRecordWithPath extends ChunkRecord {
  filePath?: string;
}

export interface FullChunkRecord {
  chunkId: number;
  filePath: string;
  chunkIndex: number;
  chunkText?: string;
  tokenCount?: number;
  embedding: Float32Array;
}

export interface FileRecord {
  fileId: number;
  fileHash: string | null;
  isDeleted: number | null;
  model: string | null;
  chunkVersion: string | null;
  mtime: number | null;
}

export interface FileDBInfo {
  isDeleted: boolean;
  mtime: number | null;
  model: string | null;
  chunkVer: string | null;
}

// src/core/retrieval/LegacyRetrievalAdapter.ts
import type { ChunkSource } from "./types/chunk-source";
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "./types/chunk";
import type { FetchQuery, FetchResult, ChunkQuery } from "./retrievers/types";

/**
 * Adapter providing deprecated chunk-access methods.
 * Used for backward compat in VectorDatabase — not part of the main API.
 *
 * @deprecated Use ChunkSource directly or RetrievalService for queries.
 */
export class LegacyRetrievalAdapter {
  constructor(private source: ChunkSource) {}

  /** @deprecated Use getChunks({ by: "filePath", value }) instead. */
  async getFileChunks(filePath: string, includeDeleted = false): Promise<ChunkData[]> {
    return await this.source.getByFilePath(filePath, includeDeleted);
  }

  /** @deprecated Use getChunks({ by: "fileName", value }) instead. */
  async getChunksByFileName(name: string, includeDeleted = false): Promise<ChunkDataWithPath[]> {
    return await this.source.getByFileName(name, includeDeleted);
  }

  /** @deprecated Use getChunks({ by }) or ChunkSource.getAll() instead. */
  async getAllChunks(includeDeleted = false): Promise<FullChunkData[]> {
    return await this.source.getAll(includeDeleted);
  }

  /** Get chunks by explicit discriminator. */
  async getChunks(query: ChunkQuery, includeDeleted = false): Promise<ChunkDataWithPath[]> {
    if (query.by === "filePath") {
      return (await this.source.getByFilePath(query.value, includeDeleted)) as ChunkDataWithPath[];
    }
    return await this.source.getByFileName(query.value, includeDeleted);
  }

  /** @deprecated Use getChunks({ by }) instead. */
  async getChunksByFileOrName(filePathOrName: string, includeDeleted = false): Promise<ChunkDataWithPath[]> {
    if (filePathOrName.includes("/")) {
      return (await this.source.getByFilePath(filePathOrName, includeDeleted)) as ChunkDataWithPath[];
    }
    return await this.source.getByFileName(filePathOrName, includeDeleted);
  }

  /** File-scoped chunk fetch (no ranking). */
  async search(query: FetchQuery = {}): Promise<FetchResult[]> {
    const includeDeleted = query.includeDeleted ?? false;

    let raw: any[];
    if (query.filePath !== undefined) {
      raw = await this.source.getByFilePath(query.filePath, includeDeleted);
    } else if (query.fileName !== undefined) {
      raw = await this.source.getByFileName(query.fileName, includeDeleted);
    } else {
      raw = await this.source.getAll(includeDeleted);
    }

    const offset = query.offset ?? 0;
    const limit = query.limit ?? raw.length;
    const page = raw.slice(offset, offset + limit);

    return page.map((r: any) => ({
      chunkId: "chunkId" in r ? r.chunkId : undefined,
      filePath: "filePath" in r ? r.filePath : query.filePath ?? "",
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText,
      tokenCount: r.tokenCount,
      embedding: r.embedding ?? new Float32Array(0),
    }));
  }
}

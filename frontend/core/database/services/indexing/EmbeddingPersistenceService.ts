// src/core/database/services/indexing/EmbeddingPersistenceService.ts
import { TauriDbConnection } from "../../infrastructure/TauriDbConnection";
import { FileRepository } from "../../repositories/FileRepository";
import { ChunkRepository } from "../../repositories/ChunkRepository";
import { MaintenanceService } from "../MaintenanceService";
import { Embedder } from "@ai/embedding/Embedder";
import { countTokens } from "@utils/chunker/token";
import { MAX_CHUNKS_PER_FILE } from "../../models/constants";
import {
  DbEventBus,
  Events,
  FileIndexedPayload,
  IndexingErrorPayload,
} from "../../EventBus";

/**
 * Persists file metadata + embeddings + chunks in a single transaction.
 * Emits events for cross-system listeners (vector index, BM25, telemetry).
 */
export class EmbeddingPersistenceService {
  constructor(
    private core: TauriDbConnection,
    private files: FileRepository,
    private chunks: ChunkRepository,
    private maintenance: MaintenanceService,
    private eventBus: DbEventBus,
  ) {}

  async persist(params: {
    filePath: string;
    cleanPath: string;
    fileName: string;
    fileHash: string;
    fileMtime: number | null;
    embeddingModel: string;
    embeddingDim: number;
    chunkingVersion: string;
    updatedAt: number;
    content: string;
    embedder: Embedder;
  }): Promise<void> {
    const {
      filePath,
      cleanPath,
      fileName,
      fileHash,
      fileMtime,
      embeddingModel,
      embeddingDim,
      chunkingVersion,
      updatedAt,
      content,
      embedder,
    } = params;

    let chunkCount = 0;
    try {
      // Phase 1: compute embeddings OUTSIDE the transaction.
      // Network calls can take seconds — keeping a tx open that long
      // blocks flush() and risks statement invalidation by db.export().
      const chunked = await embedder.embedChunked(content);
      chunkCount = chunked.length;
      if (chunked.length > MAX_CHUNKS_PER_FILE) {
        throw new Error(
          `File "${filePath}" produced ${chunked.length} chunks, exceeding limit of ${MAX_CHUNKS_PER_FILE}`,
        );
      }

      const rows = chunked.map((c, i) => ({
        chunkIndex: i,
        text: c.text,
        tokens: countTokens(c.text),
        embedding: new Uint8Array(c.embedding.buffer, c.embedding.byteOffset, c.embedding.byteLength),
      }));

      // Phase 2: DB writes only — now async via Tauri.
      await this.core.withTransaction(async () => {
        let fileId: number | null = null;
        const existing = await this.files.getByPath(cleanPath);

        if (!existing) {
          fileId = await this.files.insertIndexing({
            cleanPath,
            fileName,
            fileHash,
            fileMtime,
            embeddingModel,
            embeddingDim,
            chunkingVersion,
            updatedAt,
          });
        } else {
          fileId = existing.fileId;
          await this.files.updateIndexing({
            fileHash,
            fileMtime,
            embeddingModel,
            embeddingDim,
            chunkingVersion,
            updatedAt,
            fileName,
            fileId,
          });
        }

        const deleted = await this.chunks.deleteByFileId(fileId);
        this.maintenance.trackDeleted(deleted);

        await this.chunks.insertChunks(fileId, rows);
        await this.files.markIndexed(fileId);
      });
    } catch (error) {
      try {
        await this.files.markFailed(filePath, String(error));
      } catch (e) {}
      await this.eventBus.emit<IndexingErrorPayload>(Events.INDEXING_ERROR, {
        filePath,
        error: String(error),
      });
      throw error;
    }

    await this.eventBus.emit<FileIndexedPayload>(Events.FILE_INDEXED, {
      filePath,
      chunkCount,
    });
  }
}

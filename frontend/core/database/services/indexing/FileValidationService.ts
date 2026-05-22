// src/core/database/services/indexing/FileValidationService.ts
import { TauriDbConnection } from "../../infrastructure/TauriDbConnection";
import { FileRepository } from "../../repositories/FileRepository";
import { normalizePath } from "@utils/pathUtils";
import { MAX_FILE_SIZE, MAX_EMBEDDING_DIM } from "../../models/constants";
import { logger } from "../../infrastructure/Logger";

/**
 * Pre-indexing validation: mtime fast-path, file size, embedding dimension.
 * Also owns change-detection logic (hasFileChanged).
 */
export class FileValidationService {
  constructor(
    private core: TauriDbConnection,
    private files: FileRepository,
  ) {}

  async hasFileChanged(
    filePath: string,
    fileHash: string,
    embeddingModel?: string,
    chunkingVersion?: string,
  ): Promise<boolean> {
    const info = await this.files.getByPath(filePath);
    if (!info) return true;
    if (info.isDeleted === 1) return true;
    if (info.fileHash !== fileHash) return true;
    if (embeddingModel !== undefined && info.model !== embeddingModel)
      return true;
    if (chunkingVersion !== undefined && info.chunkVersion !== chunkingVersion)
      return true;
    return false;
  }

  /**
   * Returns true if the file can be skipped via mtime match.
   */
  async canSkipByMtime(
    filePath: string,
    fileHash: string,
    embeddingModel: string,
    chunkingVersion: string,
    fileMtime: number,
  ): Promise<boolean> {
    const cleanPath = normalizePath(filePath);
    const info = await this.files.getByPath(cleanPath);
    if (!info || info.mtime === null) return false;
    return (
      info.mtime === fileMtime &&
      info.fileHash === fileHash &&
      info.model === embeddingModel &&
      info.chunkVersion === chunkingVersion
    );
  }

  /**
   * Validates file size. Returns true if skipped (recorded as failed).
   */
  async validateSize(params: {
    filePath: string;
    content: string;
    cleanPath: string;
    fileName: string;
    fileHash: string;
    fileMtime: number | null;
  }): Promise<boolean> {
    const byteLength = new TextEncoder().encode(params.content).byteLength;
    if (byteLength <= MAX_FILE_SIZE) return false;

    logger.warn(
      `File "${params.filePath}" too large (${byteLength} bytes), skipping.`,
    );
    await this.core.ensureDb();
    const updatedAt = Date.now();
    await this.core.withTransaction(async () => {
      await this.files.insertFailed({
        cleanPath: params.cleanPath,
        fileName: params.fileName,
        fileHash: params.fileHash,
        fileMtime: params.fileMtime,
        error: "File too large",
        updatedAt,
      });
    });
    return true;
  }

  /**
   * Validates embedding dimension. Returns true if skipped (recorded as failed).
   */
  async validateDimension(params: {
    filePath: string;
    embeddingDim: number;
    cleanPath: string;
    fileName: string;
    fileHash: string;
    fileMtime: number | null;
  }): Promise<boolean> {
    if (params.embeddingDim <= MAX_EMBEDDING_DIM) return false;

    logger.warn(
      `Embedding dimension ${params.embeddingDim} exceeds limit, skipping.`,
    );
    await this.core.ensureDb();
    const updatedAt = Date.now();
    await this.core.withTransaction(async () => {
      await this.files.insertFailed({
        cleanPath: params.cleanPath,
        fileName: params.fileName,
        fileHash: params.fileHash,
        fileMtime: params.fileMtime,
        error: "Embedding dimension too large",
        updatedAt,
      });
    });
    return true;
  }
}

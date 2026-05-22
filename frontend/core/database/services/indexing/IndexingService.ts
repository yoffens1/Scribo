// src/core/database/services/indexing/IndexingService.ts
import { TauriDbConnection } from "../../infrastructure/TauriDbConnection";
import { FileRepository } from "../../repositories/FileRepository";
import { ChunkRepository } from "../../repositories/ChunkRepository";
import { MaintenanceService } from "../MaintenanceService";
import { ReindexScheduler } from "../ReindexScheduler";
import { Embedder } from "@ai/embedding/Embedder";
import { normalizePath } from "@utils/pathUtils";
import { HashService } from "./HashService";
import { FileValidationService } from "./FileValidationService";
import { EmbeddingPersistenceService } from "./EmbeddingPersistenceService";
import { DbEventBus, Events, FileIndexedPayload, IndexingErrorPayload } from "../../EventBus";
import { logger } from "../../infrastructure/Logger";

/**
 * Thin indexing orchestrator. Delegates hashing, validation, and persistence
 * to dedicated services. Owns the indexFile/indexFiles flow.
 */
export class IndexingService {
  private hashService: HashService;
  private validation: FileValidationService;
  private persistence: EmbeddingPersistenceService;

  constructor(
    private core: TauriDbConnection,
    files: FileRepository,
    chunks: ChunkRepository,
    maintenance: MaintenanceService,
    private scheduler: ReindexScheduler,
    private eventBus: DbEventBus,
  ) {
    this.hashService = new HashService();
    this.validation = new FileValidationService(core, files);
    this.persistence = new EmbeddingPersistenceService(
      core,
      files,
      chunks,
      maintenance,
      eventBus,
    );
  }

  async computeHash(content: string): Promise<string> {
    return this.hashService.compute(content);
  }

  async hasFileChanged(
    filePath: string,
    fileHash: string,
    embeddingModel?: string,
    chunkingVersion?: string,
  ): Promise<boolean> {
    return await this.validation.hasFileChanged(
      filePath,
      fileHash,
      embeddingModel,
      chunkingVersion,
    );
  }

  async indexFile(
    filePath: string,
    content: string,
    embedder: Embedder,
    fileMtime?: number,
    chunkingVersion: string = "1",
    precomputedHash?: string,
  ): Promise<void> {
    await this.core.ensureDb();
    const cleanPath = normalizePath(filePath);
    const fileName = cleanPath.split("/").pop() ?? cleanPath;
    const fileHash = precomputedHash ?? (await this.hashService.compute(content));

    const embeddingModel = (embedder as any).model ?? "unknown";
    const embeddingDim = embedder.getDimensions();
    const updatedAt = Date.now();

    // Mtime fast-path
    if (fileMtime !== undefined) {
      if (
        await this.validation.canSkipByMtime(
          filePath,
          fileHash,
          embeddingModel,
          chunkingVersion,
          fileMtime,
        )
      ) {
        logger.info(`File "${filePath}" unchanged (mtime match), skipping.`);
        return;
      }
    } else if (
      !(await this.validation.hasFileChanged(
        cleanPath,
        fileHash,
        embeddingModel,
        chunkingVersion,
      ))
    ) {
      logger.info(`File "${filePath}" hasn't changed, skipping indexing.`);
      return;
    }

    // Size guard
    if (
      await this.validation.validateSize({
        filePath,
        content,
        cleanPath,
        fileName,
        fileHash,
        fileMtime: fileMtime ?? null,
      })
    ) {
      return;
    }

    // Dimension guard
    if (
      await this.validation.validateDimension({
        filePath,
        embeddingDim,
        cleanPath,
        fileName,
        fileHash,
        fileMtime: fileMtime ?? null,
      })
    ) {
      return;
    }

    // Persist
    await this.persistence.persist({
      filePath,
      cleanPath,
      fileName,
      fileHash,
      fileMtime: fileMtime ?? null,
      embeddingModel,
      embeddingDim,
      chunkingVersion,
      updatedAt,
      content,
      embedder,
    });

    this.core.scheduleFlush();
  }

  async indexFiles(
    files: string[],
    contents: string[],
    embedder: Embedder,
    chunkingVersion: string = "1",
  ): Promise<void> {
    if (files.length !== contents.length) {
      throw new Error("Files and contents arrays must have the same length");
    }
    await this.scheduler.enqueueIndexing(async () => {
      await this.core.ensureDb();
      for (let i = 0; i < files.length; i++) {
        await this.indexFile(
          files[i],
          contents[i],
          embedder,
          undefined,
          chunkingVersion,
        );
      }
      this.core.scheduleFlush();
    });
  }
}

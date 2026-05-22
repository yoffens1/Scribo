// src/core/database/services/ReconciliationService.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { FileRepository } from "../repositories/FileRepository";
import { IndexingService } from "./indexing/IndexingService";
import { MaintenanceService } from "./MaintenanceService";
import { ReindexScheduler } from "./ReindexScheduler";
import { Embedder } from "@ai/embedding/Embedder";
import { logger } from "../infrastructure/Logger";

/**
 * Startup recovery — vault vs DB reconciliation.
 * Detects missing, deleted, and stale files.
 */
export class ReconciliationService {
  constructor(
    private core: TauriDbConnection,
    private files: FileRepository,
    private indexing: IndexingService,
    private maintenance: MaintenanceService,
    private scheduler: ReindexScheduler,
  ) {}

  async reconcile(
    embedder: Embedder,
    listFiles: () => Promise<string[]>,
    readFile: (path: string) => Promise<string>,
    getMtime?: (path: string) => Promise<number | undefined>,
  ): Promise<void> {
    return this.scheduler.enqueueIndexing(async () => {
    await this.core.ensureDb();

    const dbFiles = await this.files.getFilesMap();
    const vaultFiles = await listFiles();
    const vaultSet = new Set(vaultFiles);

    // Soft-delete files missing from vault
    for (const [dbPath, info] of dbFiles) {
      if (!info.isDeleted && !vaultSet.has(dbPath)) {
        logger.info(`Soft deleting (missing in vault): ${dbPath}`);
        await this.core.withTransaction(async () => {
          await this.files.softDelete(dbPath, Date.now());
        });
      }
    }

    // Index new/changed files
    const model = (embedder as any).model ?? "unknown";
    let mtimeSkips = 0;
    for (const filePath of vaultFiles) {
      try {
        // Mtime fast-path
        if (getMtime !== undefined) {
          const dbInfo = dbFiles.get(filePath);
          if (dbInfo && !dbInfo.isDeleted) {
            const currentMtime = await getMtime(filePath);
            if (
              currentMtime !== undefined &&
              dbInfo.mtime === currentMtime &&
              dbInfo.model === model &&
              dbInfo.chunkVer === "1"
            ) {
              mtimeSkips++;
              continue;
            }
          }
        }

        const content = await readFile(filePath);
        const fileHash = await this.indexing.computeHash(content);
        if (await this.indexing.hasFileChanged(filePath, fileHash, model, "1")) {
          logger.info(`Indexing changed/new file: ${filePath}`);
          await this.indexing.indexFile(
            filePath,
            content,
            embedder,
            undefined,
            "1",
            fileHash,
          );
        }
      } catch (err) {
        logger.warn(`Skipping ${filePath}: ${err}`);
      }
    }

    if (mtimeSkips > 0) {
      logger.info(
        `Reconcile: skipped ${mtimeSkips} files via mtime fast-path`,
      );
    }
    this.core.scheduleFlush();
    });
  }

  async reindexAllFiles(
    embedder: Embedder,
    listFiles: () => Promise<string[]>,
    readFile: (path: string) => Promise<string>,
    force = false,
  ): Promise<void> {
    return this.scheduler.enqueueIndexing(async () => {
    await this.core.ensureDb();
    const files = await listFiles();
    const model = (embedder as any).model ?? "unknown";

    for (const filePath of files) {
      try {
        const content = await readFile(filePath);
        const fileHash = await this.indexing.computeHash(content);
        if (
          force ||
          await this.indexing.hasFileChanged(filePath, fileHash, model, "1")
        ) {
          logger.info(`Reindexing: ${filePath}`);
          await this.indexing.indexFile(
            filePath,
            content,
            embedder,
            undefined,
            "1",
            fileHash,
          );
        }
      } catch (err) {
        logger.warn(`Skipping ${filePath}: ${err}`);
      }
    }
    this.core.scheduleFlush();
    await this.maintenance.optimize();
    });
  }
}

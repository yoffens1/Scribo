// src/core/database/services/FileMutationService.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { logger } from "../infrastructure/Logger";
import { FileRepository } from "../repositories/FileRepository";
import { MaintenanceService } from "./MaintenanceService";
import { ReindexScheduler } from "./ReindexScheduler";
import { DbEventBus, Events, FileDeletedPayload } from "../EventBus";

/**
 * Structural file mutations: rename, hard-delete, path update.
 */
export class FileMutationService {
  constructor(
    private core: TauriDbConnection,
    private files: FileRepository,
    private maintenance: MaintenanceService,
    private scheduler: ReindexScheduler,
    private eventBus: DbEventBus,
  ) {}

  async rename(oldPath: string, newPath: string): Promise<boolean> {
    await this.core.ensureDb();
    if (!(await this.files.exists(oldPath))) {
      logger.warn(`File "${oldPath}" not found, cannot rename.`);
      return false;
    }
    const updatedAt = Date.now();
    await this.core.withTransaction(async () => {
      await this.files.rename(oldPath, newPath, updatedAt);
    });
    this.core.scheduleFlush();
    return true;
  }

  async updateFilePath(oldPath: string, newPath: string): Promise<void> {
    await this.rename(oldPath, newPath);
  }

  async hardDelete(filePath: string): Promise<void> {
    await this.core.ensureDb();
    await this.scheduler.enqueueIndexing(async () => {
      await this.core.withTransaction(async () => {
        const cnt = await this.files.countChunksForFile(filePath);
        await this.files.hardDelete(filePath);
        this.maintenance.trackDeleted(cnt);
      });
    });
    this.core.scheduleFlush();
    await this.eventBus.emit<FileDeletedPayload>(Events.FILE_DELETED, { filePath });
  }
}

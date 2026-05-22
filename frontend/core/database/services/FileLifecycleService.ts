// src/core/database/services/FileLifecycleService.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { FileRepository } from "../repositories/FileRepository";
import { ReindexScheduler } from "./ReindexScheduler";
import { DbEventBus, Events, FileDeletedPayload } from "../EventBus";

/**
 * File lifecycle state transitions: soft-delete, restore.
 */
export class FileLifecycleService {
  constructor(
    private core: TauriDbConnection,
    private files: FileRepository,
    private scheduler: ReindexScheduler,
    private eventBus: DbEventBus,
  ) {}

  async softDelete(filePath: string): Promise<void> {
    await this.core.ensureDb();
    const updatedAt = Date.now();
    await this.scheduler.enqueueIndexing(async () => {
      await this.core.withTransaction(async () => {
        await this.files.softDelete(filePath, updatedAt);
      });
    });
    await this.eventBus.emit<FileDeletedPayload>(Events.FILE_DELETED, { filePath });
  }

  async restore(filePath: string): Promise<void> {
    await this.core.ensureDb();
    const updatedAt = Date.now();
    await this.core.withTransaction(async () => {
      await this.files.restore(filePath, updatedAt);
    });
  }
}

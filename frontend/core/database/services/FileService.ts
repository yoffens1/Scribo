// src/core/database/services/FileService.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { FileRepository } from "../repositories/FileRepository";
import { MaintenanceService } from "./MaintenanceService";
import { ReindexScheduler } from "./ReindexScheduler";
import { DbEventBus } from "../EventBus";
import { FileLifecycleService } from "./FileLifecycleService";
import { FileMutationService } from "./FileMutationService";

/**
 * Thin facade over file sub-services: lifecycle + mutations.
 */
export class FileService {
  private lifecycle: FileLifecycleService;
  private mutations: FileMutationService;

  constructor(
    core: TauriDbConnection,
    files: FileRepository,
    maintenance: MaintenanceService,
    scheduler: ReindexScheduler,
    eventBus: DbEventBus,
  ) {
    this.lifecycle = new FileLifecycleService(core, files, scheduler, eventBus);
    this.mutations = new FileMutationService(core, files, maintenance, scheduler, eventBus);
  }

  async softDelete(filePath: string): Promise<void> {
    return this.lifecycle.softDelete(filePath);
  }

  async restore(filePath: string): Promise<void> {
    return this.lifecycle.restore(filePath);
  }

  async rename(oldPath: string, newPath: string): Promise<boolean> {
    return this.mutations.rename(oldPath, newPath);
  }

  async updateFilePath(oldPath: string, newPath: string): Promise<void> {
    return this.mutations.updateFilePath(oldPath, newPath);
  }

  async hardDelete(filePath: string): Promise<void> {
    return this.mutations.hardDelete(filePath);
  }
}

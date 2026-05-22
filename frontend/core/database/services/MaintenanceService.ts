import { invoke } from "@tauri-apps/api/core";
// src/core/database/services/MaintenanceService.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { logger } from "../infrastructure/Logger";

/**
 * Database maintenance: optimize, vacuum, chunk counter tracking.
 */
export class MaintenanceService {
  private _deletedChunksCount = 0;

  get deletedChunksCount(): number {
    return this._deletedChunksCount;
  }

  constructor(private core: TauriDbConnection) {}

  trackDeleted(count: number): void {
    this._deletedChunksCount += count;
  }

  resetCounters(): void {
    this._deletedChunksCount = 0;
  }

  async optimize(): Promise<void> {
    await invoke("db_optimize");
    this.core.markDirty();
    this.core.scheduleFlush();
  }

  async vacuumIfNeeded(threshold: number = 1000): Promise<void> {
    if (this.deletedChunksCount >= threshold) {
      logger.info(
        `Performing VACUUM (${this.deletedChunksCount} deleted chunks)`,
      );
      await invoke("db_vacuum");
      this._deletedChunksCount = 0;
      this.core.scheduleFlush();
    }
  }

  async forceVacuum(): Promise<void> {
    logger.info("Forced VACUUM");
    await invoke("db_vacuum");
    this._deletedChunksCount = 0;
    this.core.scheduleFlush();
  }
}

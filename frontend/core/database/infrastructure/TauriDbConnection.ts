import { invoke } from "@tauri-apps/api/core";
import { normalizePath } from "@utils/pathUtils";
import { sanitizeFileName } from "@utils/sanitizeFileName";

export class TauriDbConnection {
  readonly dbPath: string;
  private initialized = false;

  constructor(pluginDir: string, modelName?: string) {
    const safeModel = modelName ? sanitizeFileName(modelName) : "default";
    this.dbPath = normalizePath(pluginDir + "/vectors-" + safeModel + ".db");
  }

  async initialize(): Promise<void> {
    if (this.initialized) return;
    await invoke("db_initialize", { dbPath: this.dbPath });
    this.initialized = true;
  }

  async ensureDb(): Promise<boolean> {
    if (this.initialized) return false;
    await this.initialize();
    return true;
  }

  markDirty(): void {}
  scheduleFlush(): void {}
  async flush(): Promise<void> {}
  async save(): Promise<void> {}

  async withTransaction<T>(fn: () => Promise<T>): Promise<T> {
    await invoke("db_begin_transaction");
    try {
      const result = await fn();
      await invoke("db_commit_transaction");
      return result;
    } catch (error) {
      try {
        await invoke("db_rollback_transaction");
      } catch {}
      throw error;
    }
  }

  async close(): Promise<void> {
    await invoke("db_close");
    this.initialized = false;
  }
}

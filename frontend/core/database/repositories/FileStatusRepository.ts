import { invoke } from "@tauri-apps/api/core";
import { normalizePath } from "@utils/pathUtils";
import { logger } from "../infrastructure/Logger";

export class FileStatusRepository {
  constructor() {}

  async markIndexed(fileId: number): Promise<void> {
    await invoke("files_mark_indexed", { fileId });
  }

  async markFailed(filePath: string, error: string): Promise<void> {
    const cleanPath = normalizePath(filePath);
    try {
      await invoke("files_mark_failed", { path: cleanPath, error });
    } catch (err) {
      logger.error("markFailed: unable to update status for", filePath, err);
    }
  }

  async insertFailed(params: {
    cleanPath: string;
    fileName: string;
    fileHash: string;
    fileMtime: number | null;
    error: string;
    updatedAt: number;
  }): Promise<void> {
    await invoke("files_insert_failed", { params });
  }

  async softDelete(filePath: string, updatedAt: number): Promise<void> {
    const cleanPath = normalizePath(filePath);
    await invoke("files_soft_delete", { path: cleanPath, updatedAt });
  }

  async restore(filePath: string, updatedAt: number): Promise<void> {
    const cleanPath = normalizePath(filePath);
    await invoke("files_restore", { path: cleanPath, updatedAt });
  }
}

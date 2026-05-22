import { invoke } from "@tauri-apps/api/core";
import { normalizePath } from "@utils/pathUtils";
import { FileDBInfo } from "../models/types";

export class FileQueryRepository {
  constructor() {}

  async rename(oldPath: string, newPath: string, updatedAt: number): Promise<void> {
    const cleanOld = normalizePath(oldPath);
    const cleanNew = normalizePath(newPath);
    await invoke("files_rename", { oldPath: cleanOld, newPath: cleanNew, updatedAt });
  }

  async countChunksForFile(filePath: string): Promise<number> {
    const cleanPath = normalizePath(filePath);
    return await invoke<number>("files_count_chunks", { path: cleanPath });
  }

  async hardDelete(filePath: string): Promise<void> {
    const cleanPath = normalizePath(filePath);
    await invoke("files_hard_delete", { path: cleanPath });
  }

  async getAllFiles(): Promise<{
    filePath: string;
    isDeleted: number;
    mtime: number | null;
    model: string | null;
    chunkVer: string | null;
    fileId: number;
  }[]> {
    return await invoke("files_get_all");
  }

  async getFilesMap(): Promise<Map<string, FileDBInfo>> {
    const record = await invoke<Record<string, {
      isDeleted: boolean;
      mtime: number | null;
      model: string | null;
      chunkVer: string | null;
    }>>("files_get_map");
    
    const dbFiles = new Map<string, FileDBInfo>();
    for (const [key, value] of Object.entries(record)) {
      dbFiles.set(key, value);
    }
    return dbFiles;
  }
}

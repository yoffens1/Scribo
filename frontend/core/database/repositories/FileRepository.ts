import { invoke } from "@tauri-apps/api/core";
import { FileStatusRepository } from "./FileStatusRepository";
import { FileQueryRepository } from "./FileQueryRepository";
import { FileRecord } from "../models/types";
import { normalizePath } from "@utils/pathUtils";

export class FileRepository {
  readonly status: FileStatusRepository;
  readonly query: FileQueryRepository;

  constructor() {
    this.status = new FileStatusRepository();
    this.query = new FileQueryRepository();
  }

  async getByPath(filePath: string): Promise<FileRecord | null> {
    const cleanPath = normalizePath(filePath);
    return await invoke<FileRecord | null>("files_get_by_path", { path: cleanPath });
  }

  async insertIndexing(params: {
    cleanPath: string;
    fileName: string;
    fileHash: string;
    fileMtime: number | null;
    embeddingModel: string;
    embeddingDim: number;
    chunkingVersion: string;
    updatedAt: number;
  }): Promise<number> {
    return await invoke<number>("files_insert_indexing", { params });
  }

  async updateIndexing(params: {
    fileHash: string;
    fileMtime: number | null;
    embeddingModel: string;
    embeddingDim: number;
    chunkingVersion: string;
    updatedAt: number;
    fileName: string;
    fileId: number;
  }): Promise<void> {
    await invoke("files_update_indexing", { params });
  }

  async exists(filePath: string): Promise<boolean> {
    const cleanPath = normalizePath(filePath);
    return await invoke<boolean>("files_exists", { path: cleanPath });
  }

  // ── Delegated ──
  async markIndexed(fileId: number): Promise<void> { return this.status.markIndexed(fileId); }
  async markFailed(filePath: string, error: string): Promise<void> { return this.status.markFailed(filePath, error); }
  async insertFailed(params: Parameters<FileStatusRepository["insertFailed"]>[0]): Promise<void> { return this.status.insertFailed(params); }
  async softDelete(filePath: string, updatedAt: number): Promise<void> { return this.status.softDelete(filePath, updatedAt); }
  async restore(filePath: string, updatedAt: number): Promise<void> { return this.status.restore(filePath, updatedAt); }

  async rename(oldPath: string, newPath: string, updatedAt: number): Promise<void> { return this.query.rename(oldPath, newPath, updatedAt); }
  async hardDelete(filePath: string): Promise<void> { return this.query.hardDelete(filePath); }
  async countChunksForFile(filePath: string): Promise<number> { return this.query.countChunksForFile(filePath); }
  async getAllFiles() { return this.query.getAllFiles(); }
  async getFilesMap() { return this.query.getFilesMap(); }
}

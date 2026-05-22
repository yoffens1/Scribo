import { invoke } from "@tauri-apps/api/core";
import { normalizePath } from "@utils/pathUtils";
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "../../retrieval/types/chunk";
import type { ChunkSource } from "../../retrieval/types/chunk-source";

function uint8ToFloat32Array(data: number[] | Uint8Array): Float32Array {
  const u8 = data instanceof Uint8Array ? data : new Uint8Array(data);
  const buf = new ArrayBuffer(u8.byteLength);
  new Uint8Array(buf).set(u8);
  return new Float32Array(buf);
}

export class ChunkRepository implements ChunkSource {
  constructor() {}

  async deleteByFileId(fileId: number): Promise<number> {
    return await invoke<number>("chunks_delete_by_file_id", { fileId });
  }

  async insertChunks(
    fileId: number,
    rows: Array<{ chunkIndex: number; text: string; tokens: number; embedding: Uint8Array | Float32Array }>,
  ): Promise<void> {
    const serializedRows = rows.map(r => ({
      chunkIndex: r.chunkIndex,
      text: r.text,
      tokens: r.tokens,
      embedding: Array.from(r.embedding instanceof Uint8Array ? r.embedding : new Uint8Array(r.embedding.buffer))
    }));

    await invoke("chunks_insert", { fileId, rows: serializedRows });
  }

  async getByFilePath(
    filePath: string,
    includeDeleted = false,
  ): Promise<ChunkData[]> {
    const cleanPath = normalizePath(filePath);
    const records = await invoke<Array<{
      chunkIndex: number;
      chunkText: string | null;
      embedding: number[];
      tokenCount: number | null;
    }>>("chunks_get_by_file_path", { filePath: cleanPath, includeDeleted });

    return records.map(r => ({
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      embedding: uint8ToFloat32Array(r.embedding),
      tokenCount: r.tokenCount || undefined,
    }));
  }

  async getAll(includeDeleted = false): Promise<FullChunkData[]> {
    const records = await invoke<Array<{
      chunkId: number;
      filePath: string;
      chunkIndex: number;
      chunkText: string | null;
      tokenCount: number | null;
      embedding: number[];
    }>>("chunks_get_all", { includeDeleted });

    return records.map(r => ({
      chunkId: r.chunkId,
      filePath: r.filePath,
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      tokenCount: r.tokenCount || undefined,
      embedding: uint8ToFloat32Array(r.embedding),
    }));
  }

  async getByFileName(name: string, includeDeleted = false): Promise<ChunkDataWithPath[]> {
    const records = await invoke<Array<{
      filePath: string;
      chunkIndex: number;
      chunkText: string | null;
      tokenCount: number | null;
      embedding: number[];
    }>>("chunks_get_by_file_name", { name, includeDeleted });

    return records.map(r => ({
      filePath: r.filePath,
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      tokenCount: r.tokenCount || undefined,
      embedding: uint8ToFloat32Array(r.embedding),
    }));
  }
}

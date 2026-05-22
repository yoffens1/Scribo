import { invoke } from "@tauri-apps/api/core";
import { normalizePath } from "@utils/pathUtils";
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "../../retrieval/types/chunk";
import type { ChunkSource } from "../../retrieval/types/chunk-source";

function uint8ToBase64(u8: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < u8.length; i++) {
    binary += String.fromCharCode(u8[i]);
  }
  return "base64:" + btoa(binary);
}

function base64ToFloat32Array(base64: string): Float32Array {
  if (base64.startsWith("base64:")) {
    base64 = base64.substring(7);
  }
  const raw = atob(base64);
  const u8 = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) {
    u8[i] = raw.charCodeAt(i);
  }
  // The first byte was prepended by EmbeddingSerializer in old DB format, 
  // but if it's raw we might need to skip it depending on logic.
  // Actually, we skip the first byte as per original logic:
  const payload = u8.subarray(1);
  const buf = new ArrayBuffer(payload.byteLength);
  new Uint8Array(buf).set(payload);
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
      embedding: uint8ToBase64(r.embedding instanceof Uint8Array ? r.embedding : new Uint8Array(r.embedding.buffer))
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
      embedding: string;
      tokenCount: number | null;
    }>>("chunks_get_by_file_path", { filePath: cleanPath, includeDeleted });

    return records.map(r => ({
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      embedding: base64ToFloat32Array(r.embedding),
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
      embedding: string;
    }>>("chunks_get_all", { includeDeleted });

    return records.map(r => ({
      chunkId: r.chunkId,
      filePath: r.filePath,
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      tokenCount: r.tokenCount || undefined,
      embedding: base64ToFloat32Array(r.embedding),
    }));
  }

  async getByFileName(name: string, includeDeleted = false): Promise<ChunkDataWithPath[]> {
    const records = await invoke<Array<{
      filePath: string;
      chunkIndex: number;
      chunkText: string | null;
      tokenCount: number | null;
      embedding: string;
    }>>("chunks_get_by_file_name", { name, includeDeleted });

    return records.map(r => ({
      filePath: r.filePath,
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText || undefined,
      tokenCount: r.tokenCount || undefined,
      embedding: base64ToFloat32Array(r.embedding),
    }));
  }
}

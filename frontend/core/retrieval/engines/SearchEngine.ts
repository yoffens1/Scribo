// src/core/rag/SearchEngine.ts
import type { Embedder } from "@ai/embedding/Embedder";
import type { SearchResult } from "../types/search";
import type { VectorIndex } from "./types";

const TOP_K = 5;

/**
 * High‑level search engine.
 * Holds a vector index, provides a simple `search()` API.
 * ChunkRef lookup is delegated to VectorIndex — no duplicate entries storage.
 */
export class SearchEngine {
  /**
   * @param embedder    Embedder instance (must be initialized)
   * @param vectorIndex  Any implementation of VectorIndex (BruteForceIndex, HnswIndex, …)
   */
  constructor(
    private embedder: Embedder,
    private vectorIndex: VectorIndex,
  ) {}

  /** Expose vector index for IndexRegistry — read-only. */
  get index(): VectorIndex { return this.vectorIndex; }

  // ── Incremental ops (delegated to vectorIndex) ──

  addChunk(chunk: { filePath: string; chunkIndex: number }, embedding: Float32Array): void {
    this.vectorIndex.addChunk(
      { filePath: chunk.filePath, chunkIndex: chunk.chunkIndex },
      embedding,
    );
  }

  removeChunk(chunk: { filePath: string; chunkIndex: number }): void {
    this.vectorIndex.removeChunk(
      { filePath: chunk.filePath, chunkIndex: chunk.chunkIndex },
    );
  }

  /**
   * Load all chunks from the database and build the vector index.
   * @param getAllChunks  async function returning all DB chunks
   */
  async buildFromDatabase(
    getAllChunks: () => Promise<
      {
        filePath: string;
        chunkIndex: number;
        chunkText?: string;
        embedding: Float32Array;
      }[]
    >,
  ): Promise<void> {
    const chunks = await getAllChunks();
    chunks.forEach((chunk, idx) => {
      if (!chunk.embedding) return;
      this.vectorIndex.add(idx, chunk.embedding);
      this.vectorIndex.setChunkRef(idx, {
        filePath: chunk.filePath,
        chunkIndex: chunk.chunkIndex,
      });
    });
  }

  /**
   * Perform semantic search for a text query.
   * Scores + chunkRef come directly from VectorIndex.search().
   */
  async search(
    queryText: string,
    topK: number = TOP_K,
  ): Promise<SearchResult[]> {
    const queryVec = await this.embedder.embedQuery(queryText);
    const scored = this.vectorIndex.search(queryVec, topK);

    return scored
      .map(({ chunkRef, score }) => ({ chunkRef, score }))
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
  }
}

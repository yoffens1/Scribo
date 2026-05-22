// src/core/retrieval/IndexRegistry.ts
import type { ChunkSource } from "./types/chunk-source";
import type { Embedder } from "@ai/embedding/Embedder";
import type { VectorIndex } from "./engines/types";
import type { ChunkEntry } from "./types/chunk";
import { Bm25Index } from "./engines/Bm25Index";
import { SearchEngine } from "./engines/SearchEngine";
import { RETRIEVAL_CONSTANTS } from './constants';
import { BruteForceIndex } from "./engines/BruteForceVectorIndex";

/**
 * Singleton-like cache for BM25 + vector indices.
 *
 * Avoids rebuilding from scratch on every RetrievalFactory.create() call.
 * Subscribers call markDirty() when chunks change; next access triggers
 * full rebuild. For frequent incremental updates, add addChunk/removeChunk.
 *
 * Memory: Bm25Index stores only termFreq (not full text).
 *
 * ChunkEntry ownership: IndexRegistry owns the canonical entries[].
 * SearchEngine holds a snapshot via setEntries() — rebuilt together on
 * getSearchEngine(). The two lists stay in sync through markDirty().
 */
export class IndexRegistry {
  private bm25: Bm25Index | null = null;
  private engine: SearchEngine | null = null;
  private dirtyBm25 = true;
  private dirtyVector = true;

  constructor(
    private chunkSource: ChunkSource,
    private embedder: Embedder | null,
  ) {}

  setEmbedder(embedder: Embedder): void {
    this.embedder = embedder;
  }

  getEmbedder(): Embedder {
    if (!this.embedder) throw new Error("setEmbedder() must be called first");
    return this.embedder;
  }

  /** BM25 text index (lazy build on first access). */
  async getBm25Index(): Promise<Bm25Index> {
    if (this.dirtyBm25 || !this.bm25) {
      this.bm25 = new Bm25Index();
      const all = await this.chunkSource.getAll();
      for (const chunk of all) {
        this.bm25.addDocument(
          { filePath: chunk.filePath, chunkIndex: chunk.chunkIndex },
          chunk.chunkText ?? "",
        );
      }
      this.dirtyBm25 = false;
    }
    return this.bm25;
  }

  /** Vector search engine (lazy build on first access). */
  async getSearchEngine(): Promise<SearchEngine> {
    if (!this.embedder) throw new Error("setEmbedder() must be called first");
    if (this.dirtyVector || !this.engine) {
      const vectorIndex: VectorIndex = new BruteForceIndex();
      this.engine = new SearchEngine(this.embedder, vectorIndex);
      const all = await this.chunkSource.getAll();
      for (let i = 0; i < all.length; i++) {
        const chunk = all[i];
        if (!chunk.embedding) continue;
        vectorIndex.add(i, chunk.embedding);
        vectorIndex.setChunkRef(i, {
          filePath: chunk.filePath,
          chunkIndex: chunk.chunkIndex,
        });
      }
      this.dirtyVector = false;
    }
    return this.engine;
  }

  private rebuildTimer: ReturnType<typeof setTimeout> | null = null;
  private pendingDirty = false;

  /**
   * Debounced cache invalidation.
   *
   * Rapid-fire events (e.g. user saves 5 files in 1s) trigger only one
   * rebuild after 500ms of inactivity. Each immediate markDirty() would
   * rebuild BM25 + vector index from scratch — expensive on 10k+ chunks.
   *
   * For truly incremental updates, add addChunk()/removeChunk() later.
   */
  markDirty(): void {
    this.pendingDirty = true;
    if (this.rebuildTimer) clearTimeout(this.rebuildTimer);
    this.rebuildTimer = setTimeout(() => {
      if (this.pendingDirty) {
        this.dirtyBm25 = true;
        this.dirtyVector = true;
        this.pendingDirty = false;
      }
    }, RETRIEVAL_CONSTANTS.INDEX_REBUILD_DEBOUNCE_MS);
  }

  /** Immediate invalidation — use for model change or manual reindex. */
  markDirtyImmediate(): void {
    if (this.rebuildTimer) {
      clearTimeout(this.rebuildTimer);
      this.rebuildTimer = null;
    }
    this.dirtyBm25 = true;
    this.dirtyVector = true;
    this.pendingDirty = false;
  }

  // ── Incremental updates ──

  /** Add one chunk to both indices without full rebuild. */
  async addChunk(chunk: { filePath: string; chunkIndex: number; chunkText?: string; embedding?: Float32Array }): Promise<void> {
    const bm25 = await this.getBm25Index();
    bm25.addChunk(chunk, chunk.chunkText ?? "");
    if (chunk.embedding) {
      const engine = await this.getSearchEngine();
      engine.addChunk(chunk, chunk.embedding);
    }
  }

  /** Remove one chunk from both indices without full rebuild. */
  async removeChunk(chunk: { filePath: string; chunkIndex: number }): Promise<void> {
    const bm25 = await this.getBm25Index();
    bm25.removeChunk(chunk);
    if (this.engine) {
      this.engine.removeChunk(chunk);
    }
  }

  // ── Persistence ──

  /** Serialize BM25 index to bytes for disk storage. */
  saveBm25(): Uint8Array | null {
    if (!this.bm25 || this.dirtyBm25) return null;
    return this.bm25.serialize();
  }

  /** Restore BM25 index from bytes (skips rebuild on next getBm25Index). */
  loadBm25(data: Uint8Array): void {
    this.bm25 = Bm25Index.deserialize(data);
    this.dirtyBm25 = false;
  }

  /** Release memory — call on plugin unload. */
  dispose(): void {
    if (this.rebuildTimer) {
      clearTimeout(this.rebuildTimer);
      this.rebuildTimer = null;
    }
    this.bm25 = null;
    this.engine = null;
  }
}

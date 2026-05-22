// src/core/retrieval/engines/BruteForceVectorIndex.ts
import { cosineSimilarity } from "./similarity";
import type { VectorIndex } from "./types";
import type { ChunkRef } from "../types/chunk";
import { logger } from "../../database/infrastructure/Logger";

/**
 * Brute-force cosine vector index. O(N) per search — acceptable up to ~10k vectors.
 *
 * Owns both vectors[sparse] and chunkRefs[Map] — single source of truth
 * for ID → (vector, chunkRef) mapping. Search returns chunkRef directly,
 * so SearchEngine doesn't need its own entries array.
 */
export class BruteForceIndex implements VectorIndex {
  private vectors: (Float32Array | null)[] = [];
  private chunkRefs = new Map<number, ChunkRef>();
  private refToId = new Map<string, number>();
  private measureTime: boolean;

  constructor(measureTime = false) {
    this.measureTime = measureTime;
  }

  add(id: number, vector: Float32Array): void {
    // Ensure sparse array is sized correctly
    while (this.vectors.length <= id) this.vectors.push(null);
    this.vectors[id] = vector;
  }

  setChunkRef(id: number, chunkRef: ChunkRef): void {
    this.chunkRefs.set(id, chunkRef);
    this.refToId.set(`${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`, id);
  }

  /** Incremental add — existing ID or next free slot. */
  addChunk(chunkRef: ChunkRef, vector: Float32Array): number {
    const key = `${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`;
    const existing = this.refToId.get(key);
    if (existing !== undefined) {
      this.vectors[existing] = vector;
      return existing;
    }
    const id = this.vectors.length;
    this.vectors.push(vector);
    this.chunkRefs.set(id, chunkRef);
    this.refToId.set(key, id);
    return id;
  }

  /** Incremental remove by chunkRef. */
  removeChunk(chunkRef: ChunkRef): void {
    const key = `${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`;
    const id = this.refToId.get(key);
    if (id !== undefined) {
      this.remove(id);
      this.refToId.delete(key);
    }
  }

  remove(id: number): void {
    if (id < this.vectors.length) this.vectors[id] = null;
    this.chunkRefs.delete(id);
  }

  search(query: Float32Array, k: number): Array<{ id: number; score: number; chunkRef: ChunkRef }> {
    const start = performance.now();
    const scores: Array<{ id: number; score: number; chunkRef: ChunkRef }> = [];
    for (let i = 0; i < this.vectors.length; i++) {
      const vec = this.vectors[i];
      const chunkRef = this.chunkRefs.get(i);
      if (!vec || !chunkRef) continue;
      scores.push({ id: i, score: cosineSimilarity(query, vec), chunkRef });
    }
    const result = scores
      .sort((a, b) => b.score - a.score)
      .slice(0, k);
    if (this.measureTime) {
      logger.info(
        `[BruteForce] search took ${(performance.now() - start).toFixed(2)} ms`,
      );
    }
    return result;
  }
}

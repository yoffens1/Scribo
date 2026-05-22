// src/core/rag/Bm25Index.ts
import type { ChunkRef } from "../types/chunk";

interface Doc {
  id: number;
  terms: Map<string, number>;
  length: number;
}

export class Bm25Index {
  private docs = new Map<number, Doc>();
  private idToChunk = new Map<number, ChunkRef>();
  private chunkToId = new Map<string, number>();
  private termDocs = new Map<string, Set<number>>();
  private totalLength = 0;
  private nextId = 0;
  private k1 = 1.5;
  private b = 0.75;

  addDocument(chunkRef: ChunkRef, text: string): void {
    const tokens = this.tokenize(text);
    if (tokens.length === 0) return;

    const termFreq = new Map<string, number>();
    for (const token of tokens) {
      termFreq.set(token, (termFreq.get(token) ?? 0) + 1);
    }

    const docId = this.nextId++;
    this.docs.set(docId, { id: docId, terms: termFreq, length: tokens.length });
    const key = `${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`;
    this.idToChunk.set(docId, chunkRef);
    this.chunkToId.set(key, docId);
    this.totalLength += tokens.length;

    for (const term of termFreq.keys()) {
      let docSet = this.termDocs.get(term);
      if (!docSet) {
        docSet = new Set();
        this.termDocs.set(term, docSet);
      }
      docSet.add(docId);
    }
  }

  addChunk(chunkRef: ChunkRef, text: string): void {
    this.addDocument(chunkRef, text);
  }

  removeChunk(chunkRef: ChunkRef): void {
    const key = `${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`;
    const docId = this.chunkToId.get(key);
    if (docId === undefined) return;

    const doc = this.docs.get(docId);
    if (!doc) return;

    this.docs.delete(docId);
    this.idToChunk.delete(docId);
    this.chunkToId.delete(key);
    this.totalLength -= doc.length;

    for (const term of doc.terms.keys()) {
      const docSet = this.termDocs.get(term);
      if (docSet) docSet.delete(docId);
    }
  }

  // ── Persistence ──
  // TODO: binary serialization (CBOR/MessagePack) for 10k+ chunks —
  // JSON can reach tens of MBs on large vaults.

  serialize(): Uint8Array {
    const docsArr = [...this.docs.values()].map(d => ({
      id: d.id,
      terms: Object.fromEntries(d.terms),
      length: d.length,
    }));
    return new TextEncoder().encode(JSON.stringify({
      docs: docsArr,
      idToChunk: [...this.idToChunk.entries()],
      totalLength: this.totalLength,
      nextId: this.nextId,
    }));
  }

  static deserialize(data: Uint8Array): Bm25Index {
    const obj = JSON.parse(new TextDecoder().decode(data));
    const index = new Bm25Index();
    index.totalLength = obj.totalLength;
    index.nextId = obj.nextId;

    for (const [id, chunkRef] of obj.idToChunk) {
      const nid = Number(id);
      index.idToChunk.set(nid, chunkRef);
      index.chunkToId.set(`${chunkRef.filePath}\u0000${chunkRef.chunkIndex}`, nid);
    }

    for (const d of obj.docs) {
      const terms = new Map<string, number>(Object.entries(d.terms));
      index.docs.set(d.id, { id: d.id, terms, length: d.length });
      for (const term of terms.keys()) {
        let docSet = index.termDocs.get(term);
        if (!docSet) { docSet = new Set(); index.termDocs.set(term, docSet); }
        docSet.add(d.id);
      }
    }

    return index;
  }

  // ── Search ──

  get avgDocLength(): number {
    return this.docs.size > 0 ? this.totalLength / this.docs.size : 0;
  }

  search(query: string, topK = 10): { chunkRef: ChunkRef; score: number }[] {
    const queryTokens = this.tokenize(query);
    if (queryTokens.length === 0) return [];

    const scores = new Map<number, number>();
    const N = this.docs.size;
    const avgLen = this.avgDocLength;

    for (const term of queryTokens) {
      const docSet = this.termDocs.get(term);
      if (!docSet || docSet.size === 0) continue;
      const df = docSet.size;
      const idf = Math.log((N - df + 0.5) / (df + 0.5) + 1);

      for (const docId of docSet) {
        const doc = this.docs.get(docId);
        if (!doc) continue;
        const tf = doc.terms.get(term) ?? 0;
        if (tf === 0) continue;

        const numerator = tf * (this.k1 + 1);
        const denominator = tf + this.k1 * (1 - this.b + this.b * (doc.length / avgLen));
        scores.set(docId, (scores.get(docId) ?? 0) + idf * (numerator / denominator));
      }
    }

    return [...scores.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, topK)
      .map(([id, score]) => ({ chunkRef: this.idToChunk.get(id)!, score }));
  }

  private tokenize(text: string): string[] {
    return text.toLowerCase().split(/[\s\p{P}]+/u).filter(t => t.length > 0);
  }
}

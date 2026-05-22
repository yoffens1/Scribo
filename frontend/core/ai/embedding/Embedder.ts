// src/core/ai/embedding/Embedder.ts
import { EmbedderProvider, EmbedderConfig, EmbedderProviderInterface } from "./_types";
import type { Chunker } from "@utils/chunker/Chunker";
import {
  Chunker as DefaultChunker,
  EMBEDDING_OPTIONS,
} from "@utils/chunker/Chunker";
import { OllamaEmbedderProvider } from "./providers/ollama/OllamaEmbedderProvider";
import { OpenAIEmbedderProvider } from "./providers/openai/OpenAIEmbedderProvider";
import { logger } from "../../database/infrastructure/Logger";

export const PROVIDER_DEFAULTS: Record<
  EmbedderProvider,
  { model: string; baseUrl: string }
> = {
  ollama: {
    model: "qwen3-embedding:latest",
    baseUrl: "http://localhost:11434",
  },
  openai: {
    model: "text-embedding-3-small",
    baseUrl: "",
  },
};

export class Embedder {
  private providerImpl: EmbedderProviderInterface;
  private chunker: Chunker;
  private dim: number | null = null;

  constructor(config: EmbedderConfig) {
    const defaults = PROVIDER_DEFAULTS[config.provider];

    switch (config.provider) {
      case "ollama":
        this.providerImpl = new OllamaEmbedderProvider(
          config.model || defaults.model,
          config.baseUrl || defaults.baseUrl,
        );
        break;
      case "openai":
        this.providerImpl = new OpenAIEmbedderProvider(
          config.model || defaults.model,
          config.apiKey || "",
        );
        break;
    }

    // Use known dimension if available — skip probe request
    if (this.providerImpl.knownDim !== null) {
      this.dim = this.providerImpl.knownDim;
    }

    this.chunker = config.chunker ?? new DefaultChunker({ ...EMBEDDING_OPTIONS });
  }

  /**
   * Probes the embedding dimension if not already known.
   * No-op when dimension is available from KNOWN_DIMS map.
   */
  async initialize(): Promise<void> {
    if (this.dim !== null) {
      logger.info(`Embedder ready: dim=${this.dim} (cached)`);
      return;
    }
    const testVec = await this.providerImpl.embed("init");
    this.dim = testVec.length;
    logger.info(`Embedder ready: dim=${this.dim} (probed)`);
  }

  getDimensions(): number {
    if (this.dim === null)
      throw new Error("Not initialized – call initialize() first");
    return this.dim;
  }

  get model(): string {
    return this.providerImpl.model;
  }

  async embed(text: string): Promise<Float32Array> {
    const vec = await this.providerImpl.embed(text);
    this.validateDim(vec);
    return vec;
  }

  /**
   * Embed a search query with model-specific instruction prefix.
   * qwen3-embedding requires "Instruct: ..." wrapper on queries —
   * without it scores drop ~30%. Documents remain unwrapped.
   */
  async embedQuery(text: string): Promise<Float32Array> {
    const prompted = `Instruct: Given a query, retrieve relevant passages\nQuery: ${text}`;
    return this.embed(prompted);
  }

  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    const vecs = await this.providerImpl.embedBatch(texts);
    for (const v of vecs) this.validateDim(v);
    return vecs;
  }

  async embedChunked(
    text: string,
  ): Promise<{ text: string; embedding: Float32Array }[]> {
    const { chunks } = this.chunker.chunkForEmbedding(text);
    if (chunks.length === 0) return [];
    const embeddings = await this.providerImpl.embedBatch(chunks);
    return chunks.map((chunk, i) => ({ text: chunk, embedding: embeddings[i] }));
  }

  private validateDim(vec: Float32Array): void {
    if (this.dim !== null && vec.length !== this.dim) {
      throw new Error(
        `[Embedder] dimension mismatch: expected ${this.dim}, got ${vec.length} (model=${this.model})`,
      );
    }
  }
}

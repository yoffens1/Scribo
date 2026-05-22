import type { EmbedderProviderInterface } from "./_types";
import { EMBEDDER_CONFIG } from "@settings";
import type { EmbedderCapabilities } from "../types/capabilities";

/**
 * Abstract embedder base.
 *
 * - Resolves `knownDim` from shared config.
 * - Provides shared `validateDim`.
 * - Default `embedBatch` does sequential chunks with configurable concurrency.
 *   Override when the API supports native batching (e.g. OpenAI).
 */
export abstract class BaseEmbedder implements EmbedderProviderInterface {
  readonly model: string;
  readonly knownDim: number | null;
  abstract get capabilities(): EmbedderCapabilities;

  /** Providers must declare their capabilities. */

  constructor(model: string) {
    this.model = model;
    this.knownDim = EMBEDDER_CONFIG.knownDims[model] ?? null;
  }

  abstract embed(text: string): Promise<Float32Array>;

  /**
   * Embed multiple texts.
   * Default: sequential chunks of parallel single-embed calls.
   * Override for true batch APIs (e.g. OpenAI sends all texts in one request).
   */
  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    if (texts.length === 0) return [];
    const results: Float32Array[] = [];
    const batchSize = EMBEDDER_CONFIG.maxConcurrent;
    for (let i = 0; i < texts.length; i += batchSize) {
      const batch = texts.slice(i, i + batchSize);
      const batchResults = await Promise.all(batch.map((t) => this.embed(t)));
      results.push(...batchResults);
    }
    return results;
  }

  protected validateDim(vec: Float32Array, label: string): void {
    if (this.knownDim !== null && vec.length !== this.knownDim) {
      throw new Error(
        `[${label}] dimension mismatch: expected ${this.knownDim}, got ${vec.length} (model=${this.model})`,
      );
    }
  }
}

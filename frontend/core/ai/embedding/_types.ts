import type { Chunker } from "@utils/chunker/Chunker";
import type { EmbedderCapabilities } from "../../ai/types/capabilities";

export type EmbedderProvider = "ollama" | "openai";

export interface EmbedderConfig {
  provider: EmbedderProvider;
  model?: string;
  apiKey?: string;
  baseUrl?: string;
  dimensions?: number;
  chunker?: Chunker;
}

export interface EmbedderProviderInterface {
  readonly model: string;
  readonly knownDim: number | null;
  readonly capabilities: EmbedderCapabilities;
  embed(text: string): Promise<Float32Array>;
  embedBatch(texts: string[]): Promise<Float32Array[]>;
}

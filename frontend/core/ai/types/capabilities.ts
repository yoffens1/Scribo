export interface LLMCapabilities {
  streaming: boolean;
  tools: boolean;
  vision: boolean;
}

export interface EmbedderCapabilities {
  batchEmbedding: boolean;
}

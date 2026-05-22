// src/core/retrieval/retrievers/types/config.ts

export type RetrievalMode = "embedding" | "keyword" | "hybrid";

export interface PipelineConfig {
  autoTranslate?: boolean;
  expandSynonyms?: "off" | "static" | "llm";
  /** Static synonym dictionary — injected, not hardcoded. */
  synonymDict?: Record<string, string[]>;
  hyde?: boolean;
}

export interface RetrievalConfig {
  mode: RetrievalMode;
  embeddingWeight?: number;
  pipeline?: PipelineConfig;
  aiRerank?: {
    enabled: boolean;
    mode?: "scoring" | "listwise";
    maxCandidates?: number;
  };
  logging?: {
    enabled: boolean;
    minLevel?: "debug" | "info" | "warn" | "error";
    sinks?: Array<"console" | "memory" | "file">;
    fileSink?: { dir: string };
  };
}

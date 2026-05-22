// src/core/retrieval/index.ts
//
// Public API barrel — import only RetrievalService and types from here.
// Internal modules (engines/, retrievers/, pipeline/, rerankers/, logging/)
// should NOT be imported directly outside of core/retrieval/.

export { RetrievalService } from "./RetrievalService";
export { RetrievalFactory } from "./retrievers/RetrievalFactory";

// Types
export type { ChunkRef, ChunkId, ChunkData, ChunkDataWithPath, FullChunkData, ChunkEntry } from "./types/chunk";
export type { ChunkSource } from "./types/chunk-source";
export type { SearchResult, SearchQuery } from "./types/search";
export type { Retriever, RetrieveOptions } from "./retrievers/types";
export type { RetrievalMode, PipelineConfig, RetrievalConfig } from "./retrievers/types";
export type { FetchQuery, FetchResult, ChunkQuery } from "./retrievers/types";
export type { Reranker } from "./rerankers/types/reranker";

// Utilities
export { mergeByFile } from "./utils/mergeByFile";
export type { FileResult } from "./utils/mergeByFile";

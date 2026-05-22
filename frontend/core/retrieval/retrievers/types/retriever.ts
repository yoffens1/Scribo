// src/core/retrieval/retrievers/types/retriever.ts
import type { SearchResult } from "../../types/search";

export interface RetrieveOptions {
  topK?: number;
  filters?: { filePath?: string; folder?: string };
}

export interface Retriever {
  retrieve(query: string, options?: RetrieveOptions): Promise<SearchResult[]>;
}

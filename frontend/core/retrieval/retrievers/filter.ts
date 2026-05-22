// src/core/retrieval/retrievers/filter.ts
import type { SearchResult } from "../types/search";
import type { RetrieveOptions } from "./types";

/**
 * Apply RetrieveOptions.filters to search results.
 * filePath: exact match. folder: prefix match (startsWith).
 */
export function applyFilters(
  results: SearchResult[],
  options?: RetrieveOptions,
): SearchResult[] {
  const filters = options?.filters;
  if (!filters) return results;

  return results.filter((r) => {
    if (filters.filePath !== undefined && r.chunkRef.filePath !== filters.filePath) {
      return false;
    }
    if (filters.folder !== undefined && !r.chunkRef.filePath.startsWith(filters.folder)) {
      return false;
    }
    return true;
  });
}

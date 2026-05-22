// src/core/retrieval/pipeline/types/query-stage.ts
import type { ExpandedQuery } from "./expanded-query";

/**
 * Chain-of-responsibility stage in the query pipeline.
 * Each stage receives the current ExpandedQuery and returns an updated one —
 * adding variants or metadata.
 */
export interface QueryStage {
  process(input: ExpandedQuery): Promise<ExpandedQuery>;
}

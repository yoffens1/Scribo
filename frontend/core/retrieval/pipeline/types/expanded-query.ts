// src/core/retrieval/pipeline/types/expanded-query.ts
import type { QueryVariant } from "./query-variant";

export interface ExpandedQuery {
  original: string;
  variants: QueryVariant[];   // includes original at index 0
  detectedLang?: string;
  vaultLang?: string;         // dominant language of the vault
}

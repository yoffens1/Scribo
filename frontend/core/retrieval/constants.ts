// src/core/retrieval/constants.ts

export const RETRIEVAL_CONSTANTS = {
  /** RRF smoothing factor (Cormack et al.) */
  RRF_K: 60,
  /** Cap per inner retrieve() — prevents explosive over-fetch chains */
  MAX_OVERFETCH: 50,
  /** Debounce wait before rebuilding BM25+vector after chunk changes */
  INDEX_REBUILD_DEBOUNCE_MS: 500,
  /** TTL for vault language detection cache */
  VAULT_LANG_CACHE_TTL_MS: 24 * 60 * 60 * 1000,
} as const;

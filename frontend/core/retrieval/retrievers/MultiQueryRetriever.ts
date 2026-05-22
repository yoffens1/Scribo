// src/core/retrieval/retrievers/MultiQueryRetriever.ts
import type { Retriever, RetrieveOptions } from "./types";
import type { SearchResult } from "../types/search";
import type { QueryPipeline } from "../pipeline/QueryPipeline";
import type { QueryVariant } from "../pipeline/types";
import { rrf } from './fusion/rrf';
import { RETRIEVAL_CONSTANTS } from '../constants';
import { RetrievalLogger } from "../logging/RetrievalLogger";

/** Hard cap: no single inner retrieve() call fetches more than this. */


/** Boost multiplier for variants matching vault language. */
const DEFAULT_VAULT_LANG_BOOST = 1.5;

/** Penalty multiplier for variants in a language different from vault. */
const DEFAULT_FOREIGN_LANG_PENALTY = 0.5;

/**
 * Runs the inner retriever on every variant produced by the QueryPipeline,
 * then fuses all result lists with weighted RRF.
 *
 * Chain: MultiQueryRetriever → inner (Hybrid / Embedding / Keyword) → RRF
 */
export class MultiQueryRetriever implements Retriever {
  constructor(
    private inner: Retriever,
    private pipeline: QueryPipeline,
    private k = 60,
    private overFetchMultiplier = 3,
    private logger?: RetrievalLogger,
    private vaultLangBoost = DEFAULT_VAULT_LANG_BOOST,
    private foreignLangPenalty = DEFAULT_FOREIGN_LANG_PENALTY,
  ) {}

  async retrieve(
    query: string,
    options?: RetrieveOptions,
  ): Promise<SearchResult[]> {
    const expanded = await this.pipeline.run(query);
    const topK = options?.topK ?? 5;
    const overFetch = Math.min(topK * this.overFetchMultiplier, RETRIEVAL_CONSTANTS.MAX_OVERFETCH);

    // ── Deduplication: remove near-identical variants ──
    const deduped = dedupVariants(expanded.variants);
    if (deduped.length < expanded.variants.length) {
      this.logger?.log("info", "multiquery.dedup", "removed duplicates", {
        before: expanded.variants.length,
        after: deduped.length,
      });
    }

    // ── Adaptive weights: boost vault-language variants, penalize mismatches ──
    const vaultLang = expanded.vaultLang;
    const weighted = deduped.map(v => {
      if (!vaultLang || !v.lang) return v;
      if (v.lang === vaultLang) return { ...v, weight: v.weight * this.vaultLangBoost };
      if (v.lang !== vaultLang) return { ...v, weight: v.weight * this.foreignLangPenalty };
      return v;
    });

    this.logger?.log("info", "multiquery.variants", "fanning out", {
      variantCount: weighted.length,
      variants: weighted.map(v => ({
        text: v.text.slice(0, 60),
        source: v.source,
        lang: v.lang,
        weight: v.weight,
      })),
    });

    // Filters flow through to leaf retrievers (Embedding/Keyword)
    // where applyFilters() runs. MultiQueryRetriever is a pass-through —
    // fan-out works, but filtering happens downstream.
    const lists = await Promise.all(
      weighted.map(async v => {
        const t0 = performance.now();
        const results = await this.inner.retrieve(v.text, { ...options, topK: overFetch });
        this.logger?.log("debug", "multiquery.variant", v.source, {
          text: v.text.slice(0, 60),
          resultsCount: results.length,
          topScore: results[0]?.score,
          durationMs: performance.now() - t0,
        });
        return { results, weight: v.weight };
      }),
    );

    const fused = rrf(lists, this.k, topK);
    this.logger?.log("info", "multiquery.rrf", "fused", {
      inputLists: lists.length,
      outputCount: fused.length,
      topScores: fused.slice(0, 3).map(r => r.score),
    });

    return fused;
  }
}

/** Exact-match dedup: keep variant with highest weight. */
function dedupVariants(variants: QueryVariant[]): QueryVariant[] {
  const seen = new Map<string, QueryVariant>();
  for (const v of variants) {
    const key = v.text.toLowerCase().trim();
    const existing = seen.get(key);
    if (!existing || v.weight > existing.weight) {
      seen.set(key, v);
    }
  }
  return [...seen.values()];
}

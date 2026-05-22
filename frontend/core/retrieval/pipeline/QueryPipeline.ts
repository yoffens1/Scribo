// src/core/retrieval/pipeline/QueryPipeline.ts
import type { QueryStage, ExpandedQuery, QueryVariant } from "./types";
import { RetrievalLogger } from "../logging/RetrievalLogger";

/**
 * Query pipeline facade — runs a chain of stages over the raw user query,
 * producing an ExpandedQuery with multiple weighted variants for multi-query
 * retrieval.
 *
 * Usage:
 *   const pipeline = new QueryPipeline([new TranslationStage(...), ...]);
 *   const expanded = await pipeline.run("machine learning");
 */
export class QueryPipeline {
  constructor(
    private stages: QueryStage[],
    private logger?: RetrievalLogger,
  ) {}

  async run(query: string): Promise<ExpandedQuery> {
    let state: ExpandedQuery = {
      original: query,
      variants: [{ text: query, source: "original", weight: 1.0 }],
    };

    for (const stage of this.stages) {
      const stageName = stage.constructor.name.replace("Stage", "");
      const before = state.variants.length;

      const t0 = performance.now();
      state = await stage.process(state);
      const durationMs = performance.now() - t0;

      const added = state.variants.slice(before);
      this.logger?.log("info", `pipeline.${stageName.toLowerCase()}`, "processed", {
        variantsBefore: before,
        variantsAfter: state.variants.length,
        addedVariants: added.map(v => ({
          text: v.text.slice(0, 80),
          source: v.source,
          weight: v.weight,
        })),
        durationMs,
      });
    }

    this.logger?.log("info", "pipeline.done", "pipeline complete", {
      totalVariants: state.variants.length,
      variantSummary: state.variants.map(v => v.source).join(","),
    });

    return state;
  }
}

export type { QueryStage, ExpandedQuery, QueryVariant };

// src/core/refinery/stages/DeduplicationStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { ChunkWithHash, ChunkDecision, DeduplicationResult } from "../types/chunk-decision";
import { LogScope } from "../types/refinery-stage";
import { SimilarityMatcher } from "../dedupe/SimilarityMatcher";

const MAX_CONCURRENT = 8;

const batch = async <T, U>(items: T[], fn: (item: T) => Promise<U>, limit = MAX_CONCURRENT): Promise<U[]> => {
  const results: U[] = new Array(items.length);
  let cursor = 0;
  const worker = async (): Promise<void> => {
    while (cursor < items.length) {
      const idx = cursor++;
      results[idx] = await fn(items[idx]);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, () => worker()));
  return results;
};

export class DeduplicationStage implements RefineryStage<ChunkWithHash[], DeduplicationResult> {
  readonly name = "DeduplicationStage";

  async run(chunks: ChunkWithHash[], ctx: RefineryContext): Promise<DeduplicationResult> {
    const matcher = new SimilarityMatcher(ctx.retrieval);

    ctx.logger.log("info", LogScope.DEDUPE_START, `checking ${chunks.length} chunks`);

    const decisions = await batch(chunks, (chunk) => matcher.classify(chunk), MAX_CONCURRENT);

    const remaining: ChunkWithHash[] = [];
    for (const d of decisions) {
      if (d.action === "keep") remaining.push(d.chunk);
    }

    const merged = decisions.filter((d) => d.action === "merge").length;
    const rejected = decisions.filter((d) => d.action === "reject").length;

    ctx.logger.log("info", LogScope.DEDUPE_DONE, "deduplication complete", {
      total: chunks.length, merged, rejected, kept: remaining.length,
    });

    return { decisions, remaining };
  }
}

// src/core/refinery/stages/ConsolidationStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { AtomChunk } from "../types/atom-chunk";
import { LogScope } from "../types/refinery-stage";

/**
 * Stage 1c: Intra-document consolidation.
 * Merge nearly-identical chunks within the same source file.
 *
 * Unlike DeduplicationStage (which checks against output vault via retrieval),
 * this stage only compares chunks from the same document using text similarity.
 * It merges chunks that are 95%+ identical within the same file.
 */
export class ConsolidationStage implements RefineryStage<AtomChunk[], AtomChunk[]> {
  readonly name = "ConsolidationStage";

  private similarityThreshold = 0.95;

  async run(chunks: AtomChunk[], ctx: RefineryContext): Promise<AtomChunk[]> {
    if (chunks.length <= 1) return chunks;

    const result: AtomChunk[] = [];
    const merged = new Set<number>();
    let mergeCount = 0;

    for (let i = 0; i < chunks.length; i++) {
      if (merged.has(i)) continue;

      let best = chunks[i];
      for (let j = i + 1; j < chunks.length; j++) {
        if (merged.has(j)) continue;
        const sim = this.jaccardSimilarity(chunks[i].embeddingText, chunks[j].embeddingText);
        if (sim >= this.similarityThreshold) {
          // Merge: keep the longer text
          best = best.embeddingText.length >= chunks[j].embeddingText.length ? best : chunks[j];
          merged.add(j);
          mergeCount++;
        }
      }
      result.push(best);
    }

    if (mergeCount > 0) {
      ctx.logger.log("info", "consolidation", `merged ${mergeCount} near-duplicate chunks`, {
        before: chunks.length, after: result.length,
      });
    }

    return result;
  }

  /** Simple word-level Jaccard similarity — fast, no embeddings needed. */
  private jaccardSimilarity(a: string, b: string): number {
    const wordsA = new Set(a.toLowerCase().split(/\s+/));
    const wordsB = new Set(b.toLowerCase().split(/\s+/));
    const intersection = new Set([...wordsA].filter(w => wordsB.has(w)));
    const union = new Set([...wordsA, ...wordsB]);
    return union.size === 0 ? 0 : intersection.size / union.size;
  }
}

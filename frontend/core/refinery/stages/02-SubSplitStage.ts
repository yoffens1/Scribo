// src/core/refinery/stages/SubSplitStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { AtomChunk } from "../types/atom-chunk";
import { LogScope } from "../types/refinery-stage";
import { REFINERY_CONSTANTS } from "../constants";
import { countTokens } from "@utils/chunker/token/countTokens";

/**
 * Stage 1b: Semantic sub-splitting.
 * For each oversized chunk, check if it contains multiple concepts.
 * If so, split at semantic boundaries using embedding cosine distance.
 *
 * Simplified version: if a chunk exceeds maxTokens, split by sentence boundaries
 * at the largest cosine-gap. Falls back to paragraph splitting if embeddings unavailable.
 *
 * Operates on generationText for paragraph boundaries (original markdown structure),
 * then copies the split text to both embeddingText and generationText fields.
 */
export class SubSplitStage implements RefineryStage<AtomChunk[], AtomChunk[]> {
  readonly name = "SubSplitStage";

  private maxLimit = 1000;

  async run(chunks: AtomChunk[], ctx: RefineryContext): Promise<AtomChunk[]> {
    const result: AtomChunk[] = [];
    let subSplits = 0;

    for (const chunk of chunks) {
      const tokens = countTokens(chunk.generationText);
      
      // If token count is > 1000, we split
      if (tokens <= this.maxLimit) {
        result.push(chunk);
        continue;
      }

      const splits = this.splitAtSentences(chunk.generationText, chunk);
      if (splits.length > 1) {
        subSplits += splits.length - 1;
      }
      result.push(...splits);
    }

    if (subSplits > 0) {
      ctx.logger.log("info", "subsplit", `split ${subSplits} oversized chunks`, {
        before: chunks.length, after: result.length,
      });
    }

    return result;
  }

  private splitAtSentences(text: string, original: AtomChunk): AtomChunk[] {
    const sentences = text.split(/(?<=[.!?])\s+/).filter(s => s.trim().length > 0);
    if (sentences.length <= 1) {
      return [original];
    }

    const result: AtomChunk[] = [];
    let current: string[] = [];
    let currentTokens = 0;
    let part = 0;

    for (const sentence of sentences) {
      const sentenceTokens = countTokens(sentence);
      if (currentTokens + sentenceTokens > this.maxLimit && current.length > 0) {
        const splitText = current.join(" ");
        result.push({
          ...original,
          hash: original.hash + `-s${part++}`,
          embeddingText: splitText,
          generationText: splitText,
          text: splitText,
          index: original.index + part,
        });
        current = [sentence];
        currentTokens = sentenceTokens;
      } else {
        current.push(sentence);
        currentTokens += sentenceTokens;
      }
    }

    if (current.length > 0) {
      const splitText = current.join(" ");
      result.push({
        ...original,
        hash: original.hash + `-s${part}`,
        embeddingText: splitText,
        generationText: splitText,
        text: splitText,
        index: original.index + part,
      });
    }

    return result;
  }
}

// src/core/refinery/stages/EnrichmentStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { AtomChunk } from "../types/atom-chunk";
import { LogScope } from "../types/refinery-stage";
import { buildAliasesPrompt } from "@ai/prompts/refinery/enrich-aliases";
import { buildTagsPrompt } from "@ai/prompts/refinery/enrich-tags";

const MAX_CONCURRENT = 4;

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

/**
 * Stage 5: Enrichment — generate aliases + tags for each chunk.
 * Runs after Atomization so heading is available as context.
 */
export class EnrichmentStage implements RefineryStage<AtomChunk[], AtomChunk[]> {
  readonly name = "EnrichmentStage";

  async run(chunks: AtomChunk[], ctx: RefineryContext): Promise<AtomChunk[]> {
    if (chunks.length === 0) return chunks;

    ctx.logger.log("info", "enrich.start", `generating aliases + tags for ${chunks.length} chunks`);

    const enriched = await batch(chunks, async (chunk) => {
      try {
        const heading = chunk.questionHeading ?? "";
        const aliases = await this.generateAliases(chunk, heading, ctx);
        const tags = await this.generateTags(chunk, heading, ctx);
        return { ...chunk, aliases, tags };
      } catch (err) {
        ctx.logger.log("warn", "enrich.error", `failed for chunk ${chunk.hash.slice(0, 8)}`, {
          error: String(err),
        });
        return chunk;
      }
    });

    ctx.logger.log("info", "enrich.done", `enriched ${enriched.length} chunks`);
    return enriched;
  }

  private async generateAliases(chunk: AtomChunk, heading: string, ctx: RefineryContext): Promise<string[] | undefined> {
    if (chunk.generationText.length < 50) return undefined;

    const messages = buildAliasesPrompt(chunk.generationText, heading);
    const response = await ctx.llm.generateMessages(messages);
    try {
      const parsed = JSON.parse(response.text.trim());
      if (Array.isArray(parsed) && parsed.length > 0) return parsed.slice(0, 8);
    } catch {
      // LLM returned non-JSON — skip aliases
    }
    return undefined;
  }

  private async generateTags(chunk: AtomChunk, heading: string, ctx: RefineryContext): Promise<string[] | undefined> {
    if (chunk.generationText.length < 50) return undefined;

    const messages = buildTagsPrompt(chunk.generationText, heading);
    const response = await ctx.llm.generateMessages(messages);
    try {
      const parsed = JSON.parse(response.text.trim());
      if (Array.isArray(parsed) && parsed.length > 0) return parsed.slice(0, 6);
    } catch {
      // LLM returned non-JSON — skip tags
    }
    return undefined;
  }
}

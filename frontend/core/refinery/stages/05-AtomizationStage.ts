import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { AtomChunk } from "../types/atom-chunk";
import { LogScope } from "../types/refinery-stage";
import { buildAtomizePrompt } from "@ai/prompts/refinery/atomize";
import { extractJsonObject } from "@retrieval/utils/jsonExtract";

/** Max concurrent LLM calls for heading/filename generation. */
const MAX_CONCURRENT = 5;

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
 * Stage 5: Atomization — generate question-heading + filename for each chunk.
 * Runs in parallel with concurrency limit to avoid hammering LLM.
 */
export class AtomizationStage implements RefineryStage<AtomChunk[], AtomChunk[]> {
  readonly name = "AtomizationStage";

  async run(chunks: AtomChunk[], ctx: RefineryContext): Promise<AtomChunk[]> {
    if (chunks.length === 0) return chunks;

    ctx.logger.log("info", "atomize.start", `generating headings for ${chunks.length} chunks`);

    const enriched = await batch(chunks, async (chunk) => {
      try {
        const { heading, filename } = await this.generateAtom(chunk, ctx);
        return { ...chunk, questionHeading: heading, filename };
      } catch (err) {
        ctx.logger.log("warn", "atomize.error", `failed for chunk ${chunk.hash.slice(0, 8)}`, {
          error: String(err),
        });
        return chunk;
      }
    });

    const withHeading = enriched.filter(c => c.questionHeading).length;
    ctx.logger.log("info", "atomize.done", `generated headings for ${withHeading}/${enriched.length} chunks`);

    return enriched;
  }

  private async generateAtom(chunk: AtomChunk, ctx: RefineryContext): Promise<{ heading?: string; filename?: string }> {
    if (chunk.generationText.length < 30) return {};

    const messages = buildAtomizePrompt(chunk.generationText, chunk.sourcePath);
    const response = await ctx.llm.generateMessages(messages);
    const jsonStr = extractJsonObject(response.text);

    if (!jsonStr) {
      throw new Error("No JSON found in LLM response");
    }

    const parsed = JSON.parse(jsonStr);
    let heading = parsed.questionHeading?.trim();
    let filename = parsed.filename?.trim();

    if (heading) {
      // Validate: must start with "## "
      if (!heading.startsWith("## ")) {
        heading = `## ${heading.replace(/^#+\s*/, "")}`;
      }
    }

    if (filename) {
      // Clean up: remove markdown, ensure .md
      filename = filename.replace(/^[#*-]+\s*/, "").replace(/["'`]/g, "");
      if (!filename.endsWith(".md")) filename += ".md";

      // Sanitize for filesystem (keep spaces and Title Case)
      filename = filename
        .replace(/[<>:"/\\|?*]/g, "")
        .replace(/\s+/g, " ")
        .trim();
    }

    return { heading, filename };
  }
}

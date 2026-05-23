// src/core/refinery/stages/ChunkingStage.ts
import * as path from "path";
import { LogScope } from "../types/refinery-stage";
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { ChunkWithHash } from "../types/chunk-decision";

import { HashService } from "@database/services/indexing/HashService";
import { invoke } from "@tauri-apps/api/core";

/**
 * Stage 1: Read a raw .md file from inbox and produce twin chunks.
 *
 * Each logical chunk has two representations:
 *   - embeddingText: cleaned form for similarity search / dedup / hash
 *   - generationText: original markdown for LLM prompts / file writes / merges
 *
 * Uses Chunker.chunkPaired() which splits once with unified structural options
 * and cleans each raw chunk twice — guaranteeing embeddingText.length == generationText.length.
 */
export class ChunkingStage implements RefineryStage<string, ChunkWithHash[]> {
  readonly name = "ChunkingStage";

  private chunker: Chunker;
  private hashService: HashService;

  constructor(chunker?: Chunker, hashService?: HashService) {
    this.chunker = chunker ?? new Chunker();
    this.hashService = hashService ?? new HashService();
  }

  async run(sourcePath: string, ctx: RefineryContext): Promise<ChunkWithHash[]> {
    const fullPath = path.posix.join(ctx.inboxRoot, sourcePath);
    ctx.logger.log("info", LogScope.CHUNKING_READ, fullPath);

    const content = await ctx.fileAccess.readText(fullPath);
    // Use the blazing fast Rust chunker backend!
    const { pairs, metadata } = await invoke<{
      pairs: Array<{ embedding: string; generation: string }>;
      metadata: Record<string, any> | null;
    }>("chunk_text_paired", { content });

    const result: ChunkWithHash[] = [];

    for (let i = 0; i < pairs.length; i++) {
      // Hash computed from embeddingText — canonical, invariant to formatting diffs
      const hash = await this.hashService.compute(pairs[i].embedding);
      result.push({
        hash,
        embeddingText: pairs[i].embedding,
        generationText: pairs[i].generation,
        // backward-compat .text = embeddingText (consumers migrating to explicit fields)
        text: pairs[i].embedding,
        index: i,
        sourcePath,
        metadata: metadata ?? undefined,
      });
    }

    ctx.logger.log("info", LogScope.CHUNKING_DONE, `paired ${result.length} chunks (embed+gen)`, {
      chunkCount: result.length,
      hasMetadata: !!metadata,
    });

    return result;
  }
}

// src/core/refinery/taxonomy/TaxonomyGenerator.ts
import type { LLMService } from "@ai/llm/LLMService";
import type { Logger } from "@logging/Logger";
import type { ChunkWithHash } from "../types/chunk-decision";
import type { ProposedTaxonomy, TaxonomyNode } from "./types/taxonomy";
import { LogScope } from "../types/refinery-stage";
import { buildTaxonomyPrompt } from "@ai/prompts/refinery/taxonomy-generate";
import { REFINERY_CONSTANTS } from "../constants";
import { extractJsonObject } from "@retrieval/utils/jsonExtract";

export class TaxonomyGenerator {
  constructor(
    private llm: LLMService,
    private logger: Logger,
    private maxDepth = REFINERY_CONSTANTS.MAX_DEPTH_OF_GENERATED_TREE,
  ) {}

  async generate(chunks: ChunkWithHash[]): Promise<ProposedTaxonomy> {
    if (chunks.length === 0) {
      return { roots: [], rationale: "no chunks to organize" };
    }

    const messages = buildTaxonomyPrompt(
      chunks.map((c) => ({
        hash: c.hash,
        text: c.generationText.slice(0, REFINERY_CONSTANTS.MAX_CHUNK_PREVIEW_CHARS),
        sourcePath: c.sourcePath,
      })),
      this.maxDepth,
    );

    this.logger.log("info", LogScope.TAXONOMY_GENERATE, "requesting taxonomy from LLM", { chunkCount: chunks.length });

    const response = await this.llm.generateMessages(messages);
    const json = extractJsonObject(response.text);
    if (!json) {
      this.logger.log("error", LogScope.TAXONOMY_GENERATE, "failed to extract JSON", {
        preview: response.text.slice(0, 200),
      });
      throw new Error("TaxonomyGenerator: LLM did not return valid JSON");
    }

    let parsed: ProposedTaxonomy;
    try {
      parsed = JSON.parse(json);
    } catch (e) {
      this.logger.log("error", LogScope.TAXONOMY_GENERATE, "JSON.parse failed", {
        json: json.slice(0, 200),
        error: String(e),
      });
      throw new Error(`TaxonomyGenerator: invalid JSON from LLM: ${String(e).slice(0, 100)}`);
    }

    this.normalize(parsed.roots);

    this.logger.log("info", LogScope.TAXONOMY_GENERATE, "taxonomy generated", {
      rootCount: parsed.roots.length,
      rationale: parsed.rationale.slice(0, 100),
    });

    return parsed;
  }

  private normalize(nodes: TaxonomyNode[] | undefined): void {
    if (!nodes) return;
    for (const n of nodes) {
      n.children ??= [];
      n.assignedChunks ??= [];
      n.description ??= "";
      this.normalize(n.children);
    }
  }
}

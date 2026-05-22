// src/core/refinery/placement/PlacementPlanner.ts
import type { LLMService } from "@ai/llm/LLMService";
import type { Logger } from "@logging/Logger";
import type { ChunkWithHash } from "../types/chunk-decision";
import type { ProposedTaxonomy, TaxonomyNode } from "../taxonomy/types/taxonomy";
import type { PlacementPlan } from "./types/placement";
import { LogScope } from "../types/refinery-stage";
import { buildPlacementPrompt } from "@ai/prompts/refinery/placement-decide";
import { extractJsonObject } from "@retrieval/utils/jsonExtract";

export class PlacementPlanner {
  constructor(
    private llm: LLMService,
    private logger: Logger,
  ) {}

  async plan(proposed: ProposedTaxonomy, existingTree: string, chunks: ChunkWithHash[]): Promise<PlacementPlan> {
    if (chunks.length === 0) {
      return { decisions: [], foldersToCreate: [], rationale: "no chunks to place" };
    }

    const proposedTreeStr = this.treeToString(proposed.roots);

    const messages = buildPlacementPrompt(
      proposedTreeStr,
      existingTree,
      chunks.map((c) => ({ hash: c.hash, text: c.generationText })),
    );

    this.logger.log("info", LogScope.PLACEMENT_PLAN, "requesting placement from LLM", { chunkCount: chunks.length });

    const response = await this.llm.generateMessages(messages);
    const json = extractJsonObject(response.text);
    if (!json) {
      this.logger.log("error", LogScope.PLACEMENT_PLAN, "failed to extract JSON", {
        preview: response.text.slice(0, 200),
      });
      throw new Error("PlacementPlanner: LLM did not return valid JSON");
    }

    let plan: PlacementPlan;
    try {
      plan = JSON.parse(json);
    } catch (e) {
      this.logger.log("error", LogScope.PLACEMENT_PLAN, "JSON.parse failed", {
        json: json.slice(0, 200),
        error: String(e),
      });
      throw new Error(`PlacementPlanner: invalid JSON from LLM: ${String(e).slice(0, 100)}`);
    }

    plan.decisions ??= [];
    plan.foldersToCreate ??= [];
    plan.rationale ??= "";

    this.logger.log("info", LogScope.PLACEMENT_PLAN, "placement planned", {
      decisions: plan.decisions.length,
      foldersToCreate: plan.foldersToCreate.length,
    });

    return plan;
  }

  private treeToString(nodes: TaxonomyNode[] | undefined, indent = 0): string {
    if (!nodes || nodes.length === 0) return "(empty)";
    return nodes
      .map((n) => {
        const chunks = n.assignedChunks ?? [];
        const children = n.children ?? [];
        const line = "  ".repeat(indent) + `- ${n.name}/ (${chunks.length} chunks)`;
        const sub = this.treeToString(children, indent + 1);
        return sub ? line + "\n" + sub : line;
      })
      .join("\n");
  }
}

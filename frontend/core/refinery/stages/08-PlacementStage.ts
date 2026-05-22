// src/core/refinery/stages/PlacementStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { ChunkWithHash } from "../types/chunk-decision";
import type { ProposedTaxonomy } from "../taxonomy/types/taxonomy";
import type { PlacementPlan, PlacementDecision } from "../placement/types/placement";
import { LogScope } from "../types/refinery-stage";
import { PlacementPlanner } from "../placement/PlacementPlanner";
import { ConflictResolver } from "../placement/ConflictResolver";
import { REFINERY_CONSTANTS } from "../constants";

export interface PlacementInput {
  taxonomy: ProposedTaxonomy;
  chunks: ChunkWithHash[];
  existingTree: string;
}

export class PlacementStage implements RefineryStage<PlacementInput, PlacementPlan> {
  readonly name = "PlacementStage";

  async run(input: PlacementInput, ctx: RefineryContext): Promise<PlacementPlan> {
    const planner = new PlacementPlanner(ctx.llm, ctx.logger);
    const resolver = new ConflictResolver(ctx.fileAccess);
    const maxPerCall = REFINERY_CONSTANTS.MAX_CHUNKS_PER_PLACEMENT_CALL;

    if (input.chunks.length <= maxPerCall) {
      const rawPlan = await planner.plan(input.taxonomy, input.existingTree, input.chunks);
      const resolvedDecisions = await resolver.resolve(rawPlan.decisions);
      return { ...rawPlan, decisions: resolvedDecisions };
    }

    ctx.logger.log("info", LogScope.PLACEMENT_BATCH, `batching ${input.chunks.length} chunks`, { maxPerCall });

    const batchCount = Math.ceil(input.chunks.length / maxPerCall);
    const allDecisions: PlacementDecision[] = [];
    const allFolders = new Set<string>();
    const rationales: string[] = [];

    for (let i = 0; i < batchCount; i++) {
      const batch = input.chunks.slice(i * maxPerCall, (i + 1) * maxPerCall);
      const rawPlan = await planner.plan(input.taxonomy, input.existingTree, batch);
      allDecisions.push(...rawPlan.decisions);
      rawPlan.foldersToCreate.forEach(f => allFolders.add(f));
      rationales.push(rawPlan.rationale);
    }

    const resolvedDecisions = await resolver.resolve(allDecisions);
    return { decisions: resolvedDecisions, foldersToCreate: [...allFolders], rationale: rationales.join(" | ") };
  }
}

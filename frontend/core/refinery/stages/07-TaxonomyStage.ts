// src/core/refinery/stages/TaxonomyStage.ts
import type { RefineryStage } from "../types/refinery-stage";
import type { RefineryContext } from "../types/refinery-context";
import type { ChunkWithHash } from "../types/chunk-decision";
import type { ProposedTaxonomy, TaxonomyNode } from "../taxonomy/types/taxonomy";
import { LogScope } from "../types/refinery-stage";
import { TaxonomyGenerator } from "../taxonomy/TaxonomyGenerator";
import { REFINERY_CONSTANTS } from "../constants";

export class TaxonomyStage implements RefineryStage<ChunkWithHash[], ProposedTaxonomy> {
  readonly name = "TaxonomyStage";

  async run(chunks: ChunkWithHash[], ctx: RefineryContext): Promise<ProposedTaxonomy> {
    const generator = new TaxonomyGenerator(ctx.llm, ctx.logger);

    if (chunks.length <= REFINERY_CONSTANTS.MAX_CHUNKS_PER_TAXONOMY_CALL) {
      return generator.generate(chunks);
    }

    ctx.logger.log("info", LogScope.TAXONOMY_BATCH, `batching ${chunks.length} chunks`, {
      maxPerCall: REFINERY_CONSTANTS.MAX_CHUNKS_PER_TAXONOMY_CALL,
    });

    const batchCount = Math.ceil(chunks.length / REFINERY_CONSTANTS.MAX_CHUNKS_PER_TAXONOMY_CALL);
    const taxonomies: ProposedTaxonomy[] = [];

    for (let i = 0; i < batchCount; i++) {
      const batch = chunks.slice(
        i * REFINERY_CONSTANTS.MAX_CHUNKS_PER_TAXONOMY_CALL,
        (i + 1) * REFINERY_CONSTANTS.MAX_CHUNKS_PER_TAXONOMY_CALL,
      );
      taxonomies.push(await generator.generate(batch));
    }

    return this.mergeTaxonomies(taxonomies);
  }

  private mergeTaxonomies(taxonomies: ProposedTaxonomy[]): ProposedTaxonomy {
    if (taxonomies.length === 1) return taxonomies[0];

    const rootMap = new Map<string, TaxonomyNode>();
    for (const t of taxonomies) {
      for (const root of t.roots) {
        const existing = rootMap.get(root.name);
        if (existing) {
          existing.assignedChunks.push(...root.assignedChunks);
          existing.children = this.mergeChildren(existing.children, root.children);
          if (root.description && !existing.description) existing.description = root.description;
        } else {
          rootMap.set(root.name, { ...root, children: [...root.children], assignedChunks: [...root.assignedChunks] });
        }
      }
    }

    return {
      roots: [...rootMap.values()],
      rationale: taxonomies.map(t => t.rationale).join(" | "),
    };
  }

  private mergeChildren(existing: TaxonomyNode[], incoming: TaxonomyNode[]): TaxonomyNode[] {
    const childMap = new Map<string, TaxonomyNode>();
    for (const c of existing) childMap.set(c.name, c);
    for (const c of incoming) {
      const prev = childMap.get(c.name);
      if (prev) {
        prev.assignedChunks.push(...c.assignedChunks);
        prev.children = this.mergeChildren(prev.children, c.children);
      } else {
        childMap.set(c.name, c);
      }
    }
    return [...childMap.values()];
  }
}

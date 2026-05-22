// src/core/refinery/RefineryPipeline.ts
//
// INTERNAL ORCHESTRATOR — glues stages together. Not exposed in public API.
// Stages: Chunk → SubSplit → Consolidate → Dedupe → Atomize → Enrich → Taxonomy → Placement → Write.
// External consumers use RefineryService, not this class directly.

import * as path from "path";
import type { RefineryContext } from "./types/refinery-context";
import type { ChunkWithHash } from "./types/chunk-decision";
import type { AtomChunk } from "./types/atom-chunk";
import type { ProposedTaxonomy } from "./taxonomy/types/taxonomy";
import type { PlacementPlan } from "./placement/types/placement";
import type { RefineryResult } from "./types/refinery-result";
import { LogScope } from "./types/refinery-stage";
import { ChunkingStage } from "./stages/01-ChunkingStage";
import { SubSplitStage } from "./stages/02-SubSplitStage";
import { ConsolidationStage } from "./stages/03-ConsolidationStage";
import { DeduplicationStage } from "./stages/04-DeduplicationStage";
import { AtomizationStage } from "./stages/05-AtomizationStage";
import { EnrichmentStage } from "./stages/06-EnrichmentStage";
import { TaxonomyStage } from "./stages/07-TaxonomyStage";
import { PlacementStage } from "./stages/08-PlacementStage";
import type { PlacementInput } from "./stages/08-PlacementStage";
import { WriteStage } from "./stages/09-WriteStage";
import type { WriteInput } from "./stages/09-WriteStage";
import { FolderTreeReader } from "./taxonomy/FolderTreeReader";

const TOTAL_STAGES = 9;

export class RefineryPipeline {
  private chunkingStage = new ChunkingStage();
  private subSplitStage = new SubSplitStage();
  private consolidationStage = new ConsolidationStage();
  private deduplicationStage = new DeduplicationStage();
  private atomizationStage = new AtomizationStage();
  private enrichmentStage = new EnrichmentStage();
  private taxonomyStage = new TaxonomyStage();
  private placementStage = new PlacementStage();
  private writeStage = new WriteStage();

  constructor(private ctx: RefineryContext) {
    if (!this.ctx.dbConnection) {
      this.ctx.dbConnection = {
        withTransaction: async <T>(fn: () => Promise<T>): Promise<T> => {
          return await fn();
        }
      } as any;
    }
  }

  async refine(sourcePath: string, opts?: { dryRun?: boolean }): Promise<RefineryResult> {
    const dryRun = opts?.dryRun ?? this.ctx.dryRun;
    const traceId = this.ctx.logger.startTrace(`refine:${sourcePath}`);

    try {
      return await this.ctx.dbConnection.withTransaction(async () => {
        // Stage 1: Chunk raw md → ChunkWithHash[]
        this.logStage(1);
        const rawChunks = await this.timeStage("chunking",
          () => this.chunkingStage.run(sourcePath, this.ctx),
        );

        // Stage 2: SubSplit oversized semantic chunks → AtomChunk[]
        this.logStage(2);
        let chunks: AtomChunk[] = rawChunks.map(c => ({ ...c, isTable: false }));
        chunks = await this.timeStage("subsplit",
          () => this.subSplitStage.run(chunks, this.ctx),
        );

        // Stage 3: Consolidate near-identical chunks within same document
        this.logStage(3);
        chunks = await this.timeStage("consolidation",
          () => this.consolidationStage.run(chunks, this.ctx),
        );

        // Stage 4: Deduplicate against output vault
        this.logStage(4);
        const dedupResult = await this.timeStage("deduplication",
          () => this.deduplicationStage.run(chunks as ChunkWithHash[], this.ctx),
        );
        
        // Keep both "keep" and "merge" chunks for atomization and enrichment
        const activeChunks = dedupResult.decisions
          .filter(d => d.action === "keep" || d.action === "merge")
          .map(d => d.action === "keep" ? d.chunk : d.sourceChunk) as AtomChunk[];

        // Stage 5: Atomize — generate headings + filenames
        this.logStage(5);
        let processedChunks = await this.timeStage("atomization",
          () => this.atomizationStage.run(activeChunks, this.ctx),
        );

        // Stage 6: Enrich — generate aliases + tags
        this.logStage(6);
        processedChunks = await this.timeStage("enrichment",
          () => this.enrichmentStage.run(processedChunks, this.ctx),
        );

        // Separate chunks that need placement (keep) vs those already placed (merge)
        const keepHashes = new Set(dedupResult.remaining.map(c => c.hash));
        const chunksToPlace = processedChunks.filter(c => keepHashes.has(c.hash));
        const mergedProcessedChunks = processedChunks.filter(c => !keepHashes.has(c.hash));

        // Stage 7: Taxonomy — LLM proposes folder tree
        this.logStage(7);
        const taxonomy = await this.timeStage("taxonomy",
          () => this.taxonomyStage.run(chunksToPlace as ChunkWithHash[], this.ctx),
        );

        // Stage 8: Placement — LLM decides file locations
        this.logStage(8);
        const folderReader = new FolderTreeReader(this.ctx.fileAccess);
        const existingTree = await folderReader.readTree(this.ctx.outputRoot);

        const placementInput: PlacementInput = { taxonomy, chunks: chunksToPlace as ChunkWithHash[], existingTree };
        let placement = await this.timeStage("placement",
          () => this.placementStage.run(placementInput, this.ctx),
        );

        // Add pre-determined merge decisions
        const mergeDecisionsMap = new Map(
          dedupResult.decisions.filter(d => d.action === "merge").map(d => [d.sourceChunk.hash, d.targetPath])
        );

        for (const mc of mergedProcessedChunks) {
          const targetPath = mergeDecisionsMap.get(mc.hash);
          if (targetPath) {
            placement.decisions.push({
              chunkHash: mc.hash,
              outputPath: targetPath,
              action: "merge",
              reason: "similar to existing — merge directly",
              existingTarget: targetPath
            });
          }
        }

        // Stage 9: Write
        this.logStage(9);
        const writeInput: WriteInput = { plan: placement, chunks: processedChunks as ChunkWithHash[], dryRun, sourcePath };
        const operations = await this.timeStage("write",
          () => this.writeStage.run(writeInput, this.ctx),
        );

        await this.ctx.logger.endTrace({
          sourcePath,
          chunkCount: rawChunks.length,
          keptChunks: dedupResult.remaining.length,
          operations: operations.length,
        });

        return {
          sourcePath,
          chunks: rawChunks, // original chunks for diagnostics
          dedup: dedupResult,
          taxonomy,
          placement,
          operations,
          dryRun,
        };
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.stack ?? err.message : String(err);
      this.ctx.logger.log("error", LogScope.PIPELINE, `refine failed: ${errorMsg}`);
      await this.ctx.logger.endTrace({ error: errorMsg });
      throw err;
    }
  }

  async plan(sourcePath: string): Promise<{
    chunks: ChunkWithHash[];
    dedup: import("./types/chunk-decision").DeduplicationResult;
    taxonomy: ProposedTaxonomy;
    placement: PlacementPlan;
  }> {
    const rawChunks = await this.chunkingStage.run(sourcePath, this.ctx);
    let chunks: AtomChunk[] = rawChunks.map(c => ({ ...c, isTable: false }));
    chunks = await this.subSplitStage.run(chunks, this.ctx);
    chunks = await this.consolidationStage.run(chunks, this.ctx);

    const dedup = await this.deduplicationStage.run(chunks as ChunkWithHash[], this.ctx);
    chunks = dedup.remaining as AtomChunk[];

    chunks = await this.atomizationStage.run(chunks, this.ctx);
    chunks = await this.enrichmentStage.run(chunks, this.ctx);

    const taxonomy = await this.taxonomyStage.run(chunks as ChunkWithHash[], this.ctx);
    const folderReader = new FolderTreeReader(this.ctx.fileAccess);
    const existingTree = await folderReader.readTree(this.ctx.outputRoot);
    const placement = await this.placementStage.run(
      { taxonomy, chunks: chunks as ChunkWithHash[], existingTree },
      this.ctx,
    );

    return { chunks: chunks as ChunkWithHash[], dedup, taxonomy, placement };
  }

  private logStage(n: number): void {
    this.ctx.logger.log("info", LogScope.PIPELINE, `stage ${n}/${TOTAL_STAGES}`);
  }

  private async timeStage<T>(name: string, fn: () => Promise<T>): Promise<T> {
    return this.ctx.logger.time(`pipeline.${name}`, fn);
  }
}

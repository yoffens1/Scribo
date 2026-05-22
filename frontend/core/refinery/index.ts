// src/core/refinery/index.ts
//
// Public API barrel for the refinery module.
// Import only RefineryService and types from here.

export { RefineryService } from "./RefineryService";
export type { RefineryServiceOptions } from "./RefineryService";

export { RefineryPipeline } from "./RefineryPipeline";

// Types
export type { RefineryStage, AnyRefineryStage } from "./types/refinery-stage";
export type { RefineryContext } from "./types/refinery-context";
export type { ChunkDecision, ChunkWithHash, DeduplicationResult } from "./types/chunk-decision";
export type { AtomChunk } from "./types/atom-chunk";
export type { RefineryResult, WriteOperation, BatchRefineryResult } from "./types/refinery-result";
export type { TaxonomyNode, ProposedTaxonomy } from "./taxonomy/types/taxonomy";
export type { PlacementDecision, PlacementPlan } from "./placement/types/placement";

// src/core/refinery/taxonomy/TaxonomyMerger.ts
import type { TaxonomyNode, ProposedTaxonomy } from "./types/taxonomy";
import type { Logger } from "@logging/Logger";

/**
 * Matches proposed taxonomy nodes against existing folder names.
 * Produces a mapping: proposed_name → existing_folder (or null if new).
 */
export class TaxonomyMerger {
  constructor(private logger: Logger) {}

  /**
   * Compare proposed taxonomy with existing folders and produce
   * a merge mapping. Simple string-based matching for now.
   */
  match(
    proposed: ProposedTaxonomy,
    existingFolders: string[],
  ): Map<string, string | null> {
    const mapping = new Map<string, string | null>();

    const flatten = (nodes: TaxonomyNode[], prefix = ""): void => {
      for (const node of nodes) {
        const fullPath = prefix ? `${prefix}/${node.name}` : node.name;
        const match = this.findBestMatch(fullPath, existingFolders);
        mapping.set(fullPath, match);
        flatten(node.children, fullPath);
      }
    };

    flatten(proposed.roots);

    this.logger.log("debug", "taxonomy.merge", "matched proposed to existing", {
      proposedCount: mapping.size,
      matched: [...mapping.values()].filter((v) => v !== null).length,
      new: [...mapping.values()].filter((v) => v === null).length,
    });

    return mapping;
  }

  private findBestMatch(
    proposed: string,
    existing: string[],
  ): string | null {
    const lowerProposed = proposed.toLowerCase();

    // 1. Exact match (highest priority)
    const exact = existing.find((e) => e.toLowerCase() === lowerProposed);
    if (exact) return exact;

    // 2. Suffix match (e.g. "machine-learning" exactly matches the end of "ai/machine-learning")
    const suffixMatch = existing.find((e) => e.toLowerCase().endsWith("/" + lowerProposed));
    if (suffixMatch) return suffixMatch;

    // 3. Fuzzy Levenshtein match
    let bestMatch: string | null = null;
    let maxSimilarity = 0;

    for (const e of existing) {
      const lowerExisting = e.toLowerCase();
      
      // Calculate similarity for the full path
      const fullSim = this.calculateSimilarity(lowerProposed, lowerExisting);
      
      // Calculate similarity comparing only the last folder names (basenames)
      const proposedBasename = lowerProposed.split("/").pop() || lowerProposed;
      const existingBasename = lowerExisting.split("/").pop() || lowerExisting;
      const basenameSim = this.calculateSimilarity(proposedBasename, existingBasename);
      
      // Use the best of full path similarity vs basename similarity
      const sim = Math.max(fullSim, basenameSim);

      if (sim > maxSimilarity) {
        maxSimilarity = sim;
        bestMatch = e;
      }
    }

    // Require at least 80% similarity to accept a fuzzy match
    if (maxSimilarity >= 0.80) {
      return bestMatch;
    }

    return null;
  }

  /**
   * Calculates string similarity based on Levenshtein distance.
   * Returns a value between 0.0 (completely different) and 1.0 (exact match).
   */
  private calculateSimilarity(a: string, b: string): number {
    if (a.length === 0) return b.length === 0 ? 1.0 : 0.0;
    if (b.length === 0) return 0.0;

    const matrix: number[][] = [];
    
    for (let i = 0; i <= b.length; i++) {
      matrix[i] = [i];
    }
    for (let j = 0; j <= a.length; j++) {
      matrix[0][j] = j;
    }

    for (let i = 1; i <= b.length; i++) {
      for (let j = 1; j <= a.length; j++) {
        if (b.charAt(i - 1) === a.charAt(j - 1)) {
          matrix[i][j] = matrix[i - 1][j - 1];
        } else {
          matrix[i][j] = Math.min(
            matrix[i - 1][j - 1] + 1, // substitution
            matrix[i][j - 1] + 1,     // insertion
            matrix[i - 1][j] + 1      // deletion
          );
        }
      }
    }

    const distance = matrix[b.length][a.length];
    const maxLength = Math.max(a.length, b.length);
    return 1.0 - (distance / maxLength);
  }
}

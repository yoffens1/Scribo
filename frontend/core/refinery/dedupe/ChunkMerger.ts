// src/core/refinery/dedupe/ChunkMerger.ts
import type { ChunkWithHash } from "../types/chunk-decision";
import type { MergeStrategy } from "./strategies/ExactMatchStrategy";
import { ExactMatchStrategy } from "./strategies/ExactMatchStrategy";
import { AppendStrategy } from "./strategies/AppendStrategy";
import type { IFileAccess } from "@utils/_types";
import type { Logger } from "@logging/Logger";
import { LogScope } from "../types/refinery-stage";

export class ChunkMerger {
  private strategies: MergeStrategy[];

  constructor(
    private fileAccess: IFileAccess,
    private logger: Logger,
    semanticMerge: MergeStrategy | null = null,
  ) {
    this.strategies = [
      new ExactMatchStrategy(),
      ...(semanticMerge ? [semanticMerge] : []),
      new AppendStrategy(),
    ];
  }

  async merge(targetPath: string, incoming: ChunkWithHash): Promise<string> {
    const existing = await this.fileAccess.readText(targetPath);

    for (const strategy of this.strategies) {
      if (strategy.canHandle(existing, incoming)) {
        this.logger.log("debug", LogScope.DEDUPE_MERGE, `using ${strategy.name}`, {
          targetPath, chunkHash: incoming.hash,
        });
        return strategy.merge(existing, incoming);
      }
    }

    return existing + "\n\n" + incoming.generationText;
  }
}

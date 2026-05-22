// src/core/refinery/dedupe/strategies/SemanticMergeStrategy.ts
import type { ChunkWithHash } from "../../types/chunk-decision";
import type { MergeStrategy } from "./ExactMatchStrategy";
import type { LLMService } from "@ai/llm/LLMService";
import { buildChunkMergePrompt } from "@ai/prompts/refinery/chunk-merge";

/**
 * LLM-powered merge — combines two semantically similar chunks into one.
 */
export class SemanticMergeStrategy implements MergeStrategy {
  readonly name = "semantic-merge";

  constructor(private llm: LLMService) {}

  canHandle(_existing: string, _incoming: ChunkWithHash): boolean {
    // This strategy is the fallback for non-exact matches
    return true;
  }

  async merge(existing: string, incoming: ChunkWithHash): Promise<string> {
    const messages = buildChunkMergePrompt(existing, incoming.generationText);
    const response = await this.llm.generateMessages(messages);
    return response.text.trim();
  }
}

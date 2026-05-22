// src/test/refinery/helpers/mockCtx.ts
import type { RefineryContext } from "@refinery/types/refinery-context";
import type { Logger } from "@logging/Logger";
import { nullLogger } from "./nullLogger";
import { fakeFs } from "./fakeFs";

export interface MockCtxOptions {
  dryRun?: boolean;
  outputRoot?: string;
  inboxRoot?: string;
  llmResponses?: {
    taxonomy?: string;
    placement?: string;
  };
  fileContents?: Record<string, string>;
  logger?: Logger;
  /** Custom retrieval mock (default: returns empty results) */
  retrievalMock?: {
    query?: any;
  };
}

const defaultTaxonomyJson = '{"roots":[],"rationale":"auto"}';
const defaultPlacementJson = '{"decisions":[],"foldersToCreate":[],"rationale":"auto"}';

/**
 * Build a mock RefineryContext for stage/pipeline tests.
 * All fields have safe defaults; override only what you need.
 */
export const makeMockCtx = (opts: MockCtxOptions = {}): RefineryContext => {
  const fileContents = opts.fileContents ?? {};
  const retrieval = opts.retrievalMock ?? {
    query: async (_text: string, _opts?: any) => [],
  } as any;

  let callCount = 0;
  const llm = {
    generateMessages: async (messages: any[]) => {
      callCount++;
      const fullContent = messages.map((m: any) => m.content ?? "").join(" ");
      // Taxonomy prompt contains "roots" in system message
      if (fullContent.includes("\"roots\"")) {
        return { text: opts.llmResponses?.taxonomy ?? defaultTaxonomyJson };
      }
      // Placement prompt contains "PROPOSED TREE"
      if (fullContent.includes("PROPOSED TREE")) {
        return { text: opts.llmResponses?.placement ?? opts.llmResponses?.taxonomy ?? defaultPlacementJson };
      }
      // Atomize filename prompt contains "Filename:"
      if (fullContent.includes("Filename:")) {
        return { text: "test-name.md" };
      }
      // Enrich aliases prompt contains "Aliases:"
      if (fullContent.includes("Aliases:")) {
        return { text: "[]" };
      }
      // Enrich tags prompt contains "Tags:"
      if (fullContent.includes("Tags:")) {
        return { text: "[]" };
      }
      // Atomize heading prompt contains "Heading:" or "question-style heading"
      if (fullContent.includes("question-style heading") || fullContent.includes("Heading:")) {
        return { text: "## Test heading" };
      }
      return { text: "## Test heading" };
    },
  } as any;

  return {
    fileAccess: fakeFs(fileContents),
    retrieval,
    llm,
    logger: opts.logger ?? nullLogger(),
    outputRoot: opts.outputRoot ?? "output",
    inboxRoot: opts.inboxRoot ?? "inbox",
    dryRun: opts.dryRun ?? true,
  };
};

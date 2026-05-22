// src/core/rag/RetrievalFactory.ts
import type { Retriever } from "./types";
import { EmbeddingRetriever } from "./EmbeddingRetriever";
import { KeywordRetriever } from "./KeywordRetriever";
import { HybridRetriever } from "./HybridRetriever";
import { MultiQueryRetriever } from "./MultiQueryRetriever";
import type { Embedder } from "@ai/embedding/Embedder";
import type { ChunkSource } from "../types/chunk-source";
import type { RetrievalConfig } from "./types";
import type { Reranker } from "../rerankers/types/reranker";
import { IndexRegistry } from "../IndexRegistry";
import { RerankingRetriever } from "./RerankingRetriever";
import { LlmReranker } from "../rerankers/LlmReranker";
import { ListwiseLlmReranker } from "../rerankers/ListwiseLlmReranker";
import { NoopReranker } from "../rerankers/NoopReranker";
import { QueryPipeline } from "../pipeline/QueryPipeline";
import { LanguageDetectionStage } from "../pipeline/stages/LanguageDetectionStage";
import { TranslationStage } from "../pipeline/stages/TranslationStage";
import { SynonymExpansionStage } from "../pipeline/stages/SynonymExpansionStage";
import { LlmSynonymStage } from "../pipeline/stages/LlmSynonymStage";
import { HydeStage } from "../pipeline/stages/HydeStage";
import { VaultLanguageStats } from "../pipeline/VaultLanguageStats";
import { SYNONYM_DICT } from "../pipeline/dictionaries/synonyms";
import type { Translator } from "@translation/Translator";
import type { LLMService } from "@ai/llm/LLMService";
import { RetrievalLogger } from "../logging/RetrievalLogger";

/**
 * Creates Retriever instances. Uses IndexRegistry for lazy + cached
 * index building — no per-call rebuild of BM25/vector indices.
 */
export class RetrievalFactory {
  static create(
    config: RetrievalConfig,
    chunkSource: ChunkSource,
    embedder: Embedder,
    registry?: IndexRegistry,
    reranker?: Reranker,
    translator?: Translator,
    llm?: LLMService,
    logger?: RetrievalLogger,
  ): Retriever {
    const reg = registry ?? new IndexRegistry(chunkSource, embedder);
    let r = RetrievalFactory.buildBaseRetriever(config, reg, logger);
    r = RetrievalFactory.wrapWithPipeline(r, config, chunkSource, translator, llm, logger);
    r = RetrievalFactory.wrapWithReranker(r, config, reranker, llm, chunkSource, logger);
    return r;
  }

  // ── Builders ──

  private static buildBaseRetriever(
    config: RetrievalConfig,
    reg: IndexRegistry,
    logger?: RetrievalLogger,
  ): Retriever {
    // Factory lambdas — always return the latest engine/index from registry
    const embedding = new EmbeddingRetriever(() => reg.getSearchEngine());
    const keyword = new KeywordRetriever(() => reg.getBm25Index());

    switch (config.mode) {
      case "embedding": return embedding;
      case "keyword":   return keyword;
      case "hybrid":
      default:
        return new HybridRetriever(embedding, keyword, 60, 3, config.embeddingWeight ?? 1, logger);
    }
  }

  private static wrapWithPipeline(
    base: Retriever,
    config: RetrievalConfig,
    chunkSource: ChunkSource,
    translator?: Translator,
    llm?: LLMService,
    logger?: RetrievalLogger,
  ): Retriever {
    if (!config.pipeline) return base;
    const pipeline = RetrievalFactory.buildPipeline(config, chunkSource, translator, llm, logger);
    if (!pipeline) return base;
    return new MultiQueryRetriever(base, pipeline, 60, 3, logger);
  }

  private static wrapWithReranker(
    base: Retriever,
    config: RetrievalConfig,
    external?: Reranker,
    llm?: LLMService,
    chunkSource?: ChunkSource,
    logger?: RetrievalLogger,
  ): Retriever {
    const reranker = RetrievalFactory.buildReranker(config, external, llm, logger);
    if (!reranker || reranker instanceof NoopReranker) return base;
    return new RerankingRetriever(base, reranker, chunkSource, 4, logger);
  }

  // ── Pipeline construction ──

  private static buildPipeline(
    config: RetrievalConfig,
    chunkSource: ChunkSource,
    translator?: Translator,
    llm?: LLMService,
    logger?: RetrievalLogger,
  ): QueryPipeline | null {
    const stages = [];
    const p = config.pipeline!;

    if (!p.autoTranslate && p.expandSynonyms === "off" && !p.hyde) return null;

    if (p.autoTranslate) {
      stages.push(new LanguageDetectionStage());
    }

    if (p.autoTranslate && translator) {
      const vaultLangStats = new VaultLanguageStats(chunkSource);
      stages.push(new TranslationStage(translator, () => vaultLangStats.getLanguage()));
    }

    if (p.expandSynonyms === "static") {
      const dict = p.synonymDict ?? SYNONYM_DICT;
      stages.push(new SynonymExpansionStage(dict));
    } else if (p.expandSynonyms === "llm" && llm) {
      stages.push(new LlmSynonymStage(llm));
    }

    if (p.hyde && llm) {
      stages.push(new HydeStage(llm));
    }

    return stages.length > 0 ? new QueryPipeline(stages, logger) : null;
  }

  // ── Reranker construction ──

  private static buildReranker(
    config: RetrievalConfig,
    external?: Reranker,
    llm?: LLMService,
    logger?: RetrievalLogger,
  ): Reranker | undefined {
    if (external) return external;
    if (!config.aiRerank?.enabled || !llm) return undefined;

    const maxCandidates = config.aiRerank.maxCandidates ?? 25;
    return config.aiRerank.mode === "listwise"
      ? new ListwiseLlmReranker(llm, maxCandidates, logger)
      : new LlmReranker(llm, maxCandidates, logger);
  }
}

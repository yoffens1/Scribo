// src/core/retrieval/RetrievalService.ts
import type { ChunkSource } from "./types/chunk-source";
import type { FetchQuery, FetchResult, Retriever, RetrieveOptions, RetrievalConfig } from "./retrievers/types";
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "./types/chunk";
import type { SearchResult } from "./types/search";
import type { Reranker } from "./rerankers/types/reranker";
import { IndexRegistry } from "./IndexRegistry";
import { RetrievalFactory } from "./retrievers/RetrievalFactory";
import { NoopReranker } from "./rerankers/NoopReranker";
import type { Embedder } from "@ai/embedding/Embedder";
import type { Translator } from "@translation/Translator";
import type { LLMService } from "@ai/llm/LLMService";
import type { DbEventBus, ChunkInsertedPayload, ChunkDeletedPayload } from "../database/EventBus";
import { RetrievalLogger } from "./logging/RetrievalLogger";
import type { Sink } from "@logging/types";
import { ConsoleSink } from "@logging/sinks/ConsoleSink";
import { MemorySink } from "@logging/sinks/MemorySink";
import { FileSink } from "@logging/sinks/FileSink";

/** Union of all chunk return types from ChunkSource. */
type AnyChunk = ChunkData | ChunkDataWithPath | FullChunkData;

export interface RetrievalServiceOptions {
  source: ChunkSource;
  config?: RetrievalConfig;
  /** External reranker override — factory builds from config.aiRerank by default. */
  reranker?: Reranker;
  translator?: Translator;
  llm?: LLMService;
  eventBus?: DbEventBus;
}

/**
 * Main public facade for the retrieval layer.
 *
 * Two operations:
 *   query(text, opts)  — text search through Retriever (semantic/keyword/hybrid)
 *   fetch(query)       — file-scoped chunk fetch from DB (no ranking)
 *
 * Nothing outside core/retrieval/ should import Retriever, RetrievalFactory,
 * or SearchEngine directly — only RetrievalService.
 */
export class RetrievalService {
  private source: ChunkSource;
  private config: RetrievalConfig;
  private reranker: Reranker;
  private translator?: Translator;
  private llm?: LLMService;
  private eventBus?: DbEventBus;
  private registry: IndexRegistry;
  private retriever: Retriever | null = null;
  private embedder: Embedder | null = null;
  readonly logger: RetrievalLogger;

  constructor(opts: RetrievalServiceOptions) {
    this.source = opts.source;
    this.config = opts.config ?? { mode: "hybrid" };
    this.reranker = opts.reranker ?? new NoopReranker();
    this.translator = opts.translator;
    this.llm = opts.llm;
    this.eventBus = opts.eventBus;
    this.registry = new IndexRegistry(opts.source, null);
    this.logger = RetrievalService.buildLogger(this.config);

    if (opts.eventBus) {
      opts.eventBus.on<ChunkInsertedPayload>("chunk:inserted", () => {
        this.registry.markDirty();
        // Retriever chain uses factory lambdas → no need to recreate on dirty
      });
      opts.eventBus.on<ChunkDeletedPayload>("chunk:deleted", () => {
        this.registry.markDirty();
      });
    }
  }

  private static buildLogger(config: RetrievalConfig): RetrievalLogger {
    if (!config.logging?.enabled) {
      return new RetrievalLogger({ enabled: false });
    }
    const sinks: Sink[] = [];
    const requested = config.logging.sinks ?? ["console"];
    if (requested.includes("console")) sinks.push(new ConsoleSink());
    if (requested.includes("memory")) sinks.push(new MemorySink());
    if (requested.includes("file")) {
      sinks.push(new FileSink(config.logging.fileSink?.dir ?? "src/test/test-db/traces"));
    }
    return new RetrievalLogger({
      enabled: true,
      minLevel: config.logging.minLevel ?? "debug",
      sinks: sinks.length > 0 ? sinks : [new ConsoleSink()],
    });
  }

  /** Set embedder after construction (lazy wiring). */
  setEmbedder(embedder: Embedder): void {
    this.embedder = embedder;
    this.registry.setEmbedder(embedder);
  }

  private ensureRetriever(): Retriever {
    if (!this.retriever) {
      this.retriever = RetrievalFactory.create(
        this.config,
        this.source,
        this.registry.getEmbedder(),
        this.registry,
        this.reranker,
        this.translator,
        this.llm,
        this.logger,
      );
    }
    return this.retriever;
  }

  // ── Text search (main API) ──

  async query(text: string, opts?: RetrieveOptions): Promise<SearchResult[]> {
    if (!this.embedder) {
      throw new Error("RetrievalService: setEmbedder() must be called before query()");
    }
    this.logger.startTrace(text);
    let result: SearchResult[] = [];
    try {
      const retriever = this.ensureRetriever();
      result = await retriever.retrieve(text, opts);
      return result;
    } finally {
      await this.logger.endTrace({
        count: result.length,
        top: result.slice(0, 10).map(r => ({
          filePath: r.chunkRef.filePath,
          chunkIndex: r.chunkRef.chunkIndex,
          score: r.score,
        })),
      });
    }
  }

  // ── File fetch (DB lookup, no ranking) ──

  async fetch(query: FetchQuery = {}): Promise<FetchResult[]> {
    const includeDeleted = query.includeDeleted ?? false;

    let raw: AnyChunk[];

    if (query.filePath !== undefined) {
      raw = await this.source.getByFilePath(query.filePath, includeDeleted);
    } else if (query.fileName !== undefined) {
      raw = await this.source.getByFileName(query.fileName, includeDeleted);
    } else {
      raw = await this.source.getAll(includeDeleted);
    }

    const offset = query.offset ?? 0;
    const limit = query.limit ?? raw.length;
    const page = raw.slice(offset, offset + limit);

    return page.map((r) => ({
      chunkId: "chunkId" in r ? r.chunkId : undefined,
      filePath: "filePath" in r ? r.filePath : query.filePath ?? "",
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText,
      tokenCount: r.tokenCount,
      embedding: r.embedding ?? new Float32Array(0),
    }));
  }

  // ── Cache invalidation ──

  /** Force full index rebuild (model change, manual reindex). Immediate. */
  markDirty(): void {
    this.registry.markDirtyImmediate();
    this.retriever = null;
  }

  /** Release memory, clear timers — call on plugin unload. */
  dispose(): void {
    this.registry.dispose();
    this.retriever = null;
    this.embedder = null;
  }
}

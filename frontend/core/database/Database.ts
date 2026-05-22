// src/core/database/VectorDatabase.ts
import { Embedder } from "@ai/embedding/Embedder";
import { TauriDbConnection } from "./infrastructure/TauriDbConnection";
import { FileRepository } from "./repositories/FileRepository";
import { ChunkRepository } from "./repositories/ChunkRepository";
import { IndexingService } from "./services/indexing/IndexingService";
import { ReconciliationService } from "./services/ReconciliationService";
import { MaintenanceService } from "./services/MaintenanceService";
import { ReindexScheduler } from "./services/ReindexScheduler";
import { RetrievalService } from "../retrieval/RetrievalService";
import { LegacyRetrievalAdapter } from "../retrieval/LegacyRetrievalAdapter";
import type { FetchQuery, FetchResult } from "../retrieval/retrievers/types";
import type { ChunkData, ChunkDataWithPath, FullChunkData } from "../retrieval/types/chunk";
import { FileService } from "./services/FileService";
import { DbEventBus } from "./EventBus";

/**
 * Thin orchestration facade. Pure delegation — no business logic, no
 * transaction management, no queuing. All logic lives in services.
 */
export class VectorDatabase {
  private core: TauriDbConnection;
  private files: FileRepository;
  private chunks: ChunkRepository;
  private indexing: IndexingService;
  private reconciliation: ReconciliationService;
  private maintenance: MaintenanceService;
  private scheduler: ReindexScheduler;
  private retrieval: RetrievalService;
  private legacy: LegacyRetrievalAdapter;
  private eventBus: DbEventBus;
  private fileService: FileService;

  constructor(pluginDir: string, modelName?: string) {
    this.core = new TauriDbConnection(pluginDir, modelName);
    this.files = new FileRepository();
    this.chunks = new ChunkRepository();
    this.maintenance = new MaintenanceService(this.core);
    this.scheduler = new ReindexScheduler(this.core);
    this.eventBus = new DbEventBus();
    this.retrieval = new RetrievalService({ source: this.chunks, eventBus: this.eventBus });
    this.legacy = new LegacyRetrievalAdapter(this.chunks);
    this.fileService = new FileService(
      this.core,
      this.files,
      this.maintenance,
      this.scheduler,
      this.eventBus,
    );
    this.indexing = new IndexingService(
      this.core,
      this.files,
      this.chunks,
      this.maintenance,
      this.scheduler,
      this.eventBus,
    );
    this.reconciliation = new ReconciliationService(
      this.core,
      this.files,
      this.indexing,
      this.maintenance,
      this.scheduler,
    );
  }

  // ── Lifecycle ──

  async initialize(): Promise<void> {
    await this.core.initialize();
  }

  async close(): Promise<void> {
    this.scheduler.cleanup();
    this.maintenance.resetCounters();
    await this.core.close();
  }

  // ── File CRUD ──

  async hasFileChanged(
    filePath: string,
    fileHash: string,
    embeddingModel?: string,
    chunkingVersion?: string,
  ): Promise<boolean> {
    await this.core.ensureDb();
    return this.indexing.hasFileChanged(
      filePath,
      fileHash,
      embeddingModel,
      chunkingVersion,
    );
  }

  async softDeleteFile(filePath: string): Promise<void> {
    return this.fileService.softDelete(filePath);
  }

  async restoreFile(filePath: string): Promise<void> {
    return this.fileService.restore(filePath);
  }

  async renameFile(oldPath: string, newPath: string): Promise<boolean> {
    return this.fileService.rename(oldPath, newPath);
  }

  async updateFilePath(oldPath: string, newPath: string): Promise<void> {
    return this.fileService.updateFilePath(oldPath, newPath);
  }

  async hardDeleteFile(filePath: string): Promise<void> {
    return this.fileService.hardDelete(filePath);
  }

  // ── Indexing ──

  async addMdFile(
    filePath: string,
    content: string,
    embedder: Embedder,
    fileMtime?: number,
    chunkingVersion: string = "1",
    precomputedHash?: string,
  ): Promise<void> {
    return this.indexing.indexFile(
      filePath,
      content,
      embedder,
      fileMtime,
      chunkingVersion,
      precomputedHash,
    );
  }

  async addMdFiles(
    files: string[],
    contents: string[],
    embedder: Embedder,
    chunkingVersion: string = "1",
  ): Promise<void> {
    return this.indexing.indexFiles(files, contents, embedder, chunkingVersion);
  }

  // ── Reconciliation ──

  async reconcile(
    embedder: Embedder,
    listFiles: () => Promise<string[]>,
    readFile: (path: string) => Promise<string>,
    getMtime?: (path: string) => Promise<number | undefined>,
  ): Promise<void> {
    return this.reconciliation.reconcile(embedder, listFiles, readFile, getMtime);
  }

  async reindexAllFiles(
    embedder: Embedder,
    listFiles: () => Promise<string[]>,
    readFile: (path: string) => Promise<string>,
    force = false,
  ): Promise<void> {
    return this.reconciliation.reindexAllFiles(embedder, listFiles, readFile, force);
  }

  // ── Maintenance ──

  async optimize(): Promise<void> {
    return this.maintenance.optimize();
  }

  async vacuumIfNeeded(threshold: number = 1000): Promise<void> {
    return this.maintenance.vacuumIfNeeded(threshold);
  }

  async forceVacuum(): Promise<void> {
    return this.maintenance.forceVacuum();
  }

  // ── Retrieval ──

  async getFileChunks(
    filePath: string,
    includeDeleted = false,
  ): Promise<ChunkData[]> {
    await this.core.ensureDb();
    return this.legacy.getFileChunks(filePath, includeDeleted);
  }

  /** @deprecated Use getChunks({ by: "filePath" | "fileName", value }) instead. */
  async getChunksByFileOrName(
    filePathOrName: string,
    includeDeleted = false,
  ): Promise<ChunkDataWithPath[]> {
    await this.core.ensureDb();
    return this.legacy.getChunksByFileOrName(filePathOrName, includeDeleted);
  }

  async getAllChunks(includeDeleted = false): Promise<FullChunkData[]> {
    await this.core.ensureDb();
    return this.legacy.getAllChunks(includeDeleted);
  }

  async getChunksByFileName(
    name: string,
    includeDeleted = false,
  ): Promise<ChunkDataWithPath[]> {
    await this.core.ensureDb();
    return this.legacy.getChunksByFileName(name, includeDeleted);
  }

  /** Text search through retrieval layer (semantic/keyword/hybrid). */
  async query(
    embedder: Embedder,
    text: string,
    opts?: { topK?: number },
  ): Promise<ChunkDataWithPath[]> {
    const results = await this.queryWithScores(embedder, text, opts);
    return results.map((r) => ({
      filePath: r.filePath,
      chunkIndex: r.chunkIndex,
      chunkText: r.chunkText,
      embedding: new Float32Array(0),
    }));
  }

  /** Like query() but preserves retrieval scores — for CLI display. */
  async queryWithScores(
    embedder: Embedder,
    text: string,
    opts?: { topK?: number; filters?: { filePath?: string; folder?: string } },
  ): Promise<Array<{ filePath: string; chunkIndex: number; chunkText?: string; score: number }>> {
    await this.core.ensureDb();
    this.retrieval.setEmbedder(embedder);
    const results = await this.retrieval.query(text, opts);
    return results.map((r) => ({
      filePath: r.chunkRef.filePath,
      chunkIndex: r.chunkRef.chunkIndex,
      chunkText: r.text,
      score: r.score,
    }));
  }

  /**
   * Full pipeline query with tracing.
   * Creates a temporary RetrievalService with the given pipeline config,
   * runs the query, and returns results with variant-level breakdown.
   */
  async queryPipeline(
    embedder: Embedder,
    text: string,
    config: import("../retrieval/retrievers/types").RetrievalConfig,
    translator?: import("@translation/Translator").Translator,
    llm?: import("@ai/llm/LLMService").LLMService,
    opts?: { topK?: number; filters?: { filePath?: string; folder?: string } },
  ): Promise<{
    results: Array<{ filePath: string; chunkIndex: number; chunkText?: string; score: number }>;
    elapsed: number;
    config: typeof config;
  }> {
    await this.core.ensureDb();

    const { RetrievalService } = await import("../retrieval/RetrievalService");
    const { NoopReranker } = await import("../retrieval/rerankers/NoopReranker");

    // Build fresh service with pipeline config — doesn't pollute the cached one
    const service = new RetrievalService({
      source: this.chunks,
      config,
      reranker: config.aiRerank?.enabled ? undefined : new NoopReranker(),
      translator,
      llm,
    });
    service.setEmbedder(embedder);

    const t0 = performance.now();
    const results = await service.query(text, opts);
    const elapsed = performance.now() - t0;

    service.dispose();

    return {
      results: results.map((r) => ({
        filePath: r.chunkRef.filePath,
        chunkIndex: r.chunkRef.chunkIndex,
        chunkText: r.text,
        score: r.score,
      })),
      elapsed,
      config,
    };
  }

  /** File-scoped chunk fetch (no ranking). */
  async search(query: FetchQuery = {}): Promise<FetchResult[]> {
    await this.core.ensureDb();
    return this.legacy.search(query);
  }

  // ── Reindex scheduling ──

  setReindexHandler(handler: (files: string[]) => Promise<void>): void {
    return this.scheduler.setReindexHandler(handler);
  }

  scheduleReindex(filePath: string): void {
    return this.scheduler.scheduleReindex(filePath);
  }
}

export type { FetchQuery, FetchResult } from "../retrieval/retrievers/types";

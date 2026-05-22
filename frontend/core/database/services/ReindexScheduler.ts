// src/core/database/services/ReindexScheduler.ts
import { TauriDbConnection } from "../infrastructure/TauriDbConnection";
import { REINDEX_DEBOUNCE_MS, MAX_REINDEX_RETRIES } from "../models/constants";
import { logger } from "../infrastructure/Logger";

/**
 * Event-driven reindex queue with debounce and retry backoff.
 * Also owns the indexing serialization queue (enqueueIndexing).
 */
export class ReindexScheduler {
  private indexingQueue: Promise<void> = Promise.resolve();
  private pendingReindex = new Set<string>();
  private reindexTimer: ReturnType<typeof setTimeout> | null = null;
  private reindexHandler?: (files: string[]) => Promise<void>;
  private retryCount = new Map<string, number>();

  constructor(private core: TauriDbConnection) {}

  /** Serialize indexing operations — never throws, queues instead. */
  enqueueIndexing<T>(fn: () => Promise<T>): Promise<T> {
    const next = this.indexingQueue.then(fn, fn);
    this.indexingQueue = next.then(
      () => {},
      () => {},
    );
    return next;
  }

  setReindexHandler(handler: (files: string[]) => Promise<void>): void {
    this.reindexHandler = handler;
  }

  scheduleReindex(filePath: string): void {
    const retries = this.retryCount.get(filePath) ?? 0;
    if (retries >= MAX_REINDEX_RETRIES) {
      logger.warn(
        `Reindex skipped for "${filePath}" after ${MAX_REINDEX_RETRIES} failed attempts.`,
      );
      return;
    }
    this.pendingReindex.add(filePath);

    if (this.reindexTimer !== null) {
      clearTimeout(this.reindexTimer);
    }

    this.reindexTimer = setTimeout(async () => {
      this.reindexTimer = null;
      await this.processPendingReindex();
    }, REINDEX_DEBOUNCE_MS);
  }

  private async processPendingReindex(): Promise<void> {
    if (this.pendingReindex.size === 0) return;
    const files = Array.from(this.pendingReindex);
    this.pendingReindex.clear();

    if (this.reindexHandler) {
      try {
        await this.reindexHandler(files);
        // Success — reset retry counts for processed files
        for (const f of files) {
          this.retryCount.delete(f);
        }
      } catch (err) {
        logger.error("Error during debounced reindex:", err);
        // Increment retry count — files that hit MAX_REINDEX_RETRIES
        // will be dropped on the next scheduleReindex call.
        for (const f of files) {
          const current = this.retryCount.get(f) ?? 0;
          this.retryCount.set(f, current + 1);
        }
      }
    } else {
      logger.warn(
        `Debounced reindex for ${files.length} files skipped (no handler set).`,
      );
    }
  }

  /** Cleanup timers and retry state on shutdown. */
  cleanup(): void {
    if (this.reindexTimer !== null) {
      clearTimeout(this.reindexTimer);
      this.reindexTimer = null;
    }
    this.retryCount.clear();
  }
}

// src/core/database/EventBus.ts
import { logger } from "./infrastructure/Logger";
export type DbEventHandler<T = unknown> = (payload: T) => void | Promise<void>;

/**
 * Database-layer typed pub/sub bus. String-keyed, sequential listeners.
 * Distinct from core/ai/events/AiEventBus (generic-keyed, parallel listeners).
 */
export class DbEventBus {
  private listeners = new Map<string, Set<DbEventHandler<unknown>>>();
  private onHandlerError: (event: string, err: unknown) => void;

  constructor(opts?: { onHandlerError?: (event: string, err: unknown) => void }) {
    this.onHandlerError = opts?.onHandlerError ?? ((event, err) => {
      logger.error(`[DbEventBus] Error in handler for "${event}":`, err);
    });
  }

  on<T>(event: string, handler: DbEventHandler<T>): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler as DbEventHandler<unknown>);
    return () => this.off(event, handler);
  }

  off<T>(event: string, handler: DbEventHandler<T>): void {
    this.listeners.get(event)?.delete(handler as DbEventHandler<unknown>);
  }

  async emit<T>(event: string, payload: T): Promise<void> {
    const handlers = this.listeners.get(event);
    if (!handlers) return;
    for (const handler of handlers) {
      try {
        await handler(payload);
      } catch (err) {
        this.onHandlerError(event, err);
      }
    }
  }

  /** Remove all listeners for the given event (or all events if omitted). */
  clear(event?: string): void {
    if (event) {
      this.listeners.delete(event);
    } else {
      this.listeners.clear();
    }
  }
}

// ── Standard event contracts ──

export const Events = {
  FILE_INDEXED: "file:indexed",
  FILE_DELETED: "file:deleted",
  FILE_MODIFIED: "file:modified",
  CHUNK_INSERTED: "chunk:inserted",
  CHUNK_DELETED: "chunk:deleted",
  INDEXING_ERROR: "indexing:error",
  INDEXING_COMPLETE: "indexing:complete",
  MAINTENANCE_VACUUM: "maintenance:vacuum",
} as const;

export interface FileIndexedPayload {
  filePath: string;
  chunkCount: number;
}

export interface FileDeletedPayload {
  filePath: string;
}

export interface FileModifiedPayload {
  filePath: string;
}

export interface ChunkInsertedPayload {
  fileId: number;
  count: number;
}

export interface ChunkDeletedPayload {
  fileId: number;
  count: number;
}

export interface IndexingErrorPayload {
  filePath: string;
  error: string;
}

export interface IndexingCompletePayload {
  fileCount: number;
}

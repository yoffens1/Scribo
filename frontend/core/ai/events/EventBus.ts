import type { AiEvents } from "../types/ai-events";

type Listener<T> = (event: T) => void | Promise<void>;

/**
 * Typed event bus keyed by an event map.
 *
 * - `.on("ai:retry", (e) => ...)` — `e` is fully typed per-key
 * - `.emit("ai:retry", { ... })` — payload is checked
 * - `emit()` runs listeners via `Promise.allSettled` — one slow
 *   listener never blocks others
 *
 * @example
 * const bus = new EventBus<AiEvents>();
 * bus.on("ai:retry", (e) => console.log(e.attempt));
 * bus.emit("ai:retry", { provider: "openai", ... });
 */
export class AiEventBus<Events> {
  private listeners: {
    [K in keyof Events]?: Set<Listener<Events[K]>>;
  } = {};

  /** Subscribe to an event. Returns an unsubscribe function. */
  on<K extends keyof Events>(
    type: K,
    listener: Listener<Events[K]>,
  ): () => void {
    (this.listeners[type] ??= new Set()).add(listener);
    return () => this.off(type, listener);
  }

  /**
   * Emit an event to all subscribers.
   * Runs listeners in parallel — a slow async handler won't stall others.
   * Errors are caught per-listener and logged.
   */
  async emit<K extends keyof Events>(type: K, event: Events[K]): Promise<void> {
    const ls = this.listeners[type];
    if (!ls) return;
    await Promise.allSettled(
      [...ls].map((listener) =>
        Promise.resolve(listener(event)).catch((err) => {
          console.error(
            `[AiEventBus] error in listener for "${String(type)}":`,
            err,
          );
        }),
      ),
    );
  }

  /** Unsubscribe a specific listener. */
  off<K extends keyof Events>(type: K, listener: Listener<Events[K]>): void {
    this.listeners[type]?.delete(listener);
  }

  /** Remove all listeners for an event type (or all if no type given). */
  clear(type?: keyof Events): void {
    if (type) {
      delete this.listeners[type];
    } else {
      this.listeners = {};
    }
  }
}

/** Typed singleton event bus for the AI layer. */
export const eventBus = new AiEventBus<AiEvents>();

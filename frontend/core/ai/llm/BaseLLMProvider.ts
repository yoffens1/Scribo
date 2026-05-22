import type { LLMProviderInterface, LLMConfig } from "../types/providers";
import type { LLMCapabilities } from "../types/capabilities";
import type { LLMResponse } from "../types/responses";
import type { Message } from "../types/messages";
import { ParseError, ConfigurationError } from "../transport/HttpError";

// ─── Shared helpers ────────────────────────────────────────────

/** Narrow `value` to a non-empty string or throw a provider-scoped error. */
export const extractRequiredString = (
  value: unknown,
  provider: string,
  rawResponse: unknown,
): string => {
  if (typeof value !== "string") {
    throw new ParseError(
      `[${provider}] invalid response: ${JSON.stringify(rawResponse).slice(0, 300)}`,
      provider,
    );
  }
  return value;
};

/** Split system message from the rest. */
export const splitSystem = (
  messages: Message[],
): { system?: string; rest: Message[] } => {
  const system = messages.find((m) => m.role === "system")?.content;
  const rest = messages.filter((m) => m.role !== "system");
  return { system, rest };
};

// ─── Shared shape for `buildRequest` return ───────────────────

export type ProviderRequest = {
  path: string;
  body: unknown;
  headers?: Record<string, string>;
};

export type StreamEvent =
  | { type: "token"; text: string }
  | { type: "done"; response: LLMResponse };

// ─── Abstract base ────────────────────────────────────────────

export abstract class BaseLLMProvider implements LLMProviderInterface {
  abstract readonly backend: string;
  abstract get capabilities(): LLMCapabilities;

  constructor(protected opts: LLMConfig) {
    opts.maxTokens ??= 256;
    opts.temperature ??= 0.7;
  }

  protected get maxTokens(): number { return this.opts.maxTokens!; }
  protected get temperature(): number { return this.opts.temperature!; }

  // ── Subclass contract ──────────────────────────────────────

  /** Build a provider-specific HTTP request from a message array. */
  protected abstract buildRequest(messages: Message[]): ProviderRequest;

  /** Parse raw JSON into a canonical LLMResponse (text will be trimmed centrally). */
  protected abstract parseResponse(json: unknown): LLMResponse;

  /** Streaming providers implement this single method. */
  protected async *stream(
    _messages: Message[],
    _signal?: AbortSignal,
  ): AsyncIterable<StreamEvent> {
    throw new Error(`${this.backend}: streaming not implemented`);
  }

  /** Perform the HTTP request. Every provider must wire its HttpClient. */
  protected abstract send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown>;

  // ── Shared helpers ───────────────────────────────────────

  /** Build a Message[] from a plain prompt, injecting system prompt. */
  protected buildPromptMessages(prompt: string): Message[] {
    const messages: Message[] = [];
    if (this.opts.systemPrompt) {
      messages.push({ role: "system", content: this.opts.systemPrompt });
    }
    messages.push({ role: "user", content: prompt });
    return messages;
  }

  // ── Public API ─────────────────────────────────────────────

  async generate(prompt: string, signal?: AbortSignal): Promise<LLMResponse> {
    return this.generateMessages(this.buildPromptMessages(prompt), signal);
  }

  async generateMessages(messages: Message[], signal?: AbortSignal): Promise<LLMResponse> {
    const req = this.buildRequest(messages);
    const json = await this.send(req, signal);
    const parsed = this.parseResponse(json);
    return { ...parsed, text: parsed.text.trim() };
  }

  async *generateStream(
    prompt: string,
    signal?: AbortSignal,
    onComplete?: (response: LLMResponse) => void,
  ): AsyncIterable<string> {
    yield* this.generateStreamMessages(this.buildPromptMessages(prompt), signal, onComplete);
  }

  async *generateStreamMessages(
    messages: Message[],
    signal?: AbortSignal,
    onComplete?: (response: LLMResponse) => void,
  ): AsyncIterable<string> {
    if (!this.capabilities.streaming) {
      throw new Error(`${this.backend}: streaming not supported`);
    }

    for await (const event of this.stream(messages, signal)) {
      if (event.type === "token") yield event.text;
      else if (event.type === "done") onComplete?.(event.response);
    }
  }
}

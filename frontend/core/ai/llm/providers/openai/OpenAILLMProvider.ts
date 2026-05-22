import { BaseLLMProvider, extractRequiredString } from "../../BaseLLMProvider";
import type { StreamEvent } from "../../BaseLLMProvider";
import { normalizeUsage } from "@utils/normalizeUsage";
import type { LLMConfig } from "../../../types/providers";
import type { ProviderRequest } from "../../BaseLLMProvider";
import type { Message } from "../../../types/messages";
import type { LLMCapabilities } from "../../../types/capabilities";
import type { LLMResponse } from "../../../types/responses";
import { HttpClient } from "../../../transport/HttpClient";
import { ConfigurationError } from "../../../transport/HttpError";
import { parseSSEStream } from "../../../transport/stream/parseSSE";

interface OpenAIChatChoice {
  message?: { content?: unknown };
  delta?: { content?: unknown };
  finish_reason?: string;
}

interface OpenAIChatResponse {
  choices?: OpenAIChatChoice[];
  usage?: { prompt_tokens?: number; completion_tokens?: number; total_tokens?: number };
}

export class OpenAILLMProvider extends BaseLLMProvider {
  static readonly capabilities: LLMCapabilities = {
    streaming: true, tools: true, vision: false,
  };
  readonly backend: string;
  get capabilities() { return OpenAILLMProvider.capabilities; }
  private client: HttpClient;

  constructor(opts: LLMConfig, backend: string) {
    if (!opts.apiKey) throw new ConfigurationError(`${backend} API key required`);
    super(opts);
    this.backend = backend;
    this.client = new HttpClient(
      opts.baseUrl ?? "https://api.openai.com/v1",
      { apiKey: opts.apiKey, label: backend, maxRPS: opts.maxRPS },
    );
  }

  protected buildRequest(messages: Message[]): ProviderRequest {
    return {
      path: "/chat/completions",
      body: {
        model: this.opts.model,
        messages,
        max_tokens: this.maxTokens,
        temperature: this.temperature,
      },
    };
  }

  protected parseResponse(json: unknown): LLMResponse {
    const data = json as OpenAIChatResponse;
    const text = extractRequiredString(
      data?.choices?.[0]?.message?.content,
      this.backend,
      data,
    );
    return {
      text,
      raw: json,
      usage: normalizeUsage({
        prompt: data?.usage?.prompt_tokens,
        completion: data?.usage?.completion_tokens,
        total: data?.usage?.total_tokens,
      }),
      finishReason: data?.choices?.[0]?.finish_reason ?? undefined,
    };
  }

  protected override async send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown> {
    return this.client.post(req.path, req.body, signal);
  }

  // ── Streaming ──────────────────────────────────────────────

  protected override async *stream(
    messages: Message[],
    signal?: AbortSignal,
  ): AsyncIterable<StreamEvent> {
    const resp = await this.client.stream("/chat/completions", {
      model: this.opts.model,
      messages,
      max_tokens: this.maxTokens,
      temperature: this.temperature,
      stream: true,
    }, signal);

    for await (const raw of parseSSEStream(resp, this.backend)) {
      const chunk = raw as OpenAIChatResponse;
      const token = chunk?.choices?.[0]?.delta?.content;
      if (typeof token === "string") yield { type: "token", text: token };
      // Last chunk carries usage/finish_reason
      if (chunk?.choices?.[0]?.finish_reason || chunk?.usage) {
        yield { type: "done", response: this.parseResponse(chunk) };
        return;
      }
    }
  }
}

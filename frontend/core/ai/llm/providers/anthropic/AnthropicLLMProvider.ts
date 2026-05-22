import { BaseLLMProvider, extractRequiredString, splitSystem } from "../../BaseLLMProvider";
import { normalizeUsage } from "@utils/normalizeUsage";
import { HttpClient } from "../../../transport/HttpClient";
import { ConfigurationError } from "../../../transport/HttpError";
import type { LLMConfig } from "../../../types/providers";
import type { ProviderRequest } from "../../BaseLLMProvider";
import type { Message } from "../../../types/messages";
import type { LLMCapabilities } from "../../../types/capabilities";
import type { LLMResponse } from "../../../types/responses";

interface AnthropicResponse {
  content?: Array<{ text?: unknown }>;
  usage?: { input_tokens?: number; output_tokens?: number };
  stop_reason?: string;
}

export class AnthropicLLMProvider extends BaseLLMProvider {
  static readonly capabilities: LLMCapabilities = {
    streaming: true, tools: false, vision: false,
  };
  readonly backend = "anthropic";
  get capabilities() { return AnthropicLLMProvider.capabilities; }
  private client: HttpClient;

  constructor(opts: LLMConfig) {
    if (!opts.apiKey) throw new ConfigurationError("Anthropic API key required");
    super(opts);
    this.client = new HttpClient("https://api.anthropic.com/v1", {
      label: "Anthropic",
      headers: {
        "x-api-key": opts.apiKey,
        "anthropic-version": "2023-06-01",
      },
    });
  }

  protected buildRequest(messages: Message[]): ProviderRequest {
    const { system, rest } = splitSystem(messages);
    const body: Record<string, unknown> = {
      model: this.opts.model,
      max_tokens: this.maxTokens,
      messages: rest.map((m) => ({ role: m.role, content: m.content })),
    };
    if (system) body.system = system;
    return { path: "/messages", body };
  }

  protected override async send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown> {
    return this.client.post(req.path, req.body, signal);
  }

  protected parseResponse(json: unknown): LLMResponse {
    const data = json as AnthropicResponse;
    const text = extractRequiredString(data?.content?.[0]?.text, "Anthropic", data);
    return {
      text,
      raw: json,
      usage: normalizeUsage({
        prompt: data?.usage?.input_tokens,
        completion: data?.usage?.output_tokens,
      }),
      finishReason: data?.stop_reason ?? undefined,
    };
  }
}

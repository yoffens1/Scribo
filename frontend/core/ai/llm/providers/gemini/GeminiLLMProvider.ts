import { BaseLLMProvider, extractRequiredString, splitSystem } from "../../BaseLLMProvider";
import { normalizeUsage } from "@utils/normalizeUsage";
import { HttpClient } from "../../../transport/HttpClient";
import { ConfigurationError } from "../../../transport/HttpError";
import type { LLMConfig } from "../../../types/providers";
import type { ProviderRequest } from "../../BaseLLMProvider";
import type { Message } from "../../../types/messages";
import type { LLMCapabilities } from "../../../types/capabilities";
import type { LLMResponse } from "../../../types/responses";

interface GeminiResponse {
  candidates?: Array<{ content?: { parts?: Array<{ text?: unknown }> }; finishReason?: string }>;
  usageMetadata?: { promptTokenCount?: number; candidatesTokenCount?: number; totalTokenCount?: number };
}

const geminiRole = (role: string): string => {
  switch (role) {
    case "user": return "user";
    case "assistant": return "model";
    default: throw new Error(`[Gemini] unsupported message role: "${role}"`);
  }
};

export class GeminiLLMProvider extends BaseLLMProvider {
  static readonly capabilities: LLMCapabilities = {
    streaming: false, tools: false, vision: false,
  };
  readonly backend = "gemini";
  get capabilities() { return GeminiLLMProvider.capabilities; }
  private client: HttpClient;

  constructor(opts: LLMConfig) {
    if (!opts.apiKey) throw new ConfigurationError("Gemini API key required");
    super(opts);
    this.client = new HttpClient(
      "https://generativelanguage.googleapis.com/v1beta",
      { label: "Gemini", headers: { "x-goog-api-key": opts.apiKey } },
    );
  }

  protected buildRequest(messages: Message[]): ProviderRequest {
    const { system, rest } = splitSystem(messages);
    const turns = rest.map((m) => ({
      role: geminiRole(m.role),
      parts: [{ text: m.content }],
    }));

    const body: Record<string, unknown> = {
      contents: turns,
      generationConfig: {
        maxOutputTokens: this.maxTokens,
        temperature: this.temperature,
      },
    };
    if (system) body.systemInstruction = { parts: [{ text: system }] };

    return { path: `/models/${this.opts.model}:generateContent`, body };
  }

  protected override async send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown> {
    return this.client.post(req.path, req.body, signal);
  }

  protected parseResponse(json: unknown): LLMResponse {
    const data = json as GeminiResponse;
    const text = extractRequiredString(
      data?.candidates?.[0]?.content?.parts?.[0]?.text,
      "Gemini",
      data,
    );
    const meta = data?.usageMetadata;
    return {
      text,
      raw: json,
      usage: normalizeUsage({
        prompt: meta?.promptTokenCount,
        completion: meta?.candidatesTokenCount,
        total: meta?.totalTokenCount,
      }),
      finishReason: data?.candidates?.[0]?.finishReason ?? undefined,
    };
  }
}

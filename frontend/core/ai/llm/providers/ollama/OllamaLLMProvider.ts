import { BaseLLMProvider, extractRequiredString } from "../../BaseLLMProvider";
import type { StreamEvent } from "../../BaseLLMProvider";
import { normalizeUsage } from "@utils/normalizeUsage";
import { HttpClient } from "../../../transport/HttpClient";
import type { LLMConfig } from "../../../types/providers";
import type { ProviderRequest } from "../../BaseLLMProvider";
import type { Message } from "../../../types/messages";
import type { LLMCapabilities } from "../../../types/capabilities";
import type { LLMResponse } from "../../../types/responses";
import { parseNDJSONStream } from "../../../transport/stream/parseNDJSON";

interface OllamaResponse {
  message?: { content?: unknown };
  prompt_eval_count?: number;
  eval_count?: number;
  done_reason?: string;
}

export class OllamaLLMProvider extends BaseLLMProvider {
  static readonly capabilities: LLMCapabilities = {
    streaming: true, tools: false, vision: false,
  };
  readonly backend = "ollama";
  get capabilities() { return OllamaLLMProvider.capabilities; }
  private client: HttpClient;

  constructor(opts: LLMConfig) {
    super(opts);
    this.client = new HttpClient(opts.baseUrl ?? "http://localhost:11434", { label: "Ollama" });
  }

  protected buildRequest(messages: Message[]): ProviderRequest {
    const body: Record<string, unknown> = { model: this.opts.model, messages, stream: false };
    if (this.opts.responseFormat === "json") {
      body.format = "json";
    }
    return {
      path: "/api/chat",
      body,
    };
  }

  protected parseResponse(json: unknown): LLMResponse {
    const data = json as OllamaResponse;
    const text = extractRequiredString(data?.message?.content, "Ollama", data);
    return {
      text,
      raw: json,
      usage: normalizeUsage({
        prompt: data?.prompt_eval_count,
        completion: data?.eval_count,
      }),
      finishReason: data?.done_reason ?? undefined,
    };
  }

  // ── Streaming ──────────────────────────────────────────────

  protected override async send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown> {
    return this.client.post(req.path, req.body, signal);
  }

  protected override async *stream(
    messages: Message[],
    signal?: AbortSignal,
  ): AsyncIterable<StreamEvent> {
    const body: Record<string, unknown> = {
      model: this.opts.model,
      messages,
      stream: true,
    };
    if (this.opts.responseFormat === "json") {
      body.format = "json";
    }
    const resp = await this.client.stream("/api/chat", body, signal);

    for await (const raw of parseNDJSONStream(resp, "Ollama")) {
      const chunk = raw as OllamaResponse;
      if (chunk.message?.content) {
        yield { type: "token", text: chunk.message.content as string };
      }
      if (chunk.done_reason || chunk.eval_count != null) {
        yield { type: "done", response: this.parseResponse(chunk) };
        return;
      }
    }
  }
}

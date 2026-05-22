import { BaseLLMProvider } from "../../BaseLLMProvider";
import type { ProviderRequest } from "../../BaseLLMProvider";
import { HttpClient } from "../../../transport/HttpClient";
import type { LLMConfig, CustomProviderConfig } from "../../../types/providers";
import type { LLMResponse } from "../../../types/responses";
import type { Message } from "../../../types/messages";
import type { LLMCapabilities } from "../../../types/capabilities";

export class CustomProvider extends BaseLLMProvider {
  override readonly backend: string;
  static readonly capabilities: LLMCapabilities = {
    streaming: false,
    tools: false,
    vision: false,
  };
  override get capabilities() { return CustomProvider.capabilities; }
  private client: HttpClient;

  constructor(
    private cfg: CustomProviderConfig,
    config: LLMConfig,
  ) {
    super(config);
    this.backend = cfg.name;
    this.client = new HttpClient(cfg.endpoint, { label: cfg.name });
  }

  protected buildRequest(messages: Message[]): ProviderRequest {
    const prompt = messages.map((m) => `[${m.role}] ${m.content}`).join("\n");
    return {
      path: "",
      body: this.cfg.buildRequestBody(prompt, this.opts),
    };
  }

  protected override async send(req: ProviderRequest, signal?: AbortSignal): Promise<unknown> {
    return this.client.post(req.path, req.body, signal);
  }

  protected parseResponse(json: unknown): LLMResponse {
    return { text: this.cfg.extractResponse(json), raw: json };
  }
}

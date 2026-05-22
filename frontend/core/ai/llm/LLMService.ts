import type {
  LLMProviderInterface,
  LLMBackend,
  LLMConfig,
  CustomProviderConfig,
} from "../types/providers";
import type { LLMResponse } from "../types/responses";
import type { Message } from "../types/messages";
import type { LLMCapabilities } from "../types/capabilities";
import { CustomProvider } from "./providers/custom/CustomProvider";
import { ProviderRegistry } from "../registry/ProviderRegistry";
import { LLM_DEFAULTS } from "@settings";

export type { LLMBackend, LLMConfig, CustomProviderConfig, LLMResponse };

/**
 * Universal LLM provider facade. Delegates to backend-specific provider classes.
 */
export class LLMService {
  private provider: LLMProviderInterface;

  constructor(config: LLMConfig) {
    // Per-instance registry — no global mutation
    const reg = new ProviderRegistry();
    for (const c of config.customProviders ?? []) {
      reg.register(c.name, (cfg) => new CustomProvider(c, cfg));
    }

    // Apply shared defaults before handing to the factory
    const cfg: LLMConfig = {
      ...config,
      baseUrl: config.baseUrl ?? LLM_DEFAULTS.baseUrls[config.backend] ?? "",
      maxTokens: config.maxTokens ?? LLM_DEFAULTS.maxTokens,
      temperature: config.temperature ?? LLM_DEFAULTS.temperature,
    };

    this.provider = reg.create(cfg);
  }

  async generate(prompt: string, signal?: AbortSignal): Promise<LLMResponse> {
    return this.provider.generate(prompt, signal);
  }

  async generateMessages(
    messages: Message[],
    signal?: AbortSignal,
  ): Promise<LLMResponse> {
    return this.provider.generateMessages!(messages, signal);
  }

  generateStream(
    prompt: string,
    signal?: AbortSignal,
    onComplete?: (r: LLMResponse) => void,
  ): AsyncIterable<string> {
    if (!this.provider.capabilities.streaming) {
      throw new Error(`${this.provider.backend}: streaming not supported`);
    }
    return this.provider.generateStream!(prompt, signal, onComplete);
  }

  generateStreamMessages(
    messages: Message[],
    signal?: AbortSignal,
    onComplete?: (r: LLMResponse) => void,
  ): AsyncIterable<string> {
    if (!this.provider.capabilities.streaming) {
      throw new Error(`${this.provider.backend}: streaming not supported`);
    }
    if (this.provider.generateStreamMessages) {
      return this.provider.generateStreamMessages(messages, signal, onComplete);
    }
    throw new Error(`${this.provider.backend}: message streaming not supported`);
  }

  get capabilities(): LLMCapabilities {
    return this.provider.capabilities;
  }

  get backend(): string {
    return this.provider.backend;
  }
}

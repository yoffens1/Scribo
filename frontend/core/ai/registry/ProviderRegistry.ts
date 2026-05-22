import type { BaseLLMProvider } from "../llm/BaseLLMProvider";
import type { LLMConfig } from "../types/providers";
import { OllamaLLMProvider } from "@ai/llm/providers/ollama/OllamaLLMProvider";
import { OpenAILLMProvider } from "@ai/llm/providers/openai/OpenAILLMProvider";
import { AnthropicLLMProvider } from "@ai/llm/providers/anthropic/AnthropicLLMProvider";
import { GeminiLLMProvider } from "@ai/llm/providers/gemini/GeminiLLMProvider";

type ProviderFactory = (config: LLMConfig) => BaseLLMProvider;

/**
 * Typed registry of LLM provider factories.
 * Call `.register()` to add backends, `.create()` to instantiate.
 */
export class ProviderRegistry {
  private factories = new Map<string, ProviderFactory>();

  constructor(registerBuiltins = true) {
    if (!registerBuiltins) return;
    // ── Built-in providers ──────────────────────────────────
    this.register("ollama", (c) => new OllamaLLMProvider(c));
    this.register("openai", (c) => new OpenAILLMProvider(c, "openai"));
    this.register("deepseek", (c) => new OpenAILLMProvider(c, "deepseek"));
    this.register("openrouter", (c) => new OpenAILLMProvider(c, "openrouter"));
    this.register("anthropic", (c) => new AnthropicLLMProvider(c));
    this.register("gemini", (c) => new GeminiLLMProvider(c));
  }

  register(backend: string, factory: ProviderFactory): this {
    this.factories.set(backend, factory);
    return this;
  }

  create(config: LLMConfig): BaseLLMProvider {
    const factory = this.factories.get(config.backend);
    if (!factory) throw new Error(`Unsupported backend: ${config.backend}`);
    return factory(config);
  }

  has(backend: string): boolean {
    return this.factories.has(backend);
  }
}

/** Default singleton — prefer creating a local instance per LLMService. */
export const defaultRegistry = new ProviderRegistry();

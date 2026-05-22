import type { LLMCapabilities } from "./capabilities";
import type { LLMResponse } from "./responses";
import type { Message } from "./messages";

export type LLMBackend =
  | "ollama"
  | "openai"
  | "anthropic"
  | "gemini"
  | "deepseek"
  | "openrouter"
  | string;

export interface LLMConfig {
  model: string;
  apiKey?: string;
  baseUrl?: string;
  systemPrompt?: string;
  maxTokens?: number;
  temperature?: number;
  maxRPS?: number;
  backend: LLMBackend;
  headers?: Record<string, string>;
  customProviders?: CustomProviderConfig[];
  responseFormat?: "json" | "text";
}

export interface CustomProviderConfig {
  name: string;
  endpoint: string;
  method?: "POST";
  buildRequestBody: (prompt: string, config: LLMConfig) => string;
  extractResponse: (json: unknown) => string;
}

export interface LLMProviderInterface {
  readonly backend: string;
  readonly capabilities: LLMCapabilities;
  generate(prompt: string, signal?: AbortSignal): Promise<LLMResponse>;
  generateMessages(messages: Message[], signal?: AbortSignal): Promise<LLMResponse>;
  generateStream?(prompt: string, signal?: AbortSignal, onComplete?: (r: LLMResponse) => void): AsyncIterable<string>;
  generateStreamMessages?(messages: Message[], signal?: AbortSignal, onComplete?: (r: LLMResponse) => void): AsyncIterable<string>;
}

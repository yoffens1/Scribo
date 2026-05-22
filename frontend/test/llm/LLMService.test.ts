import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { LLMService } from "@ai/llm/LLMService";
import type { LLMConfig } from "@ai/types/providers";

function makeConfig(overrides: Partial<LLMConfig> = {}): LLMConfig {
  return { backend: "ollama", model: "llama3", maxTokens: 100, temperature: 0.7, ...overrides };
}

describe("LLMService", () => {
  it("constructs for all built-in backends", async () => {
    assert.doesNotThrow(() => new LLMService(makeConfig({ backend: "ollama" })));
    assert.doesNotThrow(() => new LLMService(makeConfig({ backend: "openai", apiKey: "sk-test" })));
    assert.doesNotThrow(() => new LLMService(makeConfig({ backend: "anthropic", apiKey: "sk-test" })));
    assert.doesNotThrow(() => new LLMService(makeConfig({ backend: "gemini", apiKey: "sk-test" })));
  });

  it("exposes backend", async () => {
    const svc = new LLMService(makeConfig({ backend: "ollama" }));
    assert.equal(svc.backend, "ollama");
  });

  it("exposes capabilities", async () => {
    const svc = new LLMService(makeConfig({ backend: "ollama" }));
    assert.equal(svc.capabilities.streaming, true);
    assert.equal(typeof svc.capabilities.tools, "boolean");
  });

  it("applies defaults", async () => {
    const svc = new LLMService(makeConfig({ backend: "ollama", maxTokens: undefined, temperature: undefined }));
    assert.equal(svc.backend, "ollama");
  });

  it("generate() delegates", async () => {
    const svc = new LLMService(makeConfig({ backend: "ollama" }));
    await assert.rejects(() => svc.generate("hi"));
  });

  it("generateMessages() delegates", async () => {
    const svc = new LLMService(makeConfig({ backend: "ollama" }));
    await assert.rejects(() => svc.generateMessages([{ role: "user", content: "hi" }]));
  });

  it("generateStream throws for non-streaming", async () => {
    const svc = new LLMService(makeConfig({ backend: "gemini", apiKey: "sk-test" }));
    assert.throws(() => svc.generateStream("hi"), /streaming not supported/);
  });

  it("custom provider", async () => {
    const svc = new LLMService(makeConfig({
      backend: "my-custom",
      customProviders: [{
        name: "my-custom", endpoint: "http://localhost/v1",
        buildRequestBody: (p: string) => JSON.stringify({ prompt: p }),
        extractResponse: (j: unknown) => (j as any).text ?? "",
      }],
    }));
    assert.equal(svc.backend, "my-custom");
  });

  it("throws on unsupported backend", async () => {
    assert.throws(() => new LLMService(makeConfig({ backend: "nonexistent" })), /Unsupported backend/);
  });
});

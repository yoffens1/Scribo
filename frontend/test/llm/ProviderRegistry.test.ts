import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ProviderRegistry } from "@ai/registry/ProviderRegistry";
import { BaseLLMProvider } from "@ai/llm/BaseLLMProvider";
import type { LLMConfig } from "@ai/types/providers";
import type { ProviderRequest } from "@ai/llm/BaseLLMProvider";
import type { Message } from "@ai/types/messages";
import type { LLMCapabilities } from "@ai/types/capabilities";
import type { LLMResponse } from "@ai/types/responses";

function makeConfig(overrides: Partial<LLMConfig> = {}): LLMConfig {
  return { backend: "test", model: "test-model", maxTokens: 100, temperature: 0.7, ...overrides };
}

let counter = 0;
class FakeProvider extends BaseLLMProvider {
  static readonly capabilities: LLMCapabilities = { streaming: false, tools: false, vision: false };
  readonly backend: string;
  readonly id: number;
  get capabilities() { return FakeProvider.capabilities; }
  constructor(opts: LLMConfig) { super(opts); counter++; this.id = counter; this.backend = opts.backend ?? "test"; }
  protected buildRequest(_m: Message[]): ProviderRequest { return { path: "/test", body: {} }; }
  protected async send(_req: ProviderRequest): Promise<unknown> { return { text: "ok", id: this.id }; }
  protected parseResponse(json: unknown): LLMResponse { return { text: (json as any).text }; }
}

describe("ProviderRegistry", () => {
  it("creates provider for registered backend", async () => {
    const reg = new ProviderRegistry(false);
    reg.register("test", (c) => new FakeProvider(c));
    const p = reg.create(makeConfig({ backend: "test" }));
    assert.ok(p instanceof FakeProvider);
  });

  it("config reaches factory", async () => {
    const reg = new ProviderRegistry(false);
    let captured: LLMConfig | undefined;
    reg.register("x", (c) => { captured = c; return new FakeProvider(c); });
    reg.create(makeConfig({ backend: "x", model: "m1", maxTokens: 42 }));
    assert.equal(captured?.model, "m1");
    assert.equal(captured?.maxTokens, 42);
  });

  it("throws on unknown backend", async () => {
    const reg = new ProviderRegistry(false);
    assert.throws(() => reg.create(makeConfig({ backend: "nonexistent" })), /Unsupported backend/);
  });

  it("has() checks registration", async () => {
    const reg = new ProviderRegistry(false);
    assert.equal(reg.has("ollama"), false);
    reg.register("ollama", (c) => new FakeProvider(c));
    assert.equal(reg.has("ollama"), true);
  });

  it("built-ins registered by default", async () => {
    const reg = new ProviderRegistry();
    for (const b of ["ollama", "openai", "anthropic", "gemini", "deepseek", "openrouter"])
      assert.equal(reg.has(b), true, b);
  });

  it("per-instance isolation", async () => {
    const r1 = new ProviderRegistry(false);
    const r2 = new ProviderRegistry(false);
    r1.register("a", (c) => new FakeProvider(c));
    r2.register("b", (c) => new FakeProvider(c));
    assert.equal(r1.has("a"), true);
    assert.equal(r1.has("b"), false);
    assert.equal(r2.has("a"), false);
    assert.equal(r2.has("b"), true);
  });

  it("register returns this", async () => {
    const reg = new ProviderRegistry(false);
    assert.equal(reg.register("a", (c) => new FakeProvider(c)), reg);
  });
});

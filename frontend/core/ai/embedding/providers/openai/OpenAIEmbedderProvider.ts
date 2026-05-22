import { BaseEmbedder } from "../../BaseEmbedder";
import { HttpClient } from "../../../transport/HttpClient";
import { ValidationError, ConfigurationError } from "../../../transport/HttpError";
import type { EmbedderCapabilities } from "../../../types/capabilities";

// ─── Response type guard ──────────────────────────────────────

interface EmbeddingDatum {
  embedding: number[];
}

const isEmbeddingResponse = (
  json: unknown,
): json is { data: EmbeddingDatum[] } => {
  if (typeof json !== "object" || json === null) return false;
  const d = (json as Record<string, unknown>).data;
  return (
    Array.isArray(d) &&
    d.every(
      (item) =>
        typeof item === "object" &&
        item !== null &&
        Array.isArray((item as Record<string, unknown>).embedding),
    )
  );
};

// ─── Provider ─────────────────────────────────────────────────

export class OpenAIEmbedderProvider extends BaseEmbedder {
  static readonly capabilities: EmbedderCapabilities = { batchEmbedding: true };
  get capabilities() { return OpenAIEmbedderProvider.capabilities; }
  private client: HttpClient;

  constructor(model: string, apiKey: string) {
    if (!apiKey) throw new ConfigurationError("OpenAIEmbedder API key required");
    super(model);
    this.client = new HttpClient("https://api.openai.com/v1", { apiKey, label: "OpenAIEmbedder" });
  }

  async embed(text: string): Promise<Float32Array> {
    const json = await this.client.post("/embeddings", { model: this.model, input: text });
    if (!isEmbeddingResponse(json)) {
      throw new ValidationError("[OpenAIEmbedder] invalid response shape", "OpenAIEmbedder");
    }
    const vec = new Float32Array(json.data[0].embedding);
    this.validateDim(vec, "OpenAIEmbedder");
    return vec;
  }

  async embedBatch(texts: string[]): Promise<Float32Array[]> {
    if (texts.length === 0) return [];
    const json = await this.client.post("/embeddings", { model: this.model, input: texts });
    if (!isEmbeddingResponse(json)) {
      throw new ValidationError("[OpenAIEmbedder] invalid batch response shape", "OpenAIEmbedder");
    }
    const vecs = json.data.map((d) => new Float32Array(d.embedding));
    for (const v of vecs) this.validateDim(v, "OpenAIEmbedder");
    return vecs;
  }
}

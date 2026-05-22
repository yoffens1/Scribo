import { BaseEmbedder } from "../../BaseEmbedder";
import { HttpClient } from "../../../transport/HttpClient";
import { ValidationError } from "../../../transport/HttpError";
import type { EmbedderCapabilities } from "../../../types/capabilities";

export class OllamaEmbedderProvider extends BaseEmbedder {
  static readonly capabilities: EmbedderCapabilities = { batchEmbedding: false };
  get capabilities() { return OllamaEmbedderProvider.capabilities; }
  private client: HttpClient;

  constructor(model: string, baseUrl = "http://localhost:11434") {
    super(model);
    this.client = new HttpClient(baseUrl, { label: "OllamaEmbedder" });
  }

  async embed(text: string): Promise<Float32Array> {
    const json = await this.client.post("/api/embeddings", { model: this.model, prompt: text }) as Record<string, unknown>;

    if (!json?.embedding || !Array.isArray(json.embedding)) {
      throw new ValidationError(`[OllamaEmbedder] missing embedding (model=${this.model})`, "OllamaEmbedder");
    }
    const vec = new Float32Array(json.embedding);
    this.validateDim(vec, "OllamaEmbedder");
    return vec;
  }
}

// src/settings.ts
import { LLMConfig } from "@ai/types";
import { sanitizeFileName } from "@utils/sanitizeFileName";

/** Конфигурация переводчика */
export const TRANSLATOR_CONFIG = {
  provider: {
    backend: "ollama",
    model: "MedAIBase/Tencent-HY-MT1.5:1.8b",
    baseUrl: "http://localhost:11434",
    temperature: 0.6,
  } as LLMConfig,
  targetLang: "rus",
} as const;

/** Безопасное имя модели для использования в именах файлов */
export const SAFE_MODEL_NAME = sanitizeFileName("qwen3-embedding:latest");

/** Относительный путь к папке плагина внутри хранилища Obsidian */
export const PLUGIN_DIR = ".obsidian/plugins/llm-assist-test";

/**
 * Конфигурация путей к хранилищу и базе данных.
 */
export const PATH_TO_VAULT = {
  /**
   * Абсолютный путь к корню хранилища Obsidian, где лежат .md файлы для индексации.
   */
  inputVault: "/home/yoffens/obsidian2026/1-INBOX/",

  /**
   * Абсолютный путь к папке, в которой будет создана база данных векторов.
   * Обычно это папка плагина внутри .obsidian или выделенное тестовое место.
   */
  outputPathForDb:
    "/home/yoffens/obsidian2026/.obsidian/plugins/LLM-Assist/src/test/test-db/",
};

/**
 * Shared AI transport defaults.
 */
export const AI_DEFAULTS = {
  timeout: 30_000,
  maxRetries: 3,
};

/**
 * Embedder configuration.
 */
export const EMBEDDER_CONFIG = {
  knownDims: {
    "qwen3-embedding:latest": 4096,
    "nomic-embed-text": 768,
    "mxbai-embed-large": 1024,
    "all-minilm": 384,
    "text-embedding-3-small": 1536,
    "text-embedding-3-large": 3072,
    "text-embedding-ada-002": 1536,
  } as Record<string, number>,
  maxConcurrent: 5,
} as const;

/**
 * LLM provider defaults.
 */
export const LLM_DEFAULTS = {
  maxTokens: 100,
  temperature: 0.7,
  baseUrls: {
    ollama: "http://localhost:11434",
    openai: "https://api.openai.com/v1",
    deepseek: "https://api.deepseek.com/v1",
    openrouter: "https://openrouter.ai/api/v1",
  } as Record<string, string>,
} as const;

// ── Retrieval config builder ──

import type { RetrievalConfig } from "@retrieval/retrievers/types";

/**
 * Build RetrievalConfig from user-facing settings.
 *
 * In the Obsidian plugin, this would be called from the settings tab
 * onChange handler. For the CLI, call it directly with your desired opts.
 *
 * UI mapping (not yet implemented — platform-dependent):
 *   - Checkbox: "AI Rerank" → aiRerank.enabled
 *   - Dropdown: "Rerank mode" → aiRerank.mode ("scoring" | "listwise")
 *   - Slider:  "Max candidates" → aiRerank.maxCandidates (10-50, default 25)
 */
export function buildRetrievalConfig(opts: {
  mode?: RetrievalConfig["mode"];
  embeddingWeight?: number;
  pipeline?: RetrievalConfig["pipeline"];
  aiRerank?: RetrievalConfig["aiRerank"];
}): RetrievalConfig {
  return {
    mode: opts.mode ?? "hybrid",
    embeddingWeight: opts.embeddingWeight,
    pipeline: opts.pipeline,
    aiRerank: opts.aiRerank,
  };
}

// src/core/ai/transport/HttpError.ts

export class HttpError extends Error {
  constructor(
    message: string,
    public status: number,
    public provider: string,
    public body?: string,
  ) {
    super(message);
    this.name = "HttpError";
  }
}

export class AIError extends Error {
  constructor(
    message: string,
    public provider: string,
    public cause?: unknown,
  ) {
    super(message);
    this.name = "AIError";
  }
}

/** Response shape mismatch — not a transient error. */
export class ValidationError extends AIError {
  constructor(message: string, provider: string, cause?: unknown) {
    super(message, provider, cause);
    this.name = "ValidationError";
  }
}

/** Failed to parse server response — not a transient error. */
export class ParseError extends AIError {
  constructor(message: string, provider: string, cause?: unknown) {
    super(message, provider, cause);
    this.name = "ParseError";
  }
}

/** Invalid configuration — missing API key, bad model, etc. */
export class ConfigurationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ConfigurationError";
  }
}

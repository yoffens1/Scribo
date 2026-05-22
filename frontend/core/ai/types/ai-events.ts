export interface AiEvents {
  "ai:request:start": {
    provider: string;
    path: string;
    timestamp: number;
  };

  "ai:request:done": {
    provider: string;
    path: string;
    latencyMs: number;
    status: number;
    timestamp: number;
  };

  "ai:request:error": {
    provider: string;
    path: string;
    status: number;
    error: string;
    latencyMs: number;
    timestamp: number;
  };

  "ai:retry": {
    provider: string;
    label: string;
    attempt: number;
    maxRetries: number;
    delayMs: number;
    error: string;
    timestamp: number;
  };
}

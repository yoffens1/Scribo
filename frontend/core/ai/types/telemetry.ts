export interface RequestMetrics {
  provider: string;
  path: string;
  latencyMs: number;
  status: number;
  tokensUsed?: number;
  error?: string;
}

export type TelemetryHook = (metrics: RequestMetrics) => void | Promise<void>;

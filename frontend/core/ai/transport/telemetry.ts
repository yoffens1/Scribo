// src/core/ai/transport/telemetry.ts

import type { RequestMetrics, TelemetryHook } from "../types/telemetry";

let globalHook: TelemetryHook | null = null;

export function setTelemetryHook(hook: TelemetryHook | null): void {
  globalHook = hook;
}

export function report(metrics: RequestMetrics): void {
  void globalHook?.(metrics);
}

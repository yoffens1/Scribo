import type { LLMResponse } from "../core/ai/types/responses";

/**
 * Normalise provider-specific token counts into the canonical `LLMResponse.usage`.
 *
 * @example
 * normalizeUsage({ prompt: 10, completion: 5 })       // { promptTokens: 10, completionTokens: 5, totalTokens: 15 }
 * normalizeUsage({ prompt: 10, completion: 5, total: 20 })  // { promptTokens: 10, completionTokens: 5, totalTokens: 20 }
 * normalizeUsage({})                                   // undefined
 */
export const normalizeUsage = (
  u: { prompt?: number; completion?: number; total?: number },
): LLMResponse["usage"] => {
  if (u.prompt == null && u.completion == null) return undefined;
  const p = u.prompt ?? 0;
  const c = u.completion ?? 0;
  return {
    promptTokens: p,
    completionTokens: c,
    totalTokens: u.total ?? p + c,
  };
};

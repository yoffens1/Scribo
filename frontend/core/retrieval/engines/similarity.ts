// src/core/retrieval/engines/vector.ts

/** L2-normalize a vector in-place. Returns the same array. */
export function l2Normalize(v: Float32Array): Float32Array {
  let norm = 0;
  for (let i = 0; i < v.length; i++) norm += v[i] ** 2;
  const invNorm = 1 / Math.sqrt(norm);
  for (let i = 0; i < v.length; i++) v[i] *= invNorm;
  return v;
}

/**
 * Cosine similarity.
 * Stored vectors are L2-normalized at serialization time (||a|| = 1),
 * so we skip computing normA. For legacy un-normalized vectors the result
 * is approximate (missing 1/||a|| factor), acceptable for ranking.
 */
export function cosineSimilarity(a: Float32Array, b: Float32Array): number {
  let dot = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normB += b[i] ** 2;
  }
  // Only query vector needs normalization; stored vector is already L2=1
  return dot / Math.sqrt(normB);
}

export function distance(a: Float32Array, b: Float32Array): number {
  return 1 - cosineSimilarity(a, b);
}

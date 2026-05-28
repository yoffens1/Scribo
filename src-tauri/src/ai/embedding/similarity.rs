//! # Cosine Similarity
//!
//! Zero-overhead similarity functions used by both the retrieval pipeline and the vector search
//! in the database layer.
//!
//! ## Implementation
//!
//! A single const-generic `cosine_similarity_impl<const NORMALIZED: bool>` is monomorphised
//! into two specialisations at compile time:
//!
//! - **`NORMALIZED = true`** (`cosine_similarity_normalized`): assumes both input vectors are
//!   already L2-normalised (unit length). The cosine similarity reduces to a plain dot product —
//!   no square roots or divisions needed. ~2× faster than the general case.
//!
//! - **`NORMALIZED = false`** (`cosine_similarity`): computes the full dot product and both
//!   L2 norms in a single fused pass, then divides. Returns 0.0 if either vector is the zero vector.
//!
//! All embeddings produced by [`Embedder`](crate::ai::Embedder) are L2-normalised before storage,
//! so [`cosine_similarity_normalized`] should be preferred in the hot path.

/// General cosine similarity — works for any pair of vectors regardless of normalisation.
/// Returns a value in `[-1.0, 1.0]` (or `0.0` if either vector has zero magnitude).
#[inline(always)]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity_impl::<false>(a, b)
}

/// Fast cosine similarity for **unit-norm** (L2-normalised) vectors.
/// Reduces to a dot product — avoids sqrt and division.
/// **Precondition**: both `a` and `b` must have L2-norm ≈ 1.0.
#[inline(always)]
pub fn cosine_similarity_normalized(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity_impl::<true>(a, b)
}

/// Monomorphic core. `NORMALIZED = true` compiles to a pure dot product.
#[inline(always)]
fn cosine_similarity_impl<const NORMALIZED: bool>(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    if NORMALIZED {
        // Unit vectors: cos(θ) = a · b
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    } else {
        // General case: fused accumulation of dot, |a|², |b|²
        let (dot, norm_a, norm_b) = a.iter().zip(b.iter()).fold(
            (0.0f32, 0.0f32, 0.0f32),
            |(d, na, nb), (&x, &y)| (d + x * y, na + x * x, nb + y * y),
        );
        let denom = (norm_a * norm_b).sqrt();
        if denom == 0.0 { 0.0 } else { dot / denom }
    }
}

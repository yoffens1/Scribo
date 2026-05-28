pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot_product += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }
    
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b).sqrt()
    }
}

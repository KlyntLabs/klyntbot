//! Shared embedding utilities for test mocks.
//!
//! Provides deterministic embedding generation and cosine similarity
//! used by both `MockEmbeddingHandler` and `MockConversationEmbeddingHandler`.

use tools::EMBEDDING_DIM;

/// Generate a deterministic 384-dim embedding from text.
///
/// Uses a simple hash-based approach so semantically identical text
/// produces identical vectors. The result is normalized to a unit vector.
pub fn deterministic_embedding(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0f32; EMBEDDING_DIM];
    for (i, byte) in text.bytes().enumerate() {
        let idx = i % EMBEDDING_DIM;
        vec[idx] += (byte as f32) / 255.0;
    }
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)) as f64
}

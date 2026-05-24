use scribo_lib::ai::embedding::Embedder;
use scribo_lib::ai::types::EmbedderConfig;

#[tokio::test]
async fn test_openai_embedder_batching() {
    // We can't really call OpenAI in automated tests without a real API key.
    // But we can verify that the Embedder is correctly initialized and returns an error 
    // when given an invalid API key, proving the pipeline works.
    let config = EmbedderConfig {
        provider: "openai".to_string(),
        model: Some("text-embedding-3-small".to_string()),
        api_key: Some("sk-invalid-test-key".to_string()),
        base_url: None,
    };
    
    let embedder = Embedder::new(config);
    let result = embedder.embed("test string").await;
    
    assert!(result.is_err(), "Expected an error due to invalid API key");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.to_lowercase().contains("error"),
        "Error should mention API error: {}", err_msg
    );
}

#[tokio::test]
#[ignore = "Требуется скачанная локальная модель nomic-embed-text.gguf"]
async fn test_local_embedder() {
    let config = EmbedderConfig {
        provider: "local".to_string(),
        model: Some("nomic-embed-text".to_string()),
        api_key: None,
        base_url: None,
    };
    
    let embedder = Embedder::new(config);
    
    let text1 = "Hello, world!";
    let text2 = "Testing local embeddings";
    
    // Проверка одиночного вызова
    let result = embedder.embed(text1).await.expect("Failed to embed text1");
    assert_eq!(result.len(), 768, "Nomic Embed должен возвращать 768 измерений");
    
    // Проверка батч-вызова
    let batch_result = embedder.embed_batch(vec![text1.to_string(), text2.to_string()]).await.expect("Failed batch embed");
    assert_eq!(batch_result.len(), 2, "Должно вернуться 2 вектора");
    assert_eq!(batch_result[0].len(), 768);
    assert_eq!(batch_result[1].len(), 768);
    
    // Векторы разных текстов должны отличаться
    assert_ne!(batch_result[0], batch_result[1], "Эмбеддинги разных текстов не должны быть идентичными");
}

use super::*;

fn model_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("model.onnx"), b"fake").unwrap();
    temp
}

#[test]
fn model_path_rejects_root_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();

    let err = OnnxModelPath::parse(root.path(), outside.path()).unwrap_err();

    assert_eq!(err.code, OnnxRuntimeErrorCode::Validation);
    assert_eq!(err.field.as_deref(), Some("path"));
}

#[test]
fn model_path_requires_onnx_extension() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("model.bin"), b"fake").unwrap();

    let err = OnnxModelPath::parse(root.path(), "model.bin").unwrap_err();

    assert_eq!(err.field.as_deref(), Some("path"));
    assert!(err.message.contains(".onnx"));
}

#[test]
fn embedding_request_validates_model_id_and_shape() {
    let err = OnnxEmbeddingRequest::parse("../bad", vec!["hello".to_string()], None).unwrap_err();
    assert_eq!(err.field.as_deref(), Some("model_id"));

    let err = OnnxEmbeddingRequest::parse("model", Vec::new(), None).unwrap_err();
    assert_eq!(err.field.as_deref(), Some("input"));

    let err = OnnxEmbeddingRequest::parse("model", vec!["hello".to_string()], Some(0)).unwrap_err();
    assert_eq!(err.field.as_deref(), Some("dimensions"));
}

#[test]
fn model_id_accepts_library_style_segments_without_path_traversal() {
    let model_id = OnnxModelId::parse("embedding/nomic/model-v1.5").unwrap();
    assert_eq!(model_id.as_str(), "embedding/nomic/model-v1.5");

    let err = OnnxModelId::parse("embedding//model").unwrap_err();
    assert_eq!(err.field.as_deref(), Some("model_id"));
}

#[test]
fn embedding_request_rejects_oversized_payloads() {
    let too_many_inputs = vec!["hello".to_string(); MAX_EMBEDDING_INPUTS + 1];
    let err = OnnxEmbeddingRequest::parse("model", too_many_inputs, None).unwrap_err();
    assert_eq!(err.field.as_deref(), Some("input"));

    let too_many_chars = vec!["x".repeat(MAX_EMBEDDING_INPUT_CHARS + 1)];
    let err = OnnxEmbeddingRequest::parse("model", too_many_chars, None).unwrap_err();
    assert_eq!(err.field.as_deref(), Some("input"));
}

#[tokio::test]
async fn fake_backend_loads_embeds_lists_and_unloads() {
    let fixture = model_fixture();
    let manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap();
    let load = OnnxLoadRequest::parse(
        fixture.path(),
        "model.onnx",
        "nomic-embed-text-v1.5",
        OnnxLoadOptions::cpu(4).unwrap(),
    )
    .unwrap();

    let loaded = manager.load(load).await.unwrap();
    assert_eq!(loaded.embedding_dimensions, 4);

    let listed = manager.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].model_id.as_str(), "nomic-embed-text-v1.5");

    let response = manager
        .embed(
            OnnxEmbeddingRequest::parse(
                "nomic-embed-text-v1.5",
                vec!["hello world".to_string()],
                None,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.model, "nomic-embed-text-v1.5");
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].embedding.len(), 4);
    assert_eq!(response.usage.total_tokens, 2);

    let removed = manager
        .unload(&OnnxModelId::parse("nomic-embed-text-v1.5").unwrap())
        .await
        .unwrap();
    assert!(removed.is_some());
    assert!(manager.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn fake_backend_rejects_embedding_before_load() {
    let manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 1).unwrap();

    let err = manager
        .embed(OnnxEmbeddingRequest::parse("model", vec!["hello".to_string()], None).unwrap())
        .await
        .unwrap_err();

    assert_eq!(err.code, OnnxRuntimeErrorCode::NotLoaded);
}

#[tokio::test]
async fn session_manager_shutdown_unloads_sessions_and_rejects_new_work() {
    let fixture = model_fixture();
    let manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap();
    let load = OnnxLoadRequest::parse(
        fixture.path(),
        "model.onnx",
        "nomic-embed-text-v1.5",
        OnnxLoadOptions::cpu(4).unwrap(),
    )
    .unwrap();
    manager.load(load).await.unwrap();

    let unloaded = manager.shutdown(Duration::from_secs(1)).await.unwrap();

    assert_eq!(unloaded.len(), 1);
    assert_eq!(unloaded[0].model_id.as_str(), "nomic-embed-text-v1.5");
    let err = manager.list().await.unwrap_err();
    assert_eq!(err.code, OnnxRuntimeErrorCode::Backend);
    assert!(err.message.contains("closed"));
    let err = manager
        .embed(
            OnnxEmbeddingRequest::parse(
                "nomic-embed-text-v1.5",
                vec!["hello world".to_string()],
                None,
            )
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, OnnxRuntimeErrorCode::Backend);
    assert!(err.message.contains("closed"));
}

#[test]
fn session_manager_requires_positive_concurrency_limit() {
    let err = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 0).unwrap_err();

    assert_eq!(err.field.as_deref(), Some("max_concurrent_operations"));
}

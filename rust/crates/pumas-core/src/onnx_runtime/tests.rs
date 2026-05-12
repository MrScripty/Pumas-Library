use super::*;
use std::{path::PathBuf, time::Duration};

fn model_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("model.onnx"), b"fake").unwrap();
    temp
}

fn model_fixture_with_tokenizer() -> tempfile::TempDir {
    let temp = model_fixture();
    std::fs::write(temp.path().join("tokenizer.json"), tokenizer_fixture_json()).unwrap();
    std::fs::write(
        temp.path().join("config.json"),
        config_fixture_json(768, 768),
    )
    .unwrap();
    temp
}

fn nested_model_fixture_with_tokenizer_at_root() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("onnx")).unwrap();
    std::fs::write(temp.path().join("onnx").join("model.onnx"), b"fake").unwrap();
    std::fs::write(temp.path().join("tokenizer.json"), tokenizer_fixture_json()).unwrap();
    std::fs::write(
        temp.path().join("config.json"),
        config_fixture_json(768, 768),
    )
    .unwrap();
    temp
}

fn config_fixture_json(hidden_size: usize, n_embd: usize) -> String {
    format!(
        r#"{{
  "hidden_size": {hidden_size},
  "n_embd": {n_embd},
  "model_type": "nomic_bert"
}}"#
    )
}

fn optional_real_fixture_load_request() -> Option<OnnxLoadRequest> {
    let root = std::env::var_os("PUMAS_ONNX_REAL_MODEL_ROOT")?;
    let model_path = std::env::var_os("PUMAS_ONNX_REAL_MODEL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("onnx/model_fp16.onnx"));
    Some(
        OnnxLoadRequest::parse(
            PathBuf::from(root),
            model_path,
            "nomic-embed-text-v1.5",
            OnnxLoadOptions::default(),
        )
        .unwrap(),
    )
}

fn tokenizer_fixture_json() -> &'static str {
    r#"{
  "version": "1.0",
  "truncation": null,
  "padding": null,
  "added_tokens": [],
  "normalizer": null,
  "pre_tokenizer": { "type": "WhitespaceSplit" },
  "post_processor": null,
  "decoder": null,
  "model": {
    "type": "WordLevel",
    "vocab": {
      "[UNK]": 0,
      "hello": 1,
      "world": 2
    },
    "unk_token": "[UNK]"
  }
}"#
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

#[test]
fn tokenizer_loads_from_model_directory_and_tokenizes_batch() {
    let fixture = model_fixture_with_tokenizer();
    let model_path = OnnxModelPath::parse(fixture.path(), "model.onnx").unwrap();
    let tokenizer = OnnxTokenizer::from_model_path(&model_path).unwrap();

    let batch = tokenizer
        .tokenize_request(
            &OnnxEmbeddingRequest::parse(
                "nomic-embed-text-v1.5",
                vec!["hello world".to_string(), "hello missing".to_string()],
                None,
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(
        tokenizer.tokenizer_path(),
        fixture
            .path()
            .join("tokenizer.json")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(batch.total_tokens, 4);
    assert_eq!(batch.inputs.len(), 2);
    assert_eq!(batch.inputs[0].input_ids, vec![1, 2]);
    assert_eq!(batch.inputs[0].attention_mask, vec![1, 1]);
    assert_eq!(batch.inputs[0].token_count, 2);
    assert_eq!(batch.inputs[1].input_ids, vec![1, 0]);
}

#[test]
fn tokenizer_loads_from_model_package_root_for_nested_onnx_file() {
    let fixture = nested_model_fixture_with_tokenizer_at_root();
    let model_path = OnnxModelPath::parse(fixture.path(), "onnx/model.onnx").unwrap();
    let tokenizer = OnnxTokenizer::from_model_path(&model_path).unwrap();

    assert_eq!(
        tokenizer.tokenizer_path(),
        fixture
            .path()
            .join("tokenizer.json")
            .canonicalize()
            .unwrap()
    );
}

#[test]
fn tokenizer_requires_tokenizer_json_under_model_root() {
    let fixture = model_fixture();
    let model_path = OnnxModelPath::parse(fixture.path(), "model.onnx").unwrap();

    let err = OnnxTokenizer::from_model_path(&model_path).unwrap_err();

    assert_eq!(err.field.as_deref(), Some("tokenizer"));
}

#[test]
fn model_config_loads_dimensions_from_package_root_for_nested_onnx_file() {
    let fixture = nested_model_fixture_with_tokenizer_at_root();
    let model_path = OnnxModelPath::parse(fixture.path(), "onnx/model.onnx").unwrap();
    let model_config = OnnxModelConfig::from_model_path(&model_path).unwrap();

    assert_eq!(model_config.embedding_dimensions(), 768);
    assert_eq!(
        model_config.config_path(),
        fixture.path().join("config.json").canonicalize().unwrap()
    );
}

#[test]
fn model_config_rejects_conflicting_dimension_metadata() {
    let fixture = nested_model_fixture_with_tokenizer_at_root();
    std::fs::write(
        fixture.path().join("config.json"),
        config_fixture_json(768, 384),
    )
    .unwrap();
    let model_path = OnnxModelPath::parse(fixture.path(), "onnx/model.onnx").unwrap();

    let err = OnnxModelConfig::from_model_path(&model_path).unwrap_err();

    assert_eq!(err.field.as_deref(), Some("config"));
    assert!(err.message.contains("hidden_size"));
}

#[test]
fn tokenizer_rejects_inputs_over_token_limit() {
    let fixture = model_fixture_with_tokenizer();
    let model_path = OnnxModelPath::parse(fixture.path(), "model.onnx").unwrap();
    let tokenizer = OnnxTokenizer::from_model_path(&model_path).unwrap();
    let oversized = std::iter::repeat_n("hello", 8_193)
        .collect::<Vec<_>>()
        .join(" ");

    let err = tokenizer
        .tokenize_request(
            &OnnxEmbeddingRequest::parse("nomic-embed-text-v1.5", vec![oversized], None).unwrap(),
        )
        .unwrap_err();

    assert_eq!(err.field.as_deref(), Some("input"));
    assert!(err.message.contains("tokens"));
}

#[test]
fn real_session_loader_rejects_explicit_dimensions_that_disagree_with_config() {
    let fixture = model_fixture_with_tokenizer();
    let request = OnnxLoadRequest::parse(
        fixture.path(),
        "model.onnx",
        "nomic-embed-text-v1.5",
        OnnxLoadOptions::cpu(384).unwrap(),
    )
    .unwrap();

    let err = match OnnxRuntimeSession::load(request) {
        Ok(_) => panic!("mismatched embedding dimensions must reject before session load"),
        Err(err) => err,
    };

    assert_eq!(err.field.as_deref(), Some("embedding_dimensions"));
}

#[test]
fn real_session_loader_uses_validated_model_directory_contract() {
    let fixture = model_fixture_with_tokenizer();
    let request = OnnxLoadRequest::parse(
        fixture.path(),
        "model.onnx",
        "nomic-embed-text-v1.5",
        OnnxLoadOptions::default(),
    )
    .unwrap();

    let err = match OnnxRuntimeSession::load(request) {
        Ok(_) => panic!("fake ONNX bytes must not load as a real ONNX Runtime session"),
        Err(err) => err,
    };

    assert_eq!(err.code, OnnxRuntimeErrorCode::Backend);
    assert!(err.message.contains("model load failed"));
}

#[test]
fn real_session_loader_smokes_optional_real_fixture() {
    let Some(request) = optional_real_fixture_load_request() else {
        return;
    };

    let session = OnnxRuntimeSession::load(request).unwrap();

    assert_eq!(session.status().embedding_dimensions, 768);
    assert_eq!(session.model_config().embedding_dimensions(), 768);
    assert!(session
        .input_names()
        .iter()
        .any(|name| name.as_str() == "input_ids"));
    assert!(session
        .input_names()
        .iter()
        .any(|name| name.as_str() == "attention_mask"));
    assert!(!session.output_names().is_empty());
}

#[tokio::test]
async fn real_backend_embeds_optional_real_fixture() {
    let Some(request) = optional_real_fixture_load_request() else {
        return;
    };
    let manager = OnnxSessionManager::new(RealOnnxEmbeddingBackend::new(), 1).unwrap();
    manager.load(request).await.unwrap();

    let response = manager
        .embed(
            OnnxEmbeddingRequest::parse(
                "nomic-embed-text-v1.5",
                vec![
                    "search_query: hello world".to_string(),
                    "search_document: hello world".to_string(),
                ],
                Some(256),
            )
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.model, "nomic-embed-text-v1.5");
    assert_eq!(response.data.len(), 2);
    assert_eq!(response.data[0].index, 0);
    assert_eq!(response.data[1].index, 1);
    assert_eq!(response.data[0].embedding.len(), 256);
    assert_eq!(response.data[1].embedding.len(), 256);
    assert!(response
        .data
        .iter()
        .flat_map(|item| &item.embedding)
        .all(|value| value.is_finite()));
    assert!(response.usage.total_tokens > 0);
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

#[test]
fn execution_provider_has_stable_log_label() {
    assert_eq!(OnnxExecutionProvider::Cpu.as_str(), "cpu");
}

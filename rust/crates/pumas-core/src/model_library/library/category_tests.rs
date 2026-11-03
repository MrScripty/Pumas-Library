//! Use categories are metadata: detection cases from the owner's library and
//! the rule that a category change never moves a model or changes its ID.
use super::*;
use crate::api::RuntimeTasks;
use tempfile::TempDir;

async fn fixture() -> (TempDir, ModelLibrary) {
    let temp = tempfile::tempdir().unwrap();
    let library = ModelLibrary::new(temp.path().join("library"))
        .await
        .unwrap();
    let downloads = temp.path().join("downloads");
    std::fs::create_dir(&downloads).unwrap();
    library
        .install_mutation_authority(
            RuntimeTasks::new(),
            crate::model_library::DownloadDestinationRoot::open(library.library_root()).unwrap(),
            Arc::new(crate::model_library::DownloadPersistence::new(&downloads)),
        )
        .unwrap();
    (temp, library)
}

/// A GGUF header carrying string keys, as llama.cpp writes them.
fn gguf(keys: &[(&str, &str)]) -> Vec<u8> {
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&3_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    for (key, value) in keys {
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key.as_bytes());
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
    bytes
}

/// A one-tensor safetensors file with the given tensor name.
fn safetensors(tensor: &str) -> Vec<u8> {
    let header = format!(r#"{{"{tensor}":{{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}}}"#);
    [
        (header.len() as u64).to_le_bytes().as_slice(),
        header.as_bytes(),
        1_f32.to_le_bytes().as_slice(),
    ]
    .concat()
}

async fn add_model(
    library: &ModelLibrary,
    model_id: &str,
    files: &[(&str, Vec<u8>)],
    metadata: ModelMetadata,
) -> PathBuf {
    let dir = library.library_root().join(model_id);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    let parts: Vec<&str> = model_id.split('/').collect();
    let metadata = ModelMetadata {
        model_id: Some(model_id.to_string()),
        model_type: Some(parts[0].to_string()),
        family: Some(parts[1].to_string()),
        cleaned_name: Some(parts[2].to_string()),
        official_name: Some(parts[2].to_string()),
        ..metadata
    };
    library.save_metadata(&dir, &metadata).await.unwrap();
    library.index_model_dir(&dir).await.unwrap();
    dir
}

fn recorded_category(library: &ModelLibrary, model_id: &str) -> (String, Option<String>) {
    let metadata = library
        .load_metadata(&library.library_root().join(model_id))
        .unwrap()
        .unwrap();
    let indexed = library.index().get(model_id).unwrap().unwrap();
    assert_eq!(
        metadata.model_type.as_deref(),
        Some(indexed.model_type.as_str()),
        "metadata and index must agree"
    );
    (indexed.model_type, metadata.model_type_resolution_source)
}

#[tokio::test]
async fn decision_gguf_is_recorded_as_decision_in_place() {
    // Kev: HF tags it text-classification and it was filed under llm/.
    let (_temp, library) = fixture().await;
    let id = "llm/qwen35/ggml-org--kev-4b-gguf";
    let dir = add_model(
        &library,
        id,
        &[(
            "Kev-4B-Q8_0.gguf",
            gguf(&[
                ("general.architecture", "qwen35"),
                ("general.type", "model"),
                ("qwen35.decision.type", "kev"),
            ]),
        )],
        ModelMetadata {
            pipeline_tag: Some("text-classification".into()),
            ..Default::default()
        },
    )
    .await;

    assert_eq!(
        library.reclassify_model(id).await.unwrap().as_deref(),
        Some(id),
        "the category changes, the ID does not"
    );
    assert!(dir.exists(), "a category change never moves a model");
    assert_eq!(
        recorded_category(&library, id),
        (
            "decision".into(),
            Some("model-type-gguf-decision-head".into())
        )
    );
    // Settled: a second pass has nothing to change.
    assert_eq!(library.reclassify_model(id).await.unwrap(), None);
}

#[tokio::test]
async fn image_to_video_task_is_video_despite_audio_tensors() {
    // LTX-2.3: one safetensors (with audio VAE tensors), task image-to-video.
    let (_temp, library) = fixture().await;
    let id = "audio/ltx-2_3/ltx-2_3-fp8";
    let dir = add_model(
        &library,
        id,
        &[(
            "ltx-2.3-22b-distilled-fp8.safetensors",
            safetensors("audio_vae.decoder.conv_in.weight"),
        )],
        ModelMetadata {
            pipeline_tag: Some("image-to-video".into()),
            ..Default::default()
        },
    )
    .await;

    library.reclassify_model(id).await.unwrap();
    assert!(dir.exists());
    assert_eq!(recorded_category(&library, id).0, "video");
    let metadata = library.load_metadata(&dir).unwrap().unwrap();
    assert_eq!(metadata.output_modalities, Some(vec!["video".to_string()]));
}

#[tokio::test]
async fn asr_architecture_outranks_a_feature_extraction_tag() {
    // Granite Speech 4.1 2b-nar: config says GraniteSpeechNarForASR, the HF
    // repo tag says feature-extraction.
    let (_temp, library) = fixture().await;
    let id = "embedding/granite-speech-4_1/granite-speech-4_1-2b-nar";
    let dir = add_model(
        &library,
        id,
        &[
            ("model.safetensors", safetensors("encoder.layers.0.weight")),
            (
                "config.json",
                br#"{"architectures":["GraniteSpeechNarForASR"],"model_type":"granite_speech_nar"}"#
                    .to_vec(),
            ),
        ],
        ModelMetadata {
            pipeline_tag: Some("feature-extraction".into()),
            ..Default::default()
        },
    )
    .await;

    library.reclassify_model(id).await.unwrap();
    assert!(dir.exists());
    assert_eq!(recorded_category(&library, id).0, "audio");
    let metadata = library.load_metadata(&dir).unwrap().unwrap();
    assert!(metadata
        .review_reasons
        .unwrap_or_default()
        .contains(&"model-type-hint-conflict".to_string()));
}

#[tokio::test]
async fn chosen_category_survives_reclassification_until_cleared() {
    let (_temp, library) = fixture().await;
    let id = "llm/qwen35/ggml-org--kev-4b-gguf";
    let dir = add_model(
        &library,
        id,
        &[(
            "Kev-4B-Q8_0.gguf",
            gguf(&[
                ("general.architecture", "qwen35"),
                ("qwen35.decision.type", "kev"),
            ]),
        )],
        ModelMetadata::default(),
    )
    .await;

    assert_eq!(
        library
            .set_model_category(id, Some(ModelType::Vision))
            .await
            .unwrap(),
        ModelType::Vision
    );
    assert!(dir.exists());
    assert_eq!(
        recorded_category(&library, id),
        ("vision".into(), Some(USER_CATEGORY_SOURCE.into()))
    );
    library.reclassify_model(id).await.unwrap();
    assert_eq!(
        recorded_category(&library, id).0,
        "vision",
        "the choice wins"
    );

    assert!(library
        .set_model_category(id, Some(ModelType::Unknown))
        .await
        .is_err());

    assert_eq!(
        library.set_model_category(id, None).await.unwrap(),
        ModelType::Decision,
        "clearing the choice re-detects"
    );
    assert!(dir.exists());
    assert_eq!(recorded_category(&library, id).0, "decision");
}

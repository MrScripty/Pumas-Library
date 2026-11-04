use pumas_library::model_library::{validate_metadata_v2, ModelMetadata};
use std::path::Path;

#[test]
fn test_qwen_image_2512_metadata_matches_curated_diffusion_contract() {
    let metadata_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../shared-resources/models/diffusion/qwen/qwen-image-2512/metadata.json");

    let metadata_json = std::fs::read_to_string(&metadata_path).unwrap();
    let metadata: ModelMetadata = serde_json::from_str(&metadata_json).unwrap();

    validate_metadata_v2(&metadata).unwrap();
    assert_eq!(metadata.schema_version, Some(2));
    assert_eq!(metadata.task_type_primary.as_deref(), Some("text-to-image"));
    assert_eq!(
        metadata.input_modalities.as_deref(),
        Some(&["text".to_string()][..])
    );
    assert_eq!(
        metadata.output_modalities.as_deref(),
        Some(&["image".to_string()][..])
    );
    assert_eq!(metadata.recommended_backend.as_deref(), Some("pytorch"));
}

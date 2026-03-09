//! Side-effect-free classification for import paths.
//!
//! This module determines whether an import path should be treated as a file,
//! a directory-root bundle, a single model directory, a multi-model container,
//! or an unsupported/ambiguous path before any persistence occurs.

use crate::model_library::external_assets::validate_diffusers_directory_for_import;
use crate::model_library::identifier::{identify_model_type, ModelTypeInfo};
use crate::model_library::sharding;
use crate::model_library::types::FileFormat;
use crate::models::{
    AssetValidationState, BundleFormat, ImportPathCandidate, ImportPathCandidateKind,
    ImportPathClassification, ImportPathClassificationKind,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn classify_import_path(path: impl AsRef<Path>) -> ImportPathClassification {
    let path = path.as_ref();
    let display_path = path.display().to_string();

    if !path.exists() {
        return unsupported(
            &display_path,
            format!("path not found: {}", display_path),
            Vec::new(),
        );
    }

    if path.is_file() {
        return classify_file(path);
    }

    if path.is_dir() {
        return classify_directory(path);
    }

    unsupported(
        &display_path,
        "path is neither a regular file nor a directory".to_string(),
        Vec::new(),
    )
}

fn classify_file(path: &Path) -> ImportPathClassification {
    let display_path = path.display().to_string();
    match identify_model_type(path) {
        Ok(type_info) if type_info.format != FileFormat::Unknown => ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::SingleFile,
            suggested_family: Some(
                type_info
                    .family
                    .as_ref()
                    .map(|family| family.to_string())
                    .unwrap_or_else(|| "imported".to_string()),
            ),
            suggested_official_name: Some(path_stem_or_name(path)),
            model_type: model_type_string(&type_info),
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons: vec!["recognized model file".to_string()],
            candidates: Vec::new(),
        },
        Ok(_) => unsupported(
            &display_path,
            "file is not a supported model artifact".to_string(),
            Vec::new(),
        ),
        Err(err) => unsupported(
            &display_path,
            format!("could not inspect file: {}", err),
            Vec::new(),
        ),
    }
}

fn classify_directory(path: &Path) -> ImportPathClassification {
    let display_path = path.display().to_string();

    let bundle_validation = validate_diffusers_directory_for_import(path);
    if bundle_validation.validation_state == AssetValidationState::Valid {
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::SingleBundle,
            suggested_family: Some("imported".to_string()),
            suggested_official_name: Some(path_name(path)),
            model_type: Some("diffusion".to_string()),
            bundle_format: Some(BundleFormat::DiffusersDirectory),
            pipeline_class: bundle_validation.pipeline_class,
            component_manifest: Some(bundle_validation.component_manifest),
            reasons: vec![
                "directory root is a supported diffusers bundle".to_string(),
                "bundle internals are treated as one executable asset".to_string(),
            ],
            candidates: Vec::new(),
        };
    }

    let child_candidates = collect_immediate_child_candidates(path);
    let terminal_dirs: HashSet<PathBuf> = child_candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.kind,
                ImportPathCandidateKind::DirectoryModel
                    | ImportPathCandidateKind::ExternalDiffusersBundle
            )
        })
        .map(|candidate| PathBuf::from(&candidate.path))
        .collect();
    let root_analysis = analyze_root_directory(path, &terminal_dirs);

    if !child_candidates.is_empty() && root_analysis.unit_count > 0 {
        let mut reasons = vec![
            "directory root appears importable, but also contains separate import candidates"
                .to_string(),
        ];
        reasons.extend(root_analysis.reasons);
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::Ambiguous,
            suggested_family: None,
            suggested_official_name: None,
            model_type: root_analysis.model_type,
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons,
            candidates: merge_candidates(root_analysis.file_candidates, child_candidates),
        };
    }

    if child_candidates.len() >= 2 {
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::MultiModelContainer,
            suggested_family: None,
            suggested_official_name: None,
            model_type: None,
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons: vec!["directory contains multiple independent import candidates".to_string()],
            candidates: child_candidates,
        };
    }

    if child_candidates.len() == 1 {
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::Ambiguous,
            suggested_family: None,
            suggested_official_name: None,
            model_type: None,
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons: vec![
                "directory contains one nested import candidate; import the child directly"
                    .to_string(),
            ],
            candidates: child_candidates,
        };
    }

    if root_analysis.unit_count == 1 {
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::SingleModelDirectory,
            suggested_family: Some(
                root_analysis
                    .family
                    .unwrap_or_else(|| "imported".to_string()),
            ),
            suggested_official_name: Some(path_name(path)),
            model_type: root_analysis.model_type,
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons: root_analysis.reasons,
            candidates: Vec::new(),
        };
    }

    if root_analysis.unit_count >= 2 {
        return ImportPathClassification {
            path: display_path,
            kind: ImportPathClassificationKind::MultiModelContainer,
            suggested_family: None,
            suggested_official_name: None,
            model_type: None,
            bundle_format: None,
            pipeline_class: None,
            component_manifest: None,
            reasons: vec![
                "directory contains multiple model files or sharded groups at the root".to_string(),
            ],
            candidates: root_analysis.file_candidates,
        };
    }

    unsupported(
        &display_path,
        "directory does not match a supported bundle or model layout".to_string(),
        Vec::new(),
    )
}

fn collect_immediate_child_candidates(path: &Path) -> Vec<ImportPathCandidate> {
    let mut candidates = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return candidates;
    };

    for entry in entries.filter_map(|entry| entry.ok()) {
        let child_path = entry.path();
        if child_path.is_dir() {
            let bundle_validation = validate_diffusers_directory_for_import(&child_path);
            if bundle_validation.validation_state == AssetValidationState::Valid {
                candidates.push(ImportPathCandidate {
                    path: child_path.display().to_string(),
                    kind: ImportPathCandidateKind::ExternalDiffusersBundle,
                    display_name: path_name(&child_path),
                    model_type: Some("diffusion".to_string()),
                    bundle_format: Some(BundleFormat::DiffusersDirectory),
                    pipeline_class: bundle_validation.pipeline_class,
                    component_manifest: Some(bundle_validation.component_manifest),
                    reasons: vec!["child directory is a supported diffusers bundle".to_string()],
                });
                continue;
            }

            if let Some(candidate) = classify_child_directory_model(&child_path) {
                candidates.push(candidate);
            }
            continue;
        }
    }

    sort_candidates(&mut candidates);
    candidates
}

fn classify_child_directory_model(path: &Path) -> Option<ImportPathCandidate> {
    let analysis = analyze_root_directory(path, &HashSet::new());
    if analysis.unit_count != 1 {
        return None;
    }

    Some(ImportPathCandidate {
        path: path.display().to_string(),
        kind: ImportPathCandidateKind::DirectoryModel,
        display_name: path_name(path),
        model_type: analysis.model_type,
        bundle_format: None,
        pipeline_class: None,
        component_manifest: None,
        reasons: analysis.reasons,
    })
}

struct RootDirectoryAnalysis {
    unit_count: usize,
    model_type: Option<String>,
    family: Option<String>,
    reasons: Vec<String>,
    file_candidates: Vec<ImportPathCandidate>,
}

fn analyze_root_directory(path: &Path, terminal_dirs: &HashSet<PathBuf>) -> RootDirectoryAnalysis {
    let model_files = collect_model_files(path, terminal_dirs);
    if model_files.is_empty() {
        return RootDirectoryAnalysis {
            unit_count: 0,
            model_type: None,
            family: None,
            reasons: Vec::new(),
            file_candidates: Vec::new(),
        };
    }

    let sharded_groups = sharding::detect_sharded_sets(&model_files);
    let grouped_paths: HashSet<PathBuf> = sharded_groups
        .values()
        .flat_map(|group| group.iter().cloned())
        .collect();
    let standalone_count = model_files
        .iter()
        .filter(|path| !grouped_paths.contains(*path))
        .count();
    let unit_count = sharded_groups.len() + standalone_count;

    let primary_type = pick_primary_model_info(&model_files);
    let mut file_candidates: Vec<ImportPathCandidate> = model_files
        .iter()
        .filter_map(|model_file| {
            let type_info = recognized_model_file(model_file)?;
            Some(ImportPathCandidate {
                path: model_file.display().to_string(),
                kind: ImportPathCandidateKind::FileModel,
                display_name: model_file
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string(),
                model_type: model_type_string(&type_info),
                bundle_format: None,
                pipeline_class: None,
                component_manifest: None,
                reasons: vec!["root contains a recognized model file".to_string()],
            })
        })
        .collect();
    sort_candidates(&mut file_candidates);

    let mut reasons = vec!["directory root contains one importable model layout".to_string()];
    if sharded_groups.len() == 1 && standalone_count == 0 {
        reasons = vec!["directory root contains one sharded model set".to_string()];
    } else if unit_count > 1 {
        reasons = vec!["directory root contains multiple model files or groups".to_string()];
    }

    RootDirectoryAnalysis {
        unit_count,
        model_type: primary_type.as_ref().and_then(model_type_string),
        family: primary_type
            .as_ref()
            .and_then(|info| info.family.as_ref().map(|family| family.to_string())),
        reasons,
        file_candidates,
    }
}

fn collect_model_files(path: &Path, terminal_dirs: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = WalkDir::new(path).into_iter().filter_entry(|entry| {
        if entry.depth() != 1 || !entry.file_type().is_dir() {
            return true;
        }
        !terminal_dirs.contains(entry.path())
    });

    for entry in walker.filter_map(|entry| entry.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        if recognized_model_file(entry.path()).is_some() {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
    files
}

fn pick_primary_model_info(paths: &[PathBuf]) -> Option<ModelTypeInfo> {
    let mut largest: Option<(&PathBuf, u64)> = None;
    for path in paths {
        let Ok(metadata) = std::fs::metadata(path) else {
            continue;
        };
        let size = metadata.len();
        if largest.is_none() || size > largest.map(|(_, largest_size)| largest_size).unwrap_or(0) {
            largest = Some((path, size));
        }
    }

    largest.and_then(|(path, _)| recognized_model_file(path))
}

fn recognized_model_file(path: &Path) -> Option<ModelTypeInfo> {
    let Ok(type_info) = identify_model_type(path) else {
        return None;
    };
    if type_info.format == FileFormat::Unknown {
        return None;
    }
    Some(type_info)
}

fn model_type_string(type_info: &ModelTypeInfo) -> Option<String> {
    let model_type = type_info.model_type.as_str();
    if model_type == "unknown" {
        None
    } else {
        Some(model_type.to_string())
    }
}

fn path_name(path: &Path) -> String {
    if let Some(value) = path.file_name().and_then(|value| value.to_str()) {
        value.to_string()
    } else {
        path.display().to_string()
    }
}

fn path_stem_or_name(path: &Path) -> String {
    if let Some(value) = path.file_stem().and_then(|value| value.to_str()) {
        value.to_string()
    } else {
        path_name(path)
    }
}

fn merge_candidates(
    mut left: Vec<ImportPathCandidate>,
    mut right: Vec<ImportPathCandidate>,
) -> Vec<ImportPathCandidate> {
    left.append(&mut right);
    sort_candidates(&mut left);
    left
}

fn sort_candidates(candidates: &mut [ImportPathCandidate]) {
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
}

fn unsupported(
    path: &str,
    reason: String,
    candidates: Vec<ImportPathCandidate>,
) -> ImportPathClassification {
    ImportPathClassification {
        path: path.to_string(),
        kind: ImportPathClassificationKind::Unsupported,
        suggested_family: None,
        suggested_official_name: None,
        model_type: None,
        bundle_format: None,
        pipeline_class: None,
        component_manifest: None,
        reasons: vec![reason],
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_test_model_file(path: &Path) {
        std::fs::write(path, b"ONNX").unwrap();
    }

    fn write_diffusers_bundle(root: &Path) {
        std::fs::create_dir_all(root.join("unet")).unwrap();
        std::fs::create_dir_all(root.join("vae")).unwrap();
        std::fs::write(
            root.join("model_index.json"),
            r#"{
                "_class_name": "AutoPipelineForText2Image",
                "unet": ["diffusers", "UNet2DConditionModel"],
                "vae": ["diffusers", "AutoencoderKL"]
            }"#,
        )
        .unwrap();
    }

    #[test]
    fn test_classify_import_path_single_bundle() {
        let temp = tempdir().unwrap();
        let bundle_root = temp.path().join("tiny-sd-turbo");
        std::fs::create_dir_all(&bundle_root).unwrap();
        write_diffusers_bundle(&bundle_root);

        let result = classify_import_path(&bundle_root);
        assert_eq!(result.kind, ImportPathClassificationKind::SingleBundle);
        assert_eq!(result.bundle_format, Some(BundleFormat::DiffusersDirectory));
    }

    #[test]
    fn test_classify_import_path_single_bundle_ignores_boolean_metadata_fields() {
        let temp = tempdir().unwrap();
        let bundle_root = temp.path().join("tiny-sd-turbo");
        std::fs::create_dir_all(bundle_root.join("scheduler")).unwrap();
        std::fs::create_dir_all(bundle_root.join("text_encoder")).unwrap();
        std::fs::create_dir_all(bundle_root.join("tokenizer")).unwrap();
        std::fs::create_dir_all(bundle_root.join("unet")).unwrap();
        std::fs::create_dir_all(bundle_root.join("vae")).unwrap();
        std::fs::write(
            bundle_root.join("model_index.json"),
            r#"{
                "_class_name": "StableDiffusionPipeline",
                "_diffusers_version": "0.32.0",
                "_name_or_path": "stabilityai/sd-turbo",
                "feature_extractor": [null, null],
                "image_encoder": [null, null],
                "requires_safety_checker": true,
                "safety_checker": [null, null],
                "scheduler": ["diffusers", "EulerDiscreteScheduler"],
                "text_encoder": ["transformers", "CLIPTextModel"],
                "tokenizer": ["transformers", "CLIPTokenizer"],
                "unet": ["diffusers", "UNet2DConditionModel"],
                "vae": ["diffusers", "AutoencoderTiny"]
            }"#,
        )
        .unwrap();

        let result = classify_import_path(&bundle_root);

        assert_eq!(result.kind, ImportPathClassificationKind::SingleBundle);
        assert_eq!(result.bundle_format, Some(BundleFormat::DiffusersDirectory));
    }

    #[test]
    fn test_classify_import_path_multi_model_container_for_sibling_dirs() {
        let temp = tempdir().unwrap();
        let container = temp.path().join("models");
        let child_a = container.join("alpha");
        let child_b = container.join("beta");
        std::fs::create_dir_all(&child_a).unwrap();
        std::fs::create_dir_all(&child_b).unwrap();
        write_test_model_file(&child_a.join("alpha.onnx"));
        write_test_model_file(&child_b.join("beta.onnx"));

        let result = classify_import_path(&container);
        assert_eq!(
            result.kind,
            ImportPathClassificationKind::MultiModelContainer
        );
        assert_eq!(result.candidates.len(), 2);
    }

    #[test]
    fn test_classify_import_path_multi_model_container_for_root_files() {
        let temp = tempdir().unwrap();
        let container = temp.path().join("models");
        std::fs::create_dir_all(&container).unwrap();
        write_test_model_file(&container.join("alpha.onnx"));
        write_test_model_file(&container.join("beta.onnx"));

        let result = classify_import_path(&container);
        assert_eq!(
            result.kind,
            ImportPathClassificationKind::MultiModelContainer
        );
        assert_eq!(result.candidates.len(), 2);
    }

    #[test]
    fn test_classify_import_path_marks_bundle_plus_sibling_as_ambiguous() {
        let temp = tempdir().unwrap();
        let container = temp.path().join("models");
        let bundle_root = container.join("tiny-sd-turbo");
        std::fs::create_dir_all(&bundle_root).unwrap();
        write_diffusers_bundle(&bundle_root);
        write_test_model_file(&container.join("extra.onnx"));

        let result = classify_import_path(&container);
        assert_eq!(result.kind, ImportPathClassificationKind::Ambiguous);
        assert_eq!(result.candidates.len(), 2);
    }

    #[test]
    fn test_classify_import_path_marks_root_and_child_candidates_as_ambiguous() {
        let temp = tempdir().unwrap();
        let container = temp.path().join("models");
        let child = container.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        write_test_model_file(&container.join("root.onnx"));
        write_test_model_file(&child.join("nested.onnx"));

        let result = classify_import_path(&container);
        assert_eq!(result.kind, ImportPathClassificationKind::Ambiguous);
        assert_eq!(result.candidates.len(), 2);
    }

    #[test]
    fn test_classify_import_path_unsupported_directory() {
        let temp = tempdir().unwrap();
        let directory = temp.path().join("empty");
        std::fs::create_dir_all(&directory).unwrap();

        let result = classify_import_path(&directory);
        assert_eq!(result.kind, ImportPathClassificationKind::Unsupported);
    }
}

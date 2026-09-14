//! Selected-artifact identity for repository-backed model downloads.
//!
//! A Hugging Face repository can expose several loadable artifacts.  For GGUF
//! repos, the selected artifact is often a single quantized file such as
//! `Q4_K_M` or `Q5_K_M`.  This module keeps that artifact selector separate
//! from the upstream repo id so path planning, progress tracking, and migration
//! can distinguish variants from the same repo.

use crate::model_library::naming::normalize_name;
use crate::model_library::types::DownloadRequest;
use crate::models::{HuggingFaceEvidence, ModelMetadata};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

const DEFAULT_REVISION: &str = "main";
const DIGEST_HEX_LEN: usize = 12;

/// The revision owned by one download execution, independent of its public request.
/// A missing commit preserves the existing operational `main` behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DownloadRevision {
    commit: Option<String>,
}

impl DownloadRevision {
    pub(crate) fn legacy_main() -> Self {
        Self { commit: None }
    }

    pub(crate) fn from_commit(commit: &str) -> crate::Result<Self> {
        if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(crate::PumasError::Validation {
                field: "download.revision".to_string(),
                message:
                    "download revision must be a full immutable 40-character hexadecimal commit"
                        .to_string(),
            });
        }
        Ok(Self {
            commit: Some(commit.to_ascii_lowercase()),
        })
    }

    pub(crate) fn from_persisted(commit: Option<&str>) -> crate::Result<Self> {
        commit.map_or_else(|| Ok(Self::legacy_main()), Self::from_commit)
    }

    pub(crate) fn as_str(&self) -> &str {
        self.commit.as_deref().unwrap_or(DEFAULT_REVISION)
    }

    pub(crate) fn as_persisted(&self) -> Option<&str> {
        self.commit.as_deref()
    }
}

static VERSION_WITH_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b([a-z][a-z0-9_-]*?)(\d+)[._-](\d+)\b").unwrap());
static COMPACT_VERSION_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-z][a-z0-9_-]*?)(\d)(\d)$").unwrap());

/// The type of upstream artifact selection represented by an identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSelectionKind {
    GgufFile,
    FileGroup,
    Quant,
    FullRepo,
    Bundle,
}

impl ArtifactSelectionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GgufFile => "gguf_file",
            Self::FileGroup => "file_group",
            Self::Quant => "quant",
            Self::FullRepo => "full_repo",
            Self::Bundle => "bundle",
        }
    }
}

/// Stable identity for one selected artifact inside an upstream repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SelectedArtifactIdentity {
    pub repo_id: String,
    pub revision: String,
    pub subfolder: Option<String>,
    pub selection_kind: ArtifactSelectionKind,
    pub selected_filenames: Vec<String>,
    pub selected_quant: Option<String>,
    pub artifact_digest: String,
    pub artifact_id: String,
}

impl SelectedArtifactIdentity {
    /// Build a selected-artifact identity from a resolved download request.
    ///
    /// `selected_filenames` should be the final selected artifact filenames
    /// when they are known. When planning a destination before file-tree
    /// resolution, pass `None` and the explicit request selectors will be used.
    pub fn from_download_request(
        request: &DownloadRequest,
        selected_filenames: Option<Vec<String>>,
    ) -> Self {
        Self::from_download_request_at_revision(
            request,
            selected_filenames,
            &DownloadRevision::legacy_main(),
        )
    }

    pub(crate) fn from_download_request_at_revision(
        request: &DownloadRequest,
        selected_filenames: Option<Vec<String>>,
        revision: &DownloadRevision,
    ) -> Self {
        let mut filenames = selected_filenames.unwrap_or_else(|| requested_filenames(request));
        filenames.sort();
        filenames.dedup();

        // A commit fixes the repository contents, so the requested selector
        // identifies the same artifact before and after its files are expanded.
        // Keep discovered filenames as evidence without renaming its destination.
        // Legacy requests retain their existing materialized-file identity rules.
        let identity_filenames = if revision.as_persisted().is_some() {
            let mut requested = requested_filenames(request);
            requested.sort();
            requested.dedup();
            requested
        } else {
            filenames.clone()
        };
        let selection_kind = selection_kind(request, &identity_filenames);
        let selected_quant = request.quant.as_ref().map(|value| normalize_name(value));
        let digest = artifact_digest(request, &identity_filenames, &selection_kind, revision);
        let artifact_selector =
            artifact_selector(request, &identity_filenames, &selection_kind, &digest);
        let mut artifact_id = format!("{}__{}", repo_slug(&request.repo_id), artifact_selector);
        // Several selectors intentionally omit the digest. Include the complete
        // requested-selector digest and pin independently so quantization labels
        // and normalized filenames cannot collapse different requested files.
        if let Some(commit) = revision.as_persisted() {
            artifact_id.push_str("__");
            artifact_id.push_str(&digest);
            artifact_id.push_str("__revision_");
            artifact_id.push_str(commit);
        }

        Self {
            repo_id: request.repo_id.clone(),
            revision: revision.as_str().to_string(),
            subfolder: None,
            selection_kind,
            selected_filenames: filenames,
            selected_quant,
            artifact_digest: digest,
            artifact_id,
        }
    }
}

/// Project selected-artifact download identity into persisted model metadata.
pub fn apply_download_artifact_metadata(
    metadata: &mut ModelMetadata,
    request: &DownloadRequest,
    evidence: Option<&HuggingFaceEvidence>,
) {
    apply_download_artifact_metadata_at_revision(
        metadata,
        request,
        evidence,
        &DownloadRevision::legacy_main(),
    );
}

pub(crate) fn apply_download_artifact_metadata_at_revision(
    metadata: &mut ModelMetadata,
    request: &DownloadRequest,
    evidence: Option<&HuggingFaceEvidence>,
    revision: &DownloadRevision,
) {
    let selected_filenames = evidence.and_then(|value| value.selected_filenames.clone());
    let selected_artifact = SelectedArtifactIdentity::from_download_request_at_revision(
        request,
        selected_filenames,
        revision,
    );

    metadata.publisher = publisher_from_repo_id(&request.repo_id);
    metadata.architecture_family = Some(infer_architecture_family_for_download(request, evidence));
    metadata.config_model_type = evidence.and_then(|value| value.config_model_type.clone());
    metadata.selected_artifact_id = Some(selected_artifact.artifact_id);
    metadata.selected_artifact_files = non_empty(selected_artifact.selected_filenames);
    metadata.selected_artifact_quant = selected_artifact.selected_quant;
    metadata.upstream_revision = Some(selected_artifact.revision);
}

/// Infer the architecture-family token to use for artifact paths.
pub fn infer_architecture_family_for_download(
    request: &DownloadRequest,
    evidence: Option<&HuggingFaceEvidence>,
) -> String {
    let mut candidates = Vec::new();

    if let Some(evidence) = evidence {
        candidates.push(evidence.config_model_type.as_deref());
        if let Some(architectures) = evidence.architectures.as_ref() {
            candidates.extend(architectures.iter().map(String::as_str).map(Some));
        }
    }

    candidates.push(Some(request.official_name.as_str()));
    candidates.push(request.repo_id.split_once('/').map(|(_, name)| name));
    candidates.push(Some(request.family.as_str()));

    for candidate in candidates.into_iter().flatten() {
        if let Some(family) = extract_versioned_family(candidate) {
            return family;
        }
    }

    if let Some(config_model_type) = evidence.and_then(|value| value.config_model_type.as_deref()) {
        return normalize_architecture_family(config_model_type);
    }

    normalize_architecture_family(&request.family)
}

/// Normalize a family/config token while preserving version separators.
pub fn normalize_architecture_family(value: &str) -> String {
    let normalized = normalize_name(value);
    normalize_compact_version_token(&normalized)
}

/// Extract a normalized versioned family token from model/repository text.
pub(crate) fn versioned_architecture_family_from_text(value: &str) -> Option<String> {
    extract_versioned_family(value)
}

/// Normalize a selected-artifact id for path/model-id use while preserving
/// artifact identity separators.
pub fn normalize_artifact_path_slug(value: &str) -> String {
    let normalized = value
        .split("__")
        .map(|artifact_part| {
            artifact_part
                .split("--")
                .map(normalize_name)
                .collect::<Vec<_>>()
                .join("--")
        })
        .collect::<Vec<_>>()
        .join("__")
        .trim_matches(|c| c == '-' || c == '_')
        .to_string();

    if normalized.is_empty() {
        normalize_name(value)
    } else {
        normalized
    }
}

fn requested_filenames(request: &DownloadRequest) -> Vec<String> {
    if let Some(filenames) = request.filenames.as_ref() {
        filenames.clone()
    } else if let Some(filename) = request.filename.as_ref() {
        vec![filename.clone()]
    } else {
        Vec::new()
    }
}

fn selection_kind(
    request: &DownloadRequest,
    selected_filenames: &[String],
) -> ArtifactSelectionKind {
    if request.bundle_format.is_some() {
        ArtifactSelectionKind::Bundle
    } else if request.filenames.is_some() || selected_filenames.len() > 1 {
        ArtifactSelectionKind::FileGroup
    } else if request.filename.as_ref().is_some_and(|name| is_gguf(name)) {
        ArtifactSelectionKind::GgufFile
    } else if request.filename.is_some() {
        ArtifactSelectionKind::FileGroup
    } else if request.quant.is_some() {
        ArtifactSelectionKind::Quant
    } else {
        ArtifactSelectionKind::FullRepo
    }
}

fn artifact_selector(
    request: &DownloadRequest,
    selected_filenames: &[String],
    selection_kind: &ArtifactSelectionKind,
    digest: &str,
) -> String {
    if let Some(quant) = request.quant.as_ref() {
        return normalize_name(quant);
    }

    match selection_kind {
        ArtifactSelectionKind::GgufFile => selected_filenames
            .first()
            .or(request.filename.as_ref())
            .map(|filename| normalize_name(filename))
            .unwrap_or_else(|| format!("gguf_{}", digest)),
        ArtifactSelectionKind::FileGroup => format!("files_{}", digest),
        ArtifactSelectionKind::Bundle => format!("bundle_{}", digest),
        ArtifactSelectionKind::FullRepo => "full_repo".to_string(),
        ArtifactSelectionKind::Quant => format!("quant_{}", digest),
    }
}

fn artifact_digest(
    request: &DownloadRequest,
    selected_filenames: &[String],
    selection_kind: &ArtifactSelectionKind,
    revision: &DownloadRevision,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.repo_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(revision.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(selection_kind.as_str().as_bytes());
    hasher.update(b"\0");
    if let Some(quant) = request.quant.as_ref() {
        hasher.update(normalize_name(quant).as_bytes());
        hasher.update(b"\0");
    }
    for filename in selected_filenames {
        hasher.update(filename.as_bytes());
        hasher.update(b"\0");
    }
    let hex = hex::encode(hasher.finalize());
    hex[..DIGEST_HEX_LEN].to_string()
}

fn repo_slug(repo_id: &str) -> String {
    let (owner, name) = repo_id.split_once('/').unwrap_or(("huggingface", repo_id));
    format!("{}--{}", normalize_name(owner), normalize_name(name))
}

fn publisher_from_repo_id(repo_id: &str) -> Option<String> {
    repo_id
        .split_once('/')
        .map(|(publisher, _)| publisher.trim())
        .filter(|publisher| !publisher.is_empty())
        .map(str::to_string)
}

fn non_empty(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn is_gguf(filename: &str) -> bool {
    filename
        .rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("gguf"))
}

fn extract_versioned_family(value: &str) -> Option<String> {
    let normalized_value = value.replace('_', ".");
    let captures = VERSION_WITH_SEPARATOR.captures(&normalized_value)?;
    let prefix = captures.get(1)?.as_str();
    let major = captures.get(2)?.as_str();
    let minor = captures.get(3)?.as_str();
    Some(normalize_architecture_family(&format!(
        "{prefix}{major}_{minor}"
    )))
}

fn normalize_compact_version_token(value: &str) -> String {
    let Some(captures) = COMPACT_VERSION_TOKEN.captures(value) else {
        return value.to_string();
    };
    let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
    let major = captures.get(2).map(|m| m.as_str()).unwrap_or_default();
    let minor = captures.get(3).map(|m| m.as_str()).unwrap_or_default();
    format!("{prefix}{major}_{minor}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(repo_id: &str) -> DownloadRequest {
        DownloadRequest {
            repo_id: repo_id.to_string(),
            family: "publisher".to_string(),
            official_name: repo_id
                .split_once('/')
                .map(|(_, name)| name.to_string())
                .unwrap_or_else(|| repo_id.to_string()),
            model_type: Some("vlm".to_string()),
            quant: None,
            filename: None,
            filenames: None,
            pipeline_tag: None,
            bundle_format: None,
            pipeline_class: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        }
    }

    #[test]
    fn execution_revision_requires_a_complete_commit_and_preserves_legacy_main() {
        let legacy = DownloadRevision::from_persisted(None).unwrap();
        assert_eq!(legacy.as_str(), "main");
        assert_eq!(legacy.as_persisted(), None);

        for invalid in [
            "",
            "main",
            "refs/main",
            "abc123",
            &"g".repeat(40),
            &"a".repeat(41),
        ] {
            assert!(DownloadRevision::from_commit(invalid).is_err());
            assert!(DownloadRevision::from_persisted(Some(invalid)).is_err());
        }
        let upper = "ABCDEF0123".repeat(4);
        assert_eq!(upper.len(), 40);
        let revision = DownloadRevision::from_commit(&upper).unwrap();
        assert_eq!(revision.as_str(), upper.to_ascii_lowercase());
        assert_eq!(revision.as_str().len(), 40);
        assert_eq!(
            DownloadRevision::from_persisted(revision.as_persisted()).unwrap(),
            revision
        );
    }

    #[test]
    fn pinned_destinations_are_distinct_for_every_artifact_selection_kind() {
        let base = request("Owner/Example");
        let mut gguf = base.clone();
        gguf.filename = Some("weights.gguf".to_string());
        let mut quant = base.clone();
        quant.quant = Some("Q4_K_M".to_string());
        let mut group = base.clone();
        group.filenames = Some(vec![
            "b.safetensors".to_string(),
            "a.safetensors".to_string(),
        ]);
        let mut file = base.clone();
        file.filename = Some("weights.safetensors".to_string());
        let mut bundle = base.clone();
        bundle.bundle_format = Some(crate::models::BundleFormat::DiffusersDirectory);
        let first_revision = DownloadRevision::from_commit(&"a".repeat(40)).unwrap();
        let second_revision = DownloadRevision::from_commit(&"b".repeat(40)).unwrap();

        for request in [base, gguf, quant, group, file, bundle] {
            let legacy = SelectedArtifactIdentity::from_download_request(&request, None);
            let first = SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                None,
                &first_revision,
            );
            let second = SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                None,
                &second_revision,
            );
            assert_eq!(first.revision, first_revision.as_str());
            assert_eq!(second.revision, second_revision.as_str());
            assert_ne!(first.artifact_id, second.artifact_id);
            assert_ne!(first.artifact_digest, second.artifact_digest);
            assert_ne!(first.artifact_id, legacy.artifact_id);
            let mut expanded_files = requested_filenames(&request);
            expanded_files.extend([
                "weights-00002.safetensors".to_string(),
                "weights-00001.safetensors".to_string(),
            ]);
            let expanded = SelectedArtifactIdentity::from_download_request_at_revision(
                &request,
                Some(expanded_files.clone()),
                &first_revision,
            );
            expanded_files.sort();
            expanded_files.dedup();
            assert_eq!(expanded.selected_filenames, expanded_files);
            assert_eq!(expanded.artifact_id, first.artifact_id);
            assert_eq!(expanded.artifact_digest, first.artifact_digest);
            assert_eq!(expanded.selection_kind, first.selection_kind);
            assert_eq!(expanded.revision, first.revision);
            assert_eq!(
                normalize_artifact_path_slug(&first.artifact_id),
                first.artifact_id
            );
            assert_eq!(
                SelectedArtifactIdentity::from_download_request_at_revision(
                    &request,
                    None,
                    &DownloadRevision::legacy_main(),
                ),
                legacy
            );
        }
    }

    #[test]
    fn pinned_metadata_and_identity_agree_after_file_order_normalization() {
        let mut req = request("Owner/Example");
        req.filenames = Some(vec![
            "b.safetensors".to_string(),
            "a.safetensors".to_string(),
        ]);
        let revision = DownloadRevision::from_commit(&"c".repeat(40)).unwrap();
        let evidence = HuggingFaceEvidence {
            selected_filenames: Some(vec![
                "a.safetensors".to_string(),
                "b.safetensors".to_string(),
            ]),
            ..Default::default()
        };
        let identity =
            SelectedArtifactIdentity::from_download_request_at_revision(&req, None, &revision);
        let mut metadata = ModelMetadata::default();
        apply_download_artifact_metadata_at_revision(
            &mut metadata,
            &req,
            Some(&evidence),
            &revision,
        );
        assert_eq!(
            metadata.upstream_revision.as_deref(),
            Some(revision.as_str())
        );
        assert_eq!(
            metadata.selected_artifact_id.as_deref(),
            Some(identity.artifact_id.as_str())
        );
        assert_eq!(
            metadata.selected_artifact_files,
            Some(identity.selected_filenames)
        );
    }

    #[test]
    fn pinned_ids_distinguish_same_quant_files_and_normalized_filename_collisions() {
        let revision = DownloadRevision::from_commit(&"d".repeat(40)).unwrap();
        for quant in [None, Some("Q4_K_M".to_string())] {
            let mut first = request("Owner/Example");
            first.filename = Some("model.weights.gguf".to_string());
            first.quant = quant.clone();
            let mut second = first.clone();
            second.filename = Some("model_weights.gguf".to_string());
            // Preserve the existing legacy naming behavior while separating pins.
            assert_eq!(
                SelectedArtifactIdentity::from_download_request(&first, None).artifact_id,
                SelectedArtifactIdentity::from_download_request(&second, None).artifact_id,
            );
            let first = SelectedArtifactIdentity::from_download_request_at_revision(
                &first, None, &revision,
            );
            let second = SelectedArtifactIdentity::from_download_request_at_revision(
                &second, None, &revision,
            );
            assert_ne!(first.artifact_digest, second.artifact_digest);
            assert_ne!(first.artifact_id, second.artifact_id);
        }
    }

    #[test]
    fn test_normalize_artifact_path_slug_preserves_identity_separators() {
        assert_eq!(
            normalize_artifact_path_slug("Owner--Qwen3.6-27B-GGUF__Q4_K_M"),
            "owner--qwen3_6-27b-gguf__q4_k_m"
        );
        assert_eq!(
            normalize_artifact_path_slug("Owner/Repo:Name__model-00001-of-00002.safetensors"),
            "ownerreponame__model-00001-of-00002_safetensors"
        );
    }

    #[test]
    fn artifact_id_includes_quant_for_same_repo_variants() {
        let mut q4 = request("Owner/Example-GGUF");
        q4.quant = Some("Q4_K_M".to_string());
        let mut q5 = q4.clone();
        q5.quant = Some("Q5_K_M".to_string());

        let q4_identity = SelectedArtifactIdentity::from_download_request(&q4, None);
        let q5_identity = SelectedArtifactIdentity::from_download_request(&q5, None);

        assert_eq!(q4_identity.artifact_id, "owner--example-gguf__q4_k_m");
        assert_eq!(q5_identity.artifact_id, "owner--example-gguf__q5_k_m");
        assert_ne!(q4_identity.artifact_id, q5_identity.artifact_id);
    }

    #[test]
    fn artifact_id_uses_stable_digest_for_file_groups() {
        let mut req = request("Owner/Multi-File");
        req.filenames = Some(vec![
            "b.safetensors".to_string(),
            "a.safetensors".to_string(),
        ]);

        let first = SelectedArtifactIdentity::from_download_request(&req, None);
        let second = SelectedArtifactIdentity::from_download_request(
            &req,
            Some(vec![
                "a.safetensors".to_string(),
                "b.safetensors".to_string(),
            ]),
        );

        assert_eq!(first.artifact_id, second.artifact_id);
        assert!(first.artifact_id.starts_with("owner--multi-file__files_"));
    }

    #[test]
    fn architecture_family_preserves_version_separators() {
        assert_eq!(normalize_architecture_family("qwen3.5"), "qwen3_5");
        assert_eq!(normalize_architecture_family("qwen3_5"), "qwen3_5");
        assert_eq!(normalize_architecture_family("qwen35"), "qwen3_5");
        assert_eq!(normalize_architecture_family("llama32"), "llama3_2");
        assert_eq!(normalize_architecture_family("gpt2"), "gpt2");
    }

    #[test]
    fn architecture_family_is_inferred_from_model_name_before_publisher() {
        let req = request("DavidAU/Qwen3.6-27B-Heretic-GGUF");

        let family = infer_architecture_family_for_download(&req, None);

        assert_eq!(family, "qwen3_6");
    }

    #[test]
    fn architecture_family_uses_config_model_type_when_no_versioned_signal_exists() {
        let req = request("QuantFactory/Qwen3-Reranker-4B-GGUF");
        let evidence = HuggingFaceEvidence {
            config_model_type: Some("qwen3".to_string()),
            ..Default::default()
        };

        let family = infer_architecture_family_for_download(&req, Some(&evidence));

        assert_eq!(family, "qwen3");
    }

    #[test]
    fn download_artifact_metadata_keeps_repo_and_artifact_identity_separate() {
        let mut req = request("QuantFactory/Qwen3.6-27B-Heretic-GGUF");
        req.family = "qwen3_6".to_string();
        req.filename = Some("Qwen3.6-27B-Heretic-Q4_K_M.gguf".to_string());
        req.quant = Some("Q4_K_M".to_string());
        let evidence = HuggingFaceEvidence {
            config_model_type: Some("qwen3".to_string()),
            ..Default::default()
        };
        let mut metadata = ModelMetadata::default();

        apply_download_artifact_metadata(&mut metadata, &req, Some(&evidence));

        assert_eq!(metadata.repo_id, None);
        assert_eq!(metadata.publisher.as_deref(), Some("QuantFactory"));
        assert_eq!(metadata.architecture_family.as_deref(), Some("qwen3_6"));
        assert_eq!(metadata.config_model_type.as_deref(), Some("qwen3"));
        assert_eq!(metadata.selected_artifact_quant.as_deref(), Some("q4_k_m"));
        assert_eq!(
            metadata.selected_artifact_files.as_deref(),
            Some(&["Qwen3.6-27B-Heretic-Q4_K_M.gguf".to_string()][..])
        );
        assert_eq!(metadata.upstream_revision.as_deref(), Some("main"));
        assert!(metadata
            .selected_artifact_id
            .as_deref()
            .is_some_and(|id| id.contains("__q4_k_m")));
    }
}

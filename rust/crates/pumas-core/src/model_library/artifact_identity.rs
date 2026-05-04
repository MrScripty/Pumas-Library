//! Selected-artifact identity for repository-backed model downloads.
//!
//! A Hugging Face repository can expose several loadable artifacts.  For GGUF
//! repos, the selected artifact is often a single quantized file such as
//! `Q4_K_M` or `Q5_K_M`.  This module keeps that artifact selector separate
//! from the upstream repo id so path planning, progress tracking, and migration
//! can distinguish variants from the same repo.

use crate::model_library::naming::normalize_name;
use crate::model_library::types::DownloadRequest;
use crate::models::HuggingFaceEvidence;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

const DEFAULT_REVISION: &str = "main";
const DIGEST_HEX_LEN: usize = 12;

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
        let mut filenames = selected_filenames.unwrap_or_else(|| requested_filenames(request));
        filenames.sort();
        filenames.dedup();

        let selection_kind = selection_kind(request, &filenames);
        let selected_quant = request.quant.as_ref().map(|value| normalize_name(value));
        let digest = artifact_digest(request, &filenames, &selection_kind);
        let artifact_selector = artifact_selector(request, &filenames, &selection_kind, &digest);
        let artifact_id = format!("{}__{}", repo_slug(&request.repo_id), artifact_selector);

        Self {
            repo_id: request.repo_id.clone(),
            revision: DEFAULT_REVISION.to_string(),
            subfolder: None,
            selection_kind,
            selected_filenames: filenames,
            selected_quant,
            artifact_digest: digest,
            artifact_id,
        }
    }
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

    normalize_architecture_family(&request.family)
}

/// Normalize a family/config token while preserving version separators.
pub fn normalize_architecture_family(value: &str) -> String {
    let normalized = normalize_name(value);
    normalize_compact_version_token(&normalized)
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
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.repo_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(DEFAULT_REVISION.as_bytes());
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
}

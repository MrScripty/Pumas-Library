//! Companion files: the model card, license and notice files, code, and the
//! other non-weight metadata a repository ships next to its weights.
//!
//! Selection is rule based so that new metadata files are kept by default: a
//! repository file is a companion when it is not weight data, is not reserved
//! by Pumas, and lies in the scope of the selected weights. Weight data is
//! derived from the shared weight classification in [`super::sharding`].
//!
//! Pumas stores companion code for provenance and inspection only. Nothing in
//! this module grants permission to import or execute it; loaders keep
//! `trust_remote_code=False` and `local_files_only=True`.

use crate::models::{MetadataArtifactRef, ModelMetadata};
use crate::{PumasError, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

/// Status recorded on a governance artifact whose bytes are stored with the model.
pub(crate) const STORED_ARTIFACT_STATUS: &str = "stored";

/// Hugging Face's documented directory for custom generation code. It is code
/// for the model as a whole, so it is in scope whenever it sits at the root.
const CUSTOM_GENERATE_DIR: &str = "custom_generate";

/// Root files that Pumas owns inside every model directory.
const RESERVED_ROOT_FILENAMES: &[&str] = &["metadata.json", "overrides.json"];

/// Lower-case filename prefixes that identify license, notice and usage-policy
/// documents, whatever their extension.
const LICENSE_PREFIXES: &[&str] = &[
    "license",
    "licence",
    "notice",
    "use_policy",
    "use-policy",
    "usepolicy",
    "acceptable_use",
    "acceptable-use",
    "copying",
    "copyright",
];

/// What a companion file is, for metadata recording and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CompanionKind {
    ModelCard,
    License,
    Code,
    Metadata,
}

impl CompanionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ModelCard => "model_card",
            Self::License => "license",
            Self::Code => "code",
            Self::Metadata => "metadata",
        }
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

/// Whether a repository path holds weight data: a weight file or a weight index.
pub(crate) fn is_weight_data(path: &str) -> bool {
    super::sharding::is_weight_file(path) || super::package_selection::is_weight_index(path)
}

/// Whether Pumas reserves this repository-relative path inside a model
/// directory: its own root metadata files and any hidden path component.
pub(crate) fn is_reserved_model_path(path: &str) -> bool {
    path.split('/').any(|component| component.starts_with('.'))
        || (!path.contains('/') && RESERVED_ROOT_FILENAMES.contains(&path))
}

/// Whether a repository file can be kept as a companion at all. Scope is
/// checked separately. `.gitattributes` and other dotfiles are repository
/// plumbing, and reserved names would collide with Pumas's own files.
pub(crate) fn is_companion_candidate(path: &str) -> bool {
    !path.is_empty() && !is_weight_data(path) && !is_reserved_model_path(path)
}

/// Classify a companion file by name.
pub(crate) fn classify(path: &str) -> CompanionKind {
    let name = basename(path).to_ascii_lowercase();
    let stem = name.split('.').next().unwrap_or(&name);
    if stem == "readme" {
        CompanionKind::ModelCard
    } else if LICENSE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        CompanionKind::License
    } else if name.ends_with(".py") {
        CompanionKind::Code
    } else {
        CompanionKind::Metadata
    }
}

/// The directories whose companions travel with a selected set of weights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanionScope {
    whole_repository: bool,
    directories: BTreeSet<String>,
}

impl CompanionScope {
    /// Every directory is in scope: the whole repository was selected.
    pub(crate) fn whole_repository() -> Self {
        Self {
            whole_repository: true,
            directories: BTreeSet::new(),
        }
    }

    /// The root plus every directory that holds a selected weight file.
    pub(crate) fn for_selected<'a>(selected: impl IntoIterator<Item = &'a str>) -> Self {
        let mut directories = BTreeSet::from([String::new()]);
        directories.extend(
            selected
                .into_iter()
                .filter(|path| is_weight_data(path))
                .map(|path| parent_dir(path).to_string()),
        );
        Self {
            whole_repository: false,
            directories,
        }
    }

    pub(crate) fn contains(&self, path: &str) -> bool {
        if self.whole_repository {
            return true;
        }
        let directory = parent_dir(path);
        self.directories.contains(directory)
            || directory == CUSTOM_GENERATE_DIR
            || directory.starts_with("custom_generate/")
    }
}

/// Whether `path` is a sidecar of `weight`: a file in the same directory whose
/// name extends the weight's filename, such as `model.onnx_data` for
/// `model.onnx`. A sidecar is part of that weight variant.
fn is_sidecar_of(path: &str, weight: &str) -> bool {
    if parent_dir(path) != parent_dir(weight) || path == weight {
        return false;
    }
    let name = basename(path);
    let weight_name = basename(weight);
    name.strip_prefix(weight_name)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|separator| matches!(separator, '.' | '_' | '-'))
}

/// Whether a candidate belongs to a weight variant that was not selected.
/// Such files are never kept: they are another variant's data, not metadata.
pub(crate) fn belongs_to_unselected_weight(
    path: &str,
    selected: &BTreeSet<&str>,
    weights: &[&str],
) -> bool {
    let owners = weights
        .iter()
        .filter(|weight| is_sidecar_of(path, weight))
        .collect::<Vec<_>>();
    !owners.is_empty() && !owners.iter().any(|weight| selected.contains(**weight))
}

/// Whether `value` is a Git blob object id as published by the Hub.
pub(crate) fn is_git_blob_oid(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Compute the Git blob object id of an open file: SHA-1 over `blob {len}\0`
/// and the bytes. This is the content hash the Hub publishes for non-LFS
/// files. Blocking.
pub(crate) fn git_blob_oid(mut file: std::fs::File) -> Result<String> {
    use sha1::Digest as _;
    let length = file.metadata()?.len();
    let mut hasher = sha1::Sha1::new();
    hasher.update(format!("blob {length}\0").as_bytes());
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut read_total = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        read_total += read as u64;
        hasher.update(&buffer[..read]);
    }
    if read_total != length {
        return Err(PumasError::Validation {
            field: "companion_files.integrity".into(),
            message: "A companion file changed while it was hashed".into(),
        });
    }
    Ok(hex::encode(hasher.finalize()))
}

fn sha256_of_file(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).map_err(|error| PumasError::io_with_path(error, path))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|error| PumasError::io_with_path(error, path))?;
    Ok(hex::encode(hasher.finalize()))
}

/// One companion file stored in a model directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredCompanion {
    /// Path relative to the model directory, `/`-separated.
    pub path: String,
    pub kind: CompanionKind,
    pub size_bytes: u64,
}

/// Companion files found in a model directory, and the code its
/// `config.json` `auto_map` names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CompanionInventory {
    pub files: Vec<StoredCompanion>,
    /// Module files named by `auto_map`, relative to the model directory.
    pub auto_map_modules: Vec<String>,
    /// `auto_map` references into other repositories (`org/repo--module.Class`).
    pub external_auto_map_references: Vec<String>,
}

impl CompanionInventory {
    pub(crate) fn of_kind(&self, kind: CompanionKind) -> impl Iterator<Item = &StoredCompanion> {
        self.files.iter().filter(move |file| file.kind == kind)
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, path: &str) -> bool {
        self.files.iter().any(|file| file.path == path)
    }

    /// The primary model card: the root `README.md`, else the shallowest one.
    pub(crate) fn model_card(&self) -> Option<&StoredCompanion> {
        self.of_kind(CompanionKind::ModelCard).min_by_key(|file| {
            (
                file.path.matches('/').count(),
                !is_markdown(&file.path),
                file.path.clone(),
            )
        })
    }

    /// The primary license: a root `LICENSE`/`LICENCE` document before
    /// notices and usage policies, then the shallowest path.
    pub(crate) fn license(&self) -> Option<&StoredCompanion> {
        self.of_kind(CompanionKind::License).min_by_key(|file| {
            let name = basename(&file.path).to_ascii_lowercase();
            let is_license = name.starts_with("license") || name.starts_with("licence");
            (
                file.path.matches('/').count(),
                !is_license,
                file.path.clone(),
            )
        })
    }
}

fn is_markdown(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".md")
}

/// Scan a model directory for stored companion files. Blocking.
pub(crate) fn scan_companions(model_dir: &Path) -> Result<CompanionInventory> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(model_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        })
    {
        let entry = entry.map_err(|error| {
            PumasError::Other(format!("Failed to scan model directory: {error}"))
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(model_dir) else {
            continue;
        };
        let Some(relative) = relative
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()
            .map(|components| components.join("/"))
        else {
            continue;
        };
        if !is_companion_candidate(&relative) {
            continue;
        }
        let size_bytes = entry
            .metadata()
            .map_err(|error| PumasError::Other(format!("Failed to stat {relative}: {error}")))?
            .len();
        files.push(StoredCompanion {
            kind: classify(&relative),
            path: relative,
            size_bytes,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let (auto_map_modules, external_auto_map_references) = auto_map_code(model_dir)?;
    Ok(CompanionInventory {
        files,
        auto_map_modules,
        external_auto_map_references,
    })
}

/// Module files and external references named by `auto_map` in the root
/// `config.json`. A reference is `module.Class` (resolved to `module.py` next
/// to the config) or `org/repo--module.Class` (code in another repository).
fn auto_map_code(model_dir: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let config_path = model_dir.join("config.json");
    let raw = match std::fs::read(&config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new()))
        }
        Err(error) => return Err(PumasError::io_with_path(error, &config_path)),
    };
    // A config that is not JSON names no code; package facts report it.
    let Ok(config) = serde_json::from_slice::<Value>(&raw) else {
        return Ok((Vec::new(), Vec::new()));
    };
    Ok(auto_map_references(&config))
}

pub(crate) fn auto_map_references(config: &Value) -> (Vec<String>, Vec<String>) {
    let mut modules = BTreeSet::new();
    let mut external = BTreeSet::new();
    let Some(auto_map) = config.get("auto_map").and_then(Value::as_object) else {
        return (Vec::new(), Vec::new());
    };
    let references = auto_map.values().flat_map(|value| match value {
        Value::String(reference) => vec![reference.as_str()],
        Value::Array(values) => values.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    });
    for reference in references {
        if reference.contains("--") {
            external.insert(reference.to_string());
            continue;
        }
        let Some((module, _class)) = reference.rsplit_once('.') else {
            continue;
        };
        let module = module.replace('.', "/");
        if !module.is_empty()
            && !module
                .split('/')
                .any(|part| part.is_empty() || part == "..")
        {
            modules.insert(format!("{module}.py"));
        }
    }
    (
        modules.into_iter().collect(),
        external.into_iter().collect(),
    )
}

/// Where stored companions came from, for their recorded source URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanionSource {
    pub repo_id: String,
    pub revision: String,
}

impl CompanionSource {
    fn blob_url(&self, path: &str) -> String {
        format!(
            "{}/{}/blob/{}/{}",
            super::hf::HF_HUB_BASE,
            self.repo_id,
            self.revision,
            path
        )
    }
}

fn artifact_ref(
    model_dir: &Path,
    file: &StoredCompanion,
    source: Option<&CompanionSource>,
    previous: Option<&MetadataArtifactRef>,
) -> Result<MetadataArtifactRef> {
    let source_url = source
        .map(|source| source.blob_url(&file.path))
        .or_else(|| {
            previous
                .filter(|previous| previous.path.as_deref() == Some(file.path.as_str()))
                .and_then(|previous| previous.source_url.clone())
        });
    Ok(MetadataArtifactRef {
        path: Some(file.path.clone()),
        source_url,
        sha256: Some(sha256_of_file(&model_dir.join(&file.path))?),
        status: Some(STORED_ARTIFACT_STATUS.to_string()),
    })
}

fn artifact_refs_equal(left: Option<&MetadataArtifactRef>, right: &MetadataArtifactRef) -> bool {
    left.is_some_and(|left| {
        left.path == right.path
            && left.source_url == right.source_url
            && left.sha256 == right.sha256
            && left.status == right.status
    })
}

/// Record stored companions in model metadata. Blocking (hashes the card and
/// license). Returns whether the metadata changed.
///
/// The model card and primary license become `model_card_artifact` and
/// `license_artifact` with their SHA-256. Stored code and the code that
/// `auto_map` names are merged into `custom_code_sources`; `auto_map` sets
/// `requires_custom_code`. Existing sources are never removed, and a missing
/// card or license leaves an existing record untouched. Identity fields are
/// never read or written here.
pub(crate) fn record_companions(
    model_dir: &Path,
    metadata: &mut ModelMetadata,
    inventory: &CompanionInventory,
    source: Option<&CompanionSource>,
) -> Result<bool> {
    let mut changed = false;
    if let Some(card) = inventory.model_card() {
        let next = artifact_ref(
            model_dir,
            card,
            source,
            metadata.model_card_artifact.as_ref(),
        )?;
        if !artifact_refs_equal(metadata.model_card_artifact.as_ref(), &next) {
            metadata.model_card_artifact = Some(next);
            changed = true;
        }
    }
    if let Some(license) = inventory.license() {
        let next = artifact_ref(
            model_dir,
            license,
            source,
            metadata.license_artifact.as_ref(),
        )?;
        if !artifact_refs_equal(metadata.license_artifact.as_ref(), &next) {
            metadata.license_artifact = Some(next);
            changed = true;
        }
    }

    let mut sources = metadata
        .custom_code_sources
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>();
    sources.extend(
        inventory
            .of_kind(CompanionKind::Code)
            .map(|file| file.path.clone()),
    );
    sources.extend(inventory.auto_map_modules.iter().cloned());
    sources.extend(inventory.external_auto_map_references.iter().cloned());
    let next_sources = (!sources.is_empty()).then(|| sources.into_iter().collect::<Vec<_>>());
    if next_sources.is_some() && metadata.custom_code_sources != next_sources {
        metadata.custom_code_sources = next_sources;
        changed = true;
    }
    let needs_code = !inventory.auto_map_modules.is_empty()
        || !inventory.external_auto_map_references.is_empty();
    if needs_code && metadata.requires_custom_code != Some(true) {
        metadata.requires_custom_code = Some(true);
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_cards_licenses_code_and_metadata_by_rule() {
        assert_eq!(classify("README.md"), CompanionKind::ModelCard);
        assert_eq!(classify("onnx/readme.md"), CompanionKind::ModelCard);
        for license in [
            "LICENSE",
            "LICENSE.txt",
            "license.md",
            "LICENCE",
            "NOTICE",
            "NOTICE.md",
            "USE_POLICY.md",
            "COPYING",
            "LICENSE-APACHE",
        ] {
            assert_eq!(classify(license), CompanionKind::License, "{license}");
        }
        assert_eq!(classify("modeling_trado.py"), CompanionKind::Code);
        for metadata in [
            "config.json",
            "tokenizer.json",
            "chat_template.jinja",
            "params.yaml",
            "vocab.txt",
            "spiece.model",
            "CHANGELOG.md",
            "brand-new-metadata.toml",
        ] {
            assert_eq!(classify(metadata), CompanionKind::Metadata, "{metadata}");
        }
    }

    #[test]
    fn weights_dotfiles_and_reserved_names_are_never_companions() {
        for path in [
            "model.safetensors",
            "model-00001-of-00002.safetensors",
            "model.safetensors.index.json",
            "pytorch_model.bin.index.json",
            "weights.Q4_K_M.gguf",
            "onnx/model.onnx",
            ".gitattributes",
            "sub/.hidden",
            "metadata.json",
            "overrides.json",
        ] {
            assert!(!is_companion_candidate(path), "{path}");
        }
        assert!(is_companion_candidate("onnx/metadata.json"));
        assert!(is_companion_candidate("README.md"));
    }

    #[test]
    fn scope_is_the_root_and_selected_weight_directories() {
        let scope = CompanionScope::for_selected(["onnx/model_q4.onnx", "onnx/tokenizer.json"]);
        assert!(scope.contains("README.md"));
        assert!(scope.contains("onnx/README.md"));
        assert!(scope.contains("custom_generate/generate.py"));
        assert!(!scope.contains("other/README.md"));
        assert!(CompanionScope::whole_repository().contains("other/README.md"));
    }

    #[test]
    fn sidecars_follow_their_weight_variant() {
        let weights = ["onnx/model.onnx", "onnx/model_fp16.onnx"];
        let selected = BTreeSet::from(["onnx/model.onnx"]);
        assert!(!belongs_to_unselected_weight(
            "onnx/model.onnx_data",
            &selected,
            &weights
        ));
        assert!(belongs_to_unselected_weight(
            "onnx/model_fp16.onnx_data",
            &selected,
            &weights
        ));
        assert!(!belongs_to_unselected_weight(
            "onnx/config.json",
            &selected,
            &weights
        ));
    }

    #[test]
    fn git_blob_oid_matches_git() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        std::fs::write(&path, b"hello\n").unwrap();
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(
            git_blob_oid(std::fs::File::open(&path).unwrap()).unwrap(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn auto_map_names_local_modules_and_external_repositories() {
        let config = serde_json::json!({
            "auto_map": {
                "AutoConfig": "configuration_trado.TradoConfig",
                "AutoModelForCausalLM": "modeling_trado.TradoForCausalLM",
                "AutoTokenizer": ["tokenization_x.XTokenizer", null],
                "AutoModel": "other/repo--modeling_y.Y"
            }
        });
        let (modules, external) = auto_map_references(&config);
        assert_eq!(
            modules,
            [
                "configuration_trado.py",
                "modeling_trado.py",
                "tokenization_x.py"
            ]
        );
        assert_eq!(external, ["other/repo--modeling_y.Y"]);
    }

    #[test]
    fn records_card_license_and_code_without_touching_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("README.md"), "# Card\n").unwrap();
        std::fs::write(root.join("LICENSE"), "Apache\n").unwrap();
        std::fs::write(root.join("NOTICE"), "Notice\n").unwrap();
        std::fs::write(root.join("modeling_trado.py"), "class X: pass\n").unwrap();
        std::fs::write(
            root.join("config.json"),
            r#"{"auto_map":{"AutoModel":"modeling_trado.X"}}"#,
        )
        .unwrap();
        std::fs::write(root.join("model.safetensors"), b"weights").unwrap();
        std::fs::write(root.join("metadata.json"), b"{}").unwrap();
        let inventory = scan_companions(root).unwrap();
        assert!(!inventory.contains("model.safetensors"));
        assert!(!inventory.contains("metadata.json"));

        let mut metadata = ModelMetadata {
            selected_artifact_id: Some("acme--model__files_abc".into()),
            ..Default::default()
        };
        let source = CompanionSource {
            repo_id: "acme/model".into(),
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
        };
        assert!(record_companions(root, &mut metadata, &inventory, Some(&source)).unwrap());
        let card = metadata.model_card_artifact.clone().unwrap();
        assert_eq!(card.path.as_deref(), Some("README.md"));
        assert_eq!(card.status.as_deref(), Some(STORED_ARTIFACT_STATUS));
        assert_eq!(
            card.source_url.as_deref(),
            Some("https://huggingface.co/acme/model/blob/0123456789abcdef0123456789abcdef01234567/README.md")
        );
        assert_eq!(
            card.sha256.as_deref(),
            Some(hex::encode(Sha256::digest(b"# Card\n")).as_str())
        );
        assert_eq!(
            metadata.license_artifact.unwrap().path.as_deref(),
            Some("LICENSE")
        );
        assert_eq!(metadata.custom_code_sources.unwrap(), ["modeling_trado.py"]);
        assert_eq!(metadata.requires_custom_code, Some(true));
        assert_eq!(
            metadata.selected_artifact_id.as_deref(),
            Some("acme--model__files_abc")
        );
    }
}

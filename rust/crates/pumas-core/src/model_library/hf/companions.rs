//! Companion-file selection for Hugging Face downloads.
//!
//! The selected weights decide the scope; [`companion_files`] decides which
//! repository files in that scope are companions. The same selection plans a
//! download, sizes it before it starts, and backfills existing models.
//!
//! [`companion_files`]: crate::model_library::companion_files

use super::types::FileToDownload;
use crate::model_library::companion_files::{
    belongs_to_unselected_weight, is_companion_candidate, is_weight_data, CompanionScope,
};
use crate::model_library::types::RepoFileTree;
use std::collections::BTreeSet;

/// Pipeline configs that explicit-file downloads keep from any directory.
const PIPELINE_CONFIG_FILENAMES: &[&str] = &["model_index.json", "scheduler_config.json"];

/// How a request chose its weights, which decides the companion scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompanionMode {
    /// Explicit files or a bundle: scoped companions, plus the non-weight LFS
    /// files and pipeline configs that explicit selections kept from anywhere.
    ExplicitFiles,
    /// A quant or a single file: companions in the root and weight directories.
    Scoped,
    /// No selector: every weight is selected, so the whole repository is in scope.
    WholeRepository,
}

impl CompanionMode {
    pub(crate) fn for_request(request: &crate::model_library::types::DownloadRequest) -> Self {
        if request.filenames.is_some()
            || request.bundle_format == Some(crate::models::BundleFormat::DiffusersDirectory)
        {
            Self::ExplicitFiles
        } else if request.filename.is_some() || request.quant.is_some() {
            Self::Scoped
        } else {
            Self::WholeRepository
        }
    }
}

fn regular(path: &str) -> FileToDownload {
    FileToDownload {
        filename: path.to_string(),
        size: None,
        sha256: None,
    }
}

/// Select the non-weight files that accompany `payload`: its model card,
/// license and notice files, code, and other metadata. Files that belong to
/// an unselected weight variant are never selected. Payload files are not
/// repeated. Regular files keep unknown size here so download verification is
/// unchanged; [`companion_size_bytes`] sizes them from the tree.
pub(crate) fn select_companion_files(
    tree: &RepoFileTree,
    payload: &[FileToDownload],
    mode: CompanionMode,
) -> Vec<FileToDownload> {
    let selected = payload
        .iter()
        .map(|file| file.filename.as_str())
        .collect::<BTreeSet<_>>();
    let weights = tree
        .lfs_files
        .iter()
        .map(|file| file.filename.as_str())
        .chain(tree.regular_files.iter().map(String::as_str))
        .filter(|path| is_weight_data(path))
        .collect::<Vec<_>>();
    let scope = match mode {
        CompanionMode::WholeRepository => CompanionScope::whole_repository(),
        CompanionMode::ExplicitFiles | CompanionMode::Scoped => {
            CompanionScope::for_selected(selected.iter().copied())
        }
    };
    let keeps = |path: &str, anywhere: bool| {
        !selected.contains(path)
            && is_companion_candidate(path)
            && (anywhere || scope.contains(path))
            && !belongs_to_unselected_weight(path, &selected, &weights)
    };

    let mut companions = Vec::new();
    let mut seen = BTreeSet::new();
    for path in &tree.regular_files {
        let pipeline_config = mode == CompanionMode::ExplicitFiles
            && PIPELINE_CONFIG_FILENAMES.contains(&path.rsplit('/').next().unwrap_or(path));
        if keeps(path, pipeline_config) && seen.insert(path.as_str()) {
            companions.push(regular(path));
        }
    }
    for file in &tree.lfs_files {
        let anywhere = mode == CompanionMode::ExplicitFiles;
        if keeps(&file.filename, anywhere) && seen.insert(file.filename.as_str()) {
            companions.push(FileToDownload {
                filename: file.filename.clone(),
                size: Some(file.size),
                sha256: Some(file.sha256.clone()),
            });
        }
    }
    companions
}

/// Total bytes of `files`, using the tree's published size for regular files.
/// `None` when any size is unknown, so a partial sum is never shown as a total.
pub(crate) fn companion_size_bytes(tree: &RepoFileTree, files: &[FileToDownload]) -> Option<u64> {
    files.iter().try_fold(0_u64, |total, file| {
        let size = file.size.or_else(|| {
            tree.regular_file_details
                .iter()
                .find(|detail| detail.filename == file.filename)
                .map(|detail| detail.size)
        })?;
        total.checked_add(size)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::types::{LfsFileInfo, RegularFileInfo};

    fn lfs(filename: &str, size: u64) -> LfsFileInfo {
        LfsFileInfo {
            filename: filename.into(),
            size,
            sha256: "a".repeat(64),
        }
    }

    fn tree(regular: &[&str], lfs_files: Vec<LfsFileInfo>) -> RepoFileTree {
        RepoFileTree {
            repo_id: "acme/model".into(),
            lfs_files,
            regular_files: regular.iter().map(|path| path.to_string()).collect(),
            regular_file_details: regular
                .iter()
                .map(|path| RegularFileInfo {
                    filename: path.to_string(),
                    size: 10,
                    git_oid: "b".repeat(40),
                })
                .collect(),
            cached_at: String::new(),
            last_modified: None,
            cache_version: crate::model_library::types::REPO_FILE_TREE_VERSION,
        }
    }

    fn names(files: &[FileToDownload]) -> Vec<&str> {
        let mut names = files
            .iter()
            .map(|file| file.filename.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    fn payload(paths: &[&str]) -> Vec<FileToDownload> {
        paths.iter().map(|path| regular(path)).collect()
    }

    #[test]
    fn keeps_card_license_code_and_metadata_but_not_weights_or_plumbing() {
        let tree = tree(
            &[
                ".gitattributes",
                "README.md",
                "LICENSE",
                "NOTICE.txt",
                "USE_POLICY.md",
                "config.json",
                "configuration_trado.py",
                "modeling_trado.py",
                "tokenizer.json",
                "chat_template.jinja",
                "params.yaml",
                "brand-new-metadata.toml",
                "metadata.json",
            ],
            vec![lfs("model.safetensors", 100)],
        );
        let selected = select_companion_files(
            &tree,
            &payload(&["model.safetensors"]),
            CompanionMode::ExplicitFiles,
        );
        assert_eq!(
            names(&selected),
            [
                "LICENSE",
                "NOTICE.txt",
                "README.md",
                "USE_POLICY.md",
                "brand-new-metadata.toml",
                "chat_template.jinja",
                "config.json",
                "configuration_trado.py",
                "modeling_trado.py",
                "params.yaml",
                "tokenizer.json",
            ]
        );
        assert_eq!(companion_size_bytes(&tree, &selected), Some(110));
    }

    #[test]
    fn subfolder_selection_keeps_root_and_selected_folder_only() {
        let tree = tree(
            &[
                "README.md",
                "LICENSE",
                "onnx/README.md",
                "onnx/config.json",
                "other/README.md",
                "other/config.json",
                "custom_generate/generate.py",
            ],
            vec![
                lfs("onnx/model_q4.onnx", 100),
                lfs("onnx/model_fp16.onnx", 200),
                lfs("onnx/model_q4.onnx_data", 50),
                lfs("onnx/model_fp16.onnx_data", 500),
                lfs("other/model.safetensors", 300),
            ],
        );
        let selected = select_companion_files(
            &tree,
            &payload(&["onnx/model_q4.onnx"]),
            CompanionMode::Scoped,
        );
        assert_eq!(
            names(&selected),
            [
                "LICENSE",
                "README.md",
                "custom_generate/generate.py",
                "onnx/README.md",
                "onnx/config.json",
                "onnx/model_q4.onnx_data",
            ]
        );
    }

    #[test]
    fn multi_quant_gguf_keeps_only_the_chosen_quant_with_card_and_license() {
        let tree = tree(
            &["README.md", "LICENSE.md", ".gitattributes"],
            vec![
                lfs("model-Q4_K_M.gguf", 400),
                lfs("model-Q8_0.gguf", 800),
                lfs("mmproj-model-f16.gguf", 90),
            ],
        );
        let selected = select_companion_files(
            &tree,
            &payload(&["model-Q4_K_M.gguf"]),
            CompanionMode::Scoped,
        );
        assert_eq!(names(&selected), ["LICENSE.md", "README.md"]);
    }

    #[test]
    fn whole_repository_selection_keeps_companions_in_every_folder() {
        let tree = tree(
            &["README.md", "1_Pooling/config.json", "modules.json"],
            vec![lfs("model.safetensors", 100)],
        );
        let selected = select_companion_files(
            &tree,
            &payload(&["model.safetensors"]),
            CompanionMode::WholeRepository,
        );
        assert_eq!(
            names(&selected),
            ["1_Pooling/config.json", "README.md", "modules.json"]
        );
    }

    #[test]
    fn unknown_regular_sizes_leave_the_companion_total_unknown() {
        let mut tree = tree(&["README.md"], Vec::new());
        tree.regular_file_details.clear();
        assert_eq!(companion_size_bytes(&tree, &payload(&["README.md"])), None);
    }
}

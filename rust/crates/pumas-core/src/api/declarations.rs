//! Resolve and record a model's sentence-transformers declarations (ADR 0007).
//!
//! Lineage order: the model's own package, then its source repository, then
//! its single recorded base model. A caller may name the repository instead.
//! The first repository that publishes a sentence-transformers package wins;
//! its files are read at one immutable commit outside the library and only
//! the validated record is kept, in the model's metadata.
//!
//! Completed downloads are resolved automatically by a reactor that observes
//! download snapshots; it never replaces a record that already exists.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::cancel::CancellationToken;
use crate::model_library::declarations::{
    self, DeclarationSource, SentenceTransformersDeclarations,
};
use crate::model_library::{HuggingFaceClient, ModelLibrary};
use crate::models::{DownloadStatus, ModelMetadata};
use crate::{PumasApi, PumasError, Result};

/// Outcome of resolving a model's declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeclarationResolution {
    /// Declarations were read and recorded in the model's metadata.
    Resolved {
        declarations: Box<SentenceTransformersDeclarations>,
    },
    /// Neither the package nor any searched repository publishes them.
    NotDeclared { searched: Vec<String> },
}

impl PumasApi {
    /// Resolve the model's sentence-transformers declarations and record them
    /// in its metadata, replacing an earlier record.
    pub async fn resolve_model_declarations(
        &self,
        model_id: &str,
        repo_id: Option<&str>,
    ) -> Result<DeclarationResolution> {
        self.try_primary()?;
        let primary = self.primary();
        resolve(
            &primary.model_library,
            primary.hf_client.as_deref(),
            &scratch_root(primary),
            model_id,
            repo_id,
        )
        .await
    }

    /// Whether the model's recorded declarations include a `LogitScore`.
    pub(super) async fn has_logit_score_declaration(&self, model_id: &str) -> Result<bool> {
        let record = self
            .primary()
            .model_library
            .get_model(model_id)
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        Ok(record
            .metadata
            .get("sentence_transformers")
            .and_then(|declared| declared.get("logit_score"))
            .is_some_and(|score| !score.is_null()))
    }
}

pub(super) fn scratch_root(primary: &super::PrimaryState) -> PathBuf {
    super::state::launcher_root_from_primary(primary)
        .join("launcher-data")
        .join("tmp")
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| PumasError::Other(format!("declaration worker failed: {error}")))?
}

/// Resolve and record one model's declarations.
pub(super) async fn resolve(
    library: &Arc<ModelLibrary>,
    client: Option<&HuggingFaceClient>,
    scratch: &Path,
    model_id: &str,
    repo_id: Option<&str>,
) -> Result<DeclarationResolution> {
    if library.get_model(model_id).await?.is_none() {
        return Err(PumasError::ModelNotFound {
            model_id: model_id.to_string(),
        });
    }
    let model_dir = library.library_root().join(model_id);
    let (metadata, has_package) = {
        let library = library.clone();
        let dir = model_dir.clone();
        blocking(move || {
            Ok((
                library.load_metadata(&dir)?.unwrap_or_default(),
                declarations::has_package(&dir),
            ))
        })
        .await?
    };

    let mut searched = Vec::new();
    let resolved = if repo_id.is_none() && has_package {
        let dir = model_dir.clone();
        Some(blocking(move || declarations::read_package(&dir, DeclarationSource::Package)).await?)
    } else {
        let candidates = lineage(repo_id, &metadata);
        if candidates.is_empty() {
            None
        } else {
            let client = client.ok_or_else(|| PumasError::InvalidParams {
                message: format!(
                    "Declarations must be read from {}, but Hugging Face access is disabled for \
                     this library instance",
                    candidates.join(", ")
                ),
            })?;
            tokio::fs::create_dir_all(scratch)
                .await
                .map_err(|error| PumasError::io("creating declaration scratch", scratch, error))?;
            let mut found = None;
            for repo in candidates {
                searched.push(repo.clone());
                let scratch = scratch.to_path_buf();
                let work = blocking(move || {
                    tempfile::Builder::new()
                        .prefix("pumas-declarations-")
                        .tempdir_in(&scratch)
                        .map_err(|error| {
                            PumasError::io("allocating declaration scratch", &scratch, error)
                        })
                })
                .await?;
                let Some(revision) = client
                    .fetch_sentence_transformers_package(
                        &repo,
                        work.path(),
                        &CancellationToken::new(),
                    )
                    .await?
                else {
                    continue;
                };
                found = Some(
                    blocking(move || {
                        let declared = declarations::read_package(
                            work.path(),
                            DeclarationSource::Repository {
                                repo_id: repo,
                                revision,
                            },
                        );
                        drop(work);
                        declared
                    })
                    .await?,
                );
                break;
            }
            found
        }
    };

    let Some(declared) = resolved else {
        return Ok(DeclarationResolution::NotDeclared { searched });
    };
    let mut metadata = metadata;
    metadata.sentence_transformers = Some(declared.clone());
    library.save_metadata(&model_dir, &metadata).await?;
    library.index_model_dir(&model_dir).await?;
    Ok(DeclarationResolution::Resolved {
        declarations: Box::new(declared),
    })
}

/// Repositories to search, in order, without duplicates.
fn lineage(requested: Option<&str>, metadata: &ModelMetadata) -> Vec<String> {
    if let Some(repo) = requested {
        return vec![repo.trim().to_string()];
    }
    let mut repos = Vec::new();
    let mut push = |repo: &str| {
        let repo = repo.trim();
        if !repo.is_empty() && !repos.iter().any(|known: &String| known == repo) {
            repos.push(repo.to_string());
        }
    };
    if let Some(repo) = metadata.repo_id.as_deref() {
        push(repo);
    }
    if let Some(base) = single_base_model(metadata) {
        push(&base);
    }
    repos
}

/// The model's one base model: the recorded `base_model`, else the model
/// card's (a string or a one-entry list). Several base models are ambiguous.
fn single_base_model(metadata: &ModelMetadata) -> Option<String> {
    if let Some(recorded) = metadata.base_model.as_deref() {
        return match recorded {
            [base] => Some(base.clone()),
            _ => None,
        };
    }
    match metadata.model_card.as_ref()?.get("base_model")? {
        serde_json::Value::String(base) => Some(base.clone()),
        serde_json::Value::Array(bases) => match bases.as_slice() {
            [serde_json::Value::String(base)] => Some(base.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Resolve declarations for each newly completed download that has none.
/// Runs until the download channel closes; failures are logged per model and
/// never affect the download.
pub(super) async fn react_to_downloads(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    scratch: PathBuf,
) {
    let mut updates = client.subscribe_download_updates();
    let mut seen = HashSet::new();
    loop {
        let snapshot = match updates.recv().await {
            Ok(update) => update.snapshot,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        };
        for download in snapshot.downloads {
            if download.status != DownloadStatus::Completed || !seen.insert(download.download_id) {
                continue;
            }
            let Some(model_id) = download.library_model_id else {
                continue;
            };
            match library.get_model(&model_id).await {
                Ok(Some(record)) if record.metadata.get("sentence_transformers").is_none() => {}
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!("Declarations for {model_id} not checked: {error}");
                    continue;
                }
            }
            match resolve(&library, Some(&client), &scratch, &model_id, None).await {
                Ok(DeclarationResolution::Resolved { .. }) => {
                    tracing::info!("Recorded sentence-transformers declarations for {model_id}");
                }
                Ok(DeclarationResolution::NotDeclared { .. }) => {}
                Err(error) => {
                    tracing::warn!("Declarations for {model_id} not resolved: {error}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_searches_the_source_then_its_single_base_model() {
        let metadata = ModelMetadata {
            repo_id: Some("mradermacher/zerank-2-GGUF".into()),
            base_model: Some(vec!["zeroentropy/zerank-2-reranker".into()]),
            ..Default::default()
        };
        assert_eq!(
            lineage(None, &metadata),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(lineage(Some(" acme/x "), &metadata), ["acme/x"]);
        let several = ModelMetadata {
            base_model: Some(vec!["a/b".into(), "c/d".into()]),
            ..Default::default()
        };
        assert!(lineage(None, &several).is_empty());
        let same = ModelMetadata {
            repo_id: Some("a/b".into()),
            base_model: Some(vec!["a/b".into()]),
            ..Default::default()
        };
        assert_eq!(lineage(None, &same), ["a/b"]);
    }

    #[test]
    fn the_model_card_names_the_base_model_when_none_is_recorded() {
        let card = |value: serde_json::Value| ModelMetadata {
            repo_id: Some("mradermacher/zerank-2-GGUF".into()),
            model_card: Some([("base_model".to_string(), value)].into_iter().collect()),
            ..Default::default()
        };
        assert_eq!(
            lineage(
                None,
                &card(serde_json::json!("zeroentropy/zerank-2-reranker"))
            ),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(
            lineage(
                None,
                &card(serde_json::json!(["zeroentropy/zerank-2-reranker"]))
            ),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(
            lineage(None, &card(serde_json::json!(["a/b", "c/d"]))),
            ["mradermacher/zerank-2-GGUF"]
        );
    }
}

//! Resolve and record a model's sentence-transformers declarations (ADR 0007).
//!
//! Lineage order: the model's own package, then its source repository, then
//! its single recorded base model. A caller may name the repository instead.
//! The first repository that publishes a sentence-transformers package wins;
//! its files are read at one immutable commit outside the library and only
//! the validated record is kept, in the model's metadata.
//!
//! Models that become complete are resolved automatically by a reactor that
//! follows the library's update feed; it never replaces a record that already
//! exists.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::cancel::CancellationToken;
use crate::model_library::declarations::{
    self, DeclarationSource, SentenceTransformersDeclarations,
};
use crate::model_library::{HuggingFaceClient, ModelLibrary};
use crate::models::ModelMetadata;
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
            &CancellationToken::new(),
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
    scratch_under(&super::state::launcher_root_from_primary(primary))
}

/// Where declaration files are read, outside the library.
pub(super) fn scratch_under(launcher_root: &Path) -> PathBuf {
    launcher_root.join("launcher-data").join("tmp")
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| PumasError::Other(format!("declaration worker failed: {error}")))?
}

fn package_file_missing(error: &PumasError) -> bool {
    matches!(error, PumasError::Io { source: Some(source), .. }
        if source.kind() == std::io::ErrorKind::NotFound)
}

/// Resolve and record one model's declarations: from its own package, else
/// from the first lineage repository that publishes them. A repository that
/// fails to read or parse does not end the search; when no candidate
/// declares them, the failures are returned with their reasons. Only the
/// model's `sentence_transformers` record is written.
pub(super) async fn resolve(
    library: &Arc<ModelLibrary>,
    client: Option<&HuggingFaceClient>,
    scratch: &Path,
    model_id: &str,
    repo_id: Option<&str>,
    cancel: &CancellationToken,
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
    let mut failures = Vec::new();
    let mut resolved = None;
    if repo_id.is_none() && has_package {
        let dir = model_dir.clone();
        match blocking(move || declarations::read_package(&dir, DeclarationSource::Package)).await {
            Ok(declared) => resolved = Some(declared),
            // A download that selected only some files can carry modules.json
            // without the module files it names; its lineage still publishes
            // the full package (ADR 0007).
            Err(error) if package_file_missing(&error) => {
                tracing::info!(
                    "The package of {model_id} is incomplete ({error}); searching its lineage"
                );
                failures.push(format!("its own package: {error}"));
            }
            Err(error) => return Err(error),
        }
    }
    if resolved.is_none() {
        let candidates = lineage(repo_id, &metadata);
        if !candidates.is_empty() {
            let client = client.ok_or_else(|| PumasError::InvalidParams {
                message: format!(
                    "Declarations must be read from {}, but Hugging Face access is disabled for \
                     this library instance",
                    candidates
                        .iter()
                        .map(|candidate| candidate.repo_id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })?;
            tokio::fs::create_dir_all(scratch)
                .await
                .map_err(|error| PumasError::io("creating declaration scratch", scratch, error))?;
            for candidate in candidates {
                searched.push(candidate.repo_id.clone());
                match read_repository(client, scratch, &candidate, cancel).await {
                    Ok(Some(declared)) => {
                        resolved = Some(declared);
                        break;
                    }
                    Ok(None) => {}
                    Err(PumasError::ConversionCancelled) => {
                        return Err(PumasError::ConversionCancelled)
                    }
                    Err(error) => {
                        tracing::info!(
                            "Declarations of {model_id} not read from {}: {error}; trying the \
                             next lineage candidate",
                            candidate.repo_id
                        );
                        failures.push(format!("{}: {error}", candidate.repo_id));
                    }
                }
            }
        }
    }

    let Some(declared) = resolved else {
        if failures.is_empty() {
            return Ok(DeclarationResolution::NotDeclared { searched });
        }
        return Err(PumasError::InvalidParams {
            message: format!(
                "No sentence-transformers declarations of {model_id} could be read: {}",
                failures.join("; ")
            ),
        });
    };
    let record = declared.clone();
    library
        .update_metadata(&model_dir, move |metadata| {
            metadata.sentence_transformers = Some(record);
        })
        .await?;
    library.index_model_dir(&model_dir).await?;
    Ok(DeclarationResolution::Resolved {
        declarations: Box::new(declared),
    })
}

/// Read one lineage repository's package at the candidate's revision, in a
/// scratch directory removed afterwards. `None` when it publishes none.
async fn read_repository(
    client: &HuggingFaceClient,
    scratch: &Path,
    candidate: &LineageCandidate,
    cancel: &CancellationToken,
) -> Result<Option<SentenceTransformersDeclarations>> {
    let work = {
        let scratch = scratch.to_path_buf();
        blocking(move || {
            tempfile::Builder::new()
                .prefix("pumas-declarations-")
                .tempdir_in(&scratch)
                .map_err(|error| PumasError::io("allocating declaration scratch", &scratch, error))
        })
        .await?
    };
    let Some(revision) = client
        .fetch_sentence_transformers_package(
            &candidate.repo_id,
            candidate.revision.as_deref(),
            work.path(),
            cancel,
        )
        .await?
    else {
        return Ok(None);
    };
    let repo_id = candidate.repo_id.clone();
    blocking(move || {
        let declared = declarations::read_package(
            work.path(),
            DeclarationSource::Repository { repo_id, revision },
        );
        drop(work);
        declared.map(Some)
    })
    .await
}

/// A repository to search, at the revision the model's files came from when
/// the model records one for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LineageCandidate {
    repo_id: String,
    revision: Option<String>,
}

/// Repositories to search, in order, without duplicates. The model's own
/// source repository is read at its recorded `upstream_revision` (the commit
/// its weights were downloaded at); a base model, which records none, at its
/// current `main`. The commit actually read is kept in the record's source.
fn lineage(requested: Option<&str>, metadata: &ModelMetadata) -> Vec<LineageCandidate> {
    let source = metadata
        .repo_id
        .as_deref()
        .map(str::trim)
        .filter(|repo| !repo.is_empty());
    let recorded_revision = metadata
        .upstream_revision
        .as_deref()
        .map(str::trim)
        .filter(|revision| !revision.is_empty())
        .map(str::to_string);
    let candidate = |repo: &str| LineageCandidate {
        repo_id: repo.to_string(),
        revision: (Some(repo) == source)
            .then(|| recorded_revision.clone())
            .flatten(),
    };
    if let Some(repo) = requested {
        return vec![candidate(repo.trim())];
    }
    let mut repos: Vec<LineageCandidate> = Vec::new();
    let mut push = |repo: &str| {
        let repo = repo.trim();
        if !repo.is_empty() && !repos.iter().any(|known| known.repo_id == repo) {
            repos.push(candidate(repo));
        }
    };
    if let Some(repo) = source {
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

/// Model types whose runtimes compute from sentence-transformers
/// declarations: embedders, rerankers and decision models.
const DECLARED_MODEL_TYPES: &[&str] = &["embedding", "reranker", "decision"];

/// Whether `record` is a complete model of a declared type without
/// declarations. A download's record exists from the moment it starts, so
/// it is complete only once `download_incomplete` is cleared.
fn needs_declarations(record: &crate::index::ModelRecord) -> bool {
    DECLARED_MODEL_TYPES.contains(&record.model_type.as_str())
        && record
            .metadata
            .get("download_incomplete")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        && record
            .metadata
            .get("sentence_transformers")
            .is_none_or(serde_json::Value::is_null)
}

/// Whether a library change leaves `record` needing declarations. "Added"
/// is not completion for a download: completion is a later metadata change.
fn wants_declarations(
    event: &crate::models::ModelLibraryUpdateEvent,
    record: &crate::index::ModelRecord,
) -> bool {
    use crate::models::ModelLibraryChangeKind as Change;
    matches!(
        event.change_kind,
        Change::ModelAdded | Change::MetadataModified
    ) && needs_declarations(record)
}

/// Cancels the token when the reactor is dropped (aborted at shutdown), so
/// a transfer in progress stops at its next check.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Resolve declarations for each model that becomes complete without them,
/// whatever made it complete: a download, an import or a conversion.
/// Best-effort: a failure is logged and retried on the model's next change.
pub(super) async fn react_to_library_updates(
    library: Arc<ModelLibrary>,
    client: Arc<HuggingFaceClient>,
    scratch: PathBuf,
) {
    let cancel = CancellationToken::new();
    let _cancel_on_drop = CancelOnDrop(cancel.clone());
    let resolver_library = library.clone();
    react_to_library_updates_with(library, move |model_id| {
        let library = resolver_library.clone();
        let client = client.clone();
        let scratch = scratch.clone();
        let cancel = cancel.clone();
        async move { resolve(&library, Some(&client), &scratch, &model_id, None, &cancel).await }
    })
    .await
}

/// Follow the update feed and resolve each model that needs declarations.
///
/// Receiving never waits on a resolution: models are queued (once each) for
/// one worker in the same task, so a slow fetch cannot make the feed lag.
/// If it lags anyway, the missed events are replaced by a rescan of the
/// library for complete models still lacking declarations. Dropping the
/// future stops both.
async fn react_to_library_updates_with<F, Fut>(library: Arc<ModelLibrary>, resolve_model: F)
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<DeclarationResolution>>,
{
    use tokio::sync::broadcast::error::RecvError;

    let mut updates = library.index().subscribe_model_library_update_events();
    // Models whose declarations are recorded or known to be absent this run.
    let settled = std::sync::Mutex::new(HashSet::<String>::new());
    // Models queued or being resolved.
    let queued = std::sync::Mutex::new(HashSet::<String>::new());
    let is_settled = |model_id: &str| settled.lock().is_ok_and(|set| set.contains(model_id));
    let settle = |model_id: String| {
        if let Ok(mut set) = settled.lock() {
            set.insert(model_id);
        }
    };
    let (queue, mut work) = tokio::sync::mpsc::unbounded_channel::<String>();
    let enqueue = |model_id: String| {
        if queued
            .lock()
            .is_ok_and(|mut set| set.insert(model_id.clone()))
        {
            let _ = queue.send(model_id);
        }
    };

    let receive = async {
        loop {
            let event = match updates.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(missed)) => {
                    tracing::info!(
                        "The declarations reactor missed {missed} library updates; rescanning \
                         the library for models without declarations"
                    );
                    match library.list_models().await {
                        Ok(records) => {
                            for record in records {
                                if needs_declarations(&record) && !is_settled(&record.id) {
                                    enqueue(record.id);
                                }
                            }
                        }
                        Err(error) => {
                            tracing::warn!("Declarations rescan failed: {error}");
                        }
                    }
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            if is_settled(&event.model_id) {
                continue;
            }
            let record = match library.get_model(&event.model_id).await {
                Ok(Some(record)) => record,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!("Declarations for {} not checked: {error}", event.model_id);
                    continue;
                }
            };
            if wants_declarations(&event, &record) {
                enqueue(event.model_id);
            } else if record
                .metadata
                .get("sentence_transformers")
                .is_some_and(|value| !value.is_null())
            {
                settle(event.model_id);
            }
        }
    };
    let resolve_queued = async {
        while let Some(model_id) = work.recv().await {
            let outcome = resolve_model(model_id.clone()).await;
            if let Ok(mut set) = queued.lock() {
                set.remove(&model_id);
            }
            match outcome {
                Ok(DeclarationResolution::Resolved { .. }) => {
                    tracing::info!("Recorded sentence-transformers declarations for {model_id}");
                    settle(model_id);
                }
                Ok(DeclarationResolution::NotDeclared { .. }) => settle(model_id),
                Err(error) => {
                    tracing::warn!("Declarations for {model_id} not resolved: {error}");
                }
            }
        }
    };
    // The worker waits on the queue for as long as the feed is open; the
    // feed closing (the library shutting down) ends the reactor.
    tokio::select! {
        () = receive => {}
        () = resolve_queued => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_reactor_resolves_a_download_when_it_completes_not_when_it_starts() {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(temp.path()).await.unwrap());
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observed = calls.clone();
        let reactor = tokio::spawn(react_to_library_updates_with(library.clone(), move |id| {
            observed.lock().unwrap().push(id);
            async {
                Ok(DeclarationResolution::NotDeclared {
                    searched: Vec::new(),
                })
            }
        }));
        tokio::task::yield_now().await;

        let dir = library.build_model_path("embedding", "acme", "encoder");
        std::fs::create_dir_all(&dir).unwrap();
        let metadata = ModelMetadata {
            model_id: Some("embedding/acme/encoder".into()),
            model_type: Some("embedding".into()),
            family: Some("acme".into()),
            cleaned_name: Some("encoder".into()),
            ..Default::default()
        };
        // The download starts: its record is indexed while bytes are partial.
        std::fs::write(dir.join("model.safetensors.part"), b"partial").unwrap();
        library.save_metadata(&dir, &metadata).await.unwrap();
        library.index_model_dir(&dir).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(
            calls.lock().unwrap().is_empty(),
            "a started download is not complete"
        );

        // It completes.
        std::fs::rename(
            dir.join("model.safetensors.part"),
            dir.join("model.safetensors"),
        )
        .unwrap();
        library.index_model_dir(&dir).await.unwrap();
        // A later change to the settled model does not resolve it again.
        let mut renamed = metadata.clone();
        renamed.official_name = Some("Encoder".into());
        library.save_metadata(&dir, &renamed).await.unwrap();
        library.index_model_dir(&dir).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        reactor.abort();
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["embedding/acme/encoder".to_string()]
        );
    }

    fn repos(candidates: Vec<LineageCandidate>) -> Vec<String> {
        candidates
            .into_iter()
            .map(|candidate| candidate.repo_id)
            .collect()
    }

    async fn encoder(library: &ModelLibrary, model_type: &str, name: &str) -> PathBuf {
        let dir = library.build_model_path(model_type, "acme", name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("model.safetensors"), b"weights").unwrap();
        let metadata = ModelMetadata {
            model_id: Some(format!("{model_type}/acme/{name}")),
            model_type: Some(model_type.into()),
            family: Some("acme".into()),
            cleaned_name: Some(name.into()),
            ..Default::default()
        };
        library.save_metadata(&dir, &metadata).await.unwrap();
        library.index_model_dir(&dir).await.unwrap();
        dir
    }

    fn recording_reactor(
        library: &Arc<ModelLibrary>,
        hold: Option<(String, Arc<tokio::sync::Notify>)>,
    ) -> (
        tokio::task::JoinHandle<()>,
        Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observed = calls.clone();
        let reactor = tokio::spawn(react_to_library_updates_with(library.clone(), move |id| {
            observed.lock().unwrap().push(id.clone());
            let hold = hold.clone();
            async move {
                if let Some((held, release)) = hold {
                    if held == id {
                        release.notified().await;
                    }
                }
                Ok(DeclarationResolution::NotDeclared {
                    searched: Vec::new(),
                })
            }
        }));
        (reactor, calls)
    }

    fn noise(library: &ModelLibrary, count: usize) {
        use crate::models::{ModelFactFamily, ModelLibraryChangeKind, ModelLibraryRefreshScope};
        for _ in 0..count {
            library
                .index()
                .append_model_library_update_event(
                    "embedding/acme/absent",
                    ModelLibraryChangeKind::MetadataModified,
                    ModelFactFamily::Metadata,
                    ModelLibraryRefreshScope::Summary,
                    None,
                    None,
                )
                .unwrap();
        }
    }

    #[tokio::test]
    async fn missed_updates_are_replaced_by_a_rescan_for_models_without_declarations() {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(temp.path()).await.unwrap());
        // Complete before the reactor starts, so only a rescan can find them.
        encoder(&library, "embedding", "missed").await;
        encoder(&library, "llm", "chat").await;
        let (reactor, calls) = recording_reactor(&library, None);
        tokio::task::yield_now().await;
        // More updates than the feed holds arrive before the reactor runs.
        noise(&library, 300);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        reactor.abort();
        assert_eq!(
            *calls.lock().unwrap(),
            vec!["embedding/acme/missed".to_string()],
            "the rescan finds the complete embedder, not the LLM"
        );
    }

    #[tokio::test]
    async fn a_resolution_in_progress_does_not_stop_the_feed_from_being_read() {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(temp.path()).await.unwrap());
        let release = Arc::new(tokio::sync::Notify::new());
        let (reactor, calls) = recording_reactor(
            &library,
            Some(("embedding/acme/first".to_string(), release.clone())),
        );
        tokio::task::yield_now().await;
        encoder(&library, "embedding", "first").await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        encoder(&library, "reranker", "second").await;
        // While the first resolution waits, the feed keeps flowing.
        for _ in 0..300 {
            noise(&library, 1);
            tokio::task::yield_now().await;
        }
        release.notify_one();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        reactor.abort();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "embedding/acme/first".to_string(),
                "reranker/acme/second".to_string()
            ]
        );
    }

    #[test]
    fn the_source_repository_is_read_at_the_recorded_revision() {
        let metadata = ModelMetadata {
            repo_id: Some("acme/encoder-GGUF".into()),
            upstream_revision: Some("0123abc".into()),
            base_model: Some(vec!["acme/encoder".into()]),
            ..Default::default()
        };
        assert_eq!(
            lineage(None, &metadata),
            [
                LineageCandidate {
                    repo_id: "acme/encoder-GGUF".into(),
                    revision: Some("0123abc".into())
                },
                LineageCandidate {
                    repo_id: "acme/encoder".into(),
                    revision: None
                }
            ]
        );
        assert_eq!(
            lineage(Some("acme/encoder-GGUF"), &metadata)[0]
                .revision
                .as_deref(),
            Some("0123abc")
        );
        assert_eq!(lineage(Some("acme/other"), &metadata)[0].revision, None);
    }

    const COMMIT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const COMMIT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    /// Answers each request by the first route whose prefix matches; before
    /// answering `gate.0`, waits for `gate.1`.
    async fn hub(
        listener: tokio::net::TcpListener,
        routes: Vec<(String, String)>,
        gate: (String, tokio::sync::oneshot::Receiver<()>),
    ) -> Vec<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (gated, mut release) = (gate.0, Some(gate.1));
        let mut seen = Vec::new();
        for _ in 0..routes.len() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0_u8; 8192];
            let read = socket.read(&mut buffer).await.unwrap();
            let line = String::from_utf8_lossy(&buffer[..read])
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            let body = routes
                .iter()
                .find(|(prefix, _)| line.starts_with(prefix.as_str()))
                .map(|(_, body)| body.clone())
                .unwrap_or_else(|| panic!("unexpected request {line}"));
            if line.starts_with(gated.as_str()) {
                if let Some(release) = release.take() {
                    release.await.unwrap();
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            seen.push(line);
        }
        seen
    }

    fn tree(files: &[&str]) -> String {
        serde_json::Value::Array(
            files
                .iter()
                .map(|path| serde_json::json!({"type": "file", "path": path, "size": 10}))
                .collect(),
        )
        .to_string()
    }

    #[tokio::test]
    async fn a_lineage_repository_that_fails_does_not_end_the_search_or_lose_concurrent_edits() {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(temp.path()).await.unwrap());
        let dir = encoder(&library, "embedding", "encoder").await;
        let mut metadata = library.load_metadata(&dir).unwrap().unwrap();
        metadata.repo_id = Some("acme/broken".into());
        metadata.upstream_revision = Some(COMMIT_A.into());
        metadata.base_model = Some(vec!["acme/base".into()]);
        library.save_metadata(&dir, &metadata).await.unwrap();
        library.index_model_dir(&dir).await.unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cache = tempfile::tempdir().unwrap();
        let mut client = HuggingFaceClient::new(cache.path()).unwrap();
        client.set_test_download_base_url(format!("http://{}", listener.local_addr().unwrap()));
        let revision = |repo: &str, sha: &str| {
            serde_json::json!({"id": repo, "modelId": repo, "sha": sha}).to_string()
        };
        let modules = serde_json::json!([
            {"path": "", "type": "sentence_transformers.models.Transformer"},
            {"path": "1_Pooling", "type": "sentence_transformers.models.Pooling"}
        ])
        .to_string();
        let routes = vec![
            // The source is read at its recorded commit, and its package is malformed.
            (
                format!("GET /api/models/acme/broken/revision/{COMMIT_A} "),
                revision("acme/broken", COMMIT_A),
            ),
            (
                format!("GET /api/models/acme/broken/tree/{COMMIT_A}"),
                tree(&["modules.json"]),
            ),
            (
                format!("GET /acme/broken/resolve/{COMMIT_A}/modules.json "),
                r#"{"not":"a list"}"#.to_string(),
            ),
            // The base model, which records no commit, is read at main.
            (
                "GET /api/models/acme/base/revision/main ".to_string(),
                revision("acme/base", COMMIT_B),
            ),
            (
                format!("GET /api/models/acme/base/tree/{COMMIT_B}"),
                tree(&["modules.json", "1_Pooling/config.json"]),
            ),
            (
                format!("GET /acme/base/resolve/{COMMIT_B}/modules.json "),
                modules,
            ),
            (
                format!("GET /acme/base/resolve/{COMMIT_B}/1_Pooling/config.json "),
                r#"{"pooling_mode":"cls"}"#.to_string(),
            ),
        ];
        let (release, gate) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(hub(
            listener,
            routes,
            (format!("GET /acme/base/resolve/{COMMIT_B}/1_Pooling"), gate),
        ));
        let scratch = temp.path().join("scratch");
        let resolving = {
            let library = library.clone();
            tokio::spawn(async move {
                resolve(
                    &library,
                    Some(&client),
                    &scratch,
                    "embedding/acme/encoder",
                    None,
                    &CancellationToken::new(),
                )
                .await
            })
        };
        // While the fetch is in flight, someone else edits the metadata.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let mut edited = library.load_metadata(&dir).unwrap().unwrap();
        edited.official_name = Some("Edited while resolving".into());
        library.save_metadata(&dir, &edited).await.unwrap();
        release.send(()).unwrap();

        let resolution = resolving.await.unwrap().unwrap();
        let DeclarationResolution::Resolved { declarations } = resolution else {
            panic!("expected declarations from the base model, got {resolution:?}");
        };
        assert_eq!(
            declarations.source,
            DeclarationSource::Repository {
                repo_id: "acme/base".into(),
                revision: COMMIT_B.into()
            }
        );
        let saved = library.load_metadata(&dir).unwrap().unwrap();
        assert_eq!(
            saved.official_name.as_deref(),
            Some("Edited while resolving")
        );
        assert_eq!(
            saved.sentence_transformers.map(|declared| declared.source),
            Some(declarations.source)
        );
        server.await.unwrap();
    }

    #[test]
    fn lineage_searches_the_source_then_its_single_base_model() {
        let metadata = ModelMetadata {
            repo_id: Some("mradermacher/zerank-2-GGUF".into()),
            base_model: Some(vec!["zeroentropy/zerank-2-reranker".into()]),
            ..Default::default()
        };
        assert_eq!(
            repos(lineage(None, &metadata)),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(repos(lineage(Some(" acme/x "), &metadata)), ["acme/x"]);
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
        assert_eq!(repos(lineage(None, &same)), ["a/b"]);
    }

    #[test]
    fn the_model_card_names_the_base_model_when_none_is_recorded() {
        let card = |value: serde_json::Value| ModelMetadata {
            repo_id: Some("mradermacher/zerank-2-GGUF".into()),
            model_card: Some([("base_model".to_string(), value)].into_iter().collect()),
            ..Default::default()
        };
        assert_eq!(
            repos(lineage(
                None,
                &card(serde_json::json!("zeroentropy/zerank-2-reranker"))
            )),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(
            repos(lineage(
                None,
                &card(serde_json::json!(["zeroentropy/zerank-2-reranker"]))
            )),
            [
                "mradermacher/zerank-2-GGUF",
                "zeroentropy/zerank-2-reranker"
            ]
        );
        assert_eq!(
            repos(lineage(None, &card(serde_json::json!(["a/b", "c/d"])))),
            ["mradermacher/zerank-2-GGUF"]
        );
    }
}

//! Fetch a sentence-transformers package's declaration files (ADR 0007) from
//! one immutable revision of a Hugging Face repository.
//!
//! This owns transport only: which files exist at the revision, their bounds
//! and their integrity. What they declare is decided by
//! [`crate::model_library::declarations`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::HuggingFaceClient;
use crate::cancel::CancellationToken;
use crate::model_library::artifact_identity::DownloadRevision;
use crate::{PumasError, Result};

/// Hugging Face stores any file over 10 MiB through LFS, so a regular (Git)
/// file is never larger. LFS files are bounded by their recorded size.
const MAX_REGULAR_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// How long a file transfer may go without receiving data before it is
/// abandoned as stalled. Activity-based: a slow transfer that keeps
/// delivering bytes is never cut off.
const STALL_AFTER: Duration = Duration::from_secs(60);

const MODULES: &str = "modules.json";
/// Package-level files a sentence-transformers cross-encoder may declare.
const PACKAGE_FILES: &[&str] = &[
    "config_sentence_transformers.json",
    "sentence_bert_config.json",
    "chat_template.jinja",
    "tokenizer_config.json",
    "tokenizer.json",
];

impl HuggingFaceClient {
    /// Write the declaration files of `repo_id`'s sentence-transformers
    /// package into `destination` (existing and empty) and return the
    /// immutable revision read, or `None` when the repository publishes no
    /// `modules.json`. `revision` selects the commit (the one the model's
    /// weights were downloaded at); without one the current `main` is read.
    /// Checks `cancel` between files and while receiving, and abandons a
    /// transfer that delivers no data for [`STALL_AFTER`].
    pub(crate) async fn fetch_sentence_transformers_package(
        &self,
        repo_id: &str,
        revision: Option<&str>,
        destination: &Path,
        cancel: &CancellationToken,
    ) -> Result<Option<String>> {
        self.fetch_sentence_transformers_package_within(
            repo_id,
            revision,
            destination,
            cancel,
            STALL_AFTER,
        )
        .await
    }

    async fn fetch_sentence_transformers_package_within(
        &self,
        repo_id: &str,
        revision: Option<&str>,
        destination: &Path,
        cancel: &CancellationToken,
        stall_after: Duration,
    ) -> Result<Option<String>> {
        let revision = self.resolve_download_revision(repo_id, revision).await?;
        let tree = self.get_repo_files_at_revision(repo_id, &revision).await?;
        let lfs: HashMap<&str, (u64, &str)> = tree
            .lfs_files
            .iter()
            .map(|file| (file.filename.as_str(), (file.size, file.sha256.as_str())))
            .collect();
        let regular: HashSet<&str> = tree.regular_files.iter().map(String::as_str).collect();
        let listed = |path: &str| lfs.contains_key(path) || regular.contains(path);
        if !listed(MODULES) {
            return Ok(None);
        }
        let modules = self
            .copy_contract_file(
                repo_id,
                &revision,
                MODULES,
                (&lfs, destination),
                cancel,
                stall_after,
            )
            .await?;
        let mut paths: Vec<String> = module_config_paths(&modules)?
            .into_iter()
            .filter(|path| listed(path))
            .collect();
        paths.extend(
            PACKAGE_FILES
                .iter()
                .filter(|path| listed(path))
                .map(|path| path.to_string()),
        );
        for path in paths {
            self.copy_contract_file(
                repo_id,
                &revision,
                &path,
                (&lfs, destination),
                cancel,
                stall_after,
            )
            .await?;
        }
        Ok(Some(revision.as_str().to_string()))
    }
}

impl HuggingFaceClient {
    async fn copy_contract_file(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
        path: &str,
        (lfs, destination): (&HashMap<&str, (u64, &str)>, &Path),
        cancel: &CancellationToken,
        stall_after: Duration,
    ) -> Result<Vec<u8>> {
        cancel_check(cancel)?;
        let bytes = self
            .fetch_contract_file(
                repo_id,
                revision,
                path,
                lfs.get(path).copied(),
                cancel,
                stall_after,
            )
            .await?;
        write_contained(destination, path, &bytes).await?;
        Ok(bytes)
    }
}

/// Resolves once `cancel` is cancelled; the token has no waker, so it is
/// observed at a short interval.
async fn until_cancelled(cancel: &CancellationToken) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// `step`'s output, `None` when it delivers nothing within `stall_after`,
/// or the cancellation when `cancel` fires first.
async fn within<T>(
    step: impl std::future::Future<Output = T>,
    cancel: &CancellationToken,
    stall_after: Duration,
) -> Result<Option<T>> {
    tokio::select! {
        biased;
        () = until_cancelled(cancel) => Err(PumasError::ConversionCancelled),
        outcome = tokio::time::timeout(stall_after, step) => Ok(outcome.ok()),
    }
}

fn cancel_check(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        Err(PumasError::ConversionCancelled)
    } else {
        Ok(())
    }
}

/// `<module path>/config.json` for each declared module with a path. Which
/// module scores is the script's decision; this only scopes the fetch.
fn module_config_paths(modules: &[u8]) -> Result<Vec<String>> {
    let modules: serde_json::Value =
        serde_json::from_slice(modules).map_err(|error| PumasError::InvalidParams {
            message: format!("modules.json is not valid JSON: {error}"),
        })?;
    let records = modules
        .as_array()
        .ok_or_else(|| PumasError::InvalidParams {
            message: "modules.json must be a list of module records".into(),
        })?;
    let mut paths = Vec::new();
    for record in records {
        let Some(path) = record.get("path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let config = format!("{path}/config.json");
        relative_path(&config)?;
        paths.push(config);
    }
    Ok(paths)
}

/// A repository path made only of normal components.
fn relative_path(path: &str) -> Result<PathBuf> {
    if !crate::model_library::declarations::is_package_relative(path) {
        return Err(PumasError::InvalidParams {
            message: format!("Scoring contract path {path:?} is not a plain repository path"),
        });
    }
    Ok(PathBuf::from(path))
}

async fn write_contained(destination: &Path, path: &str, bytes: &[u8]) -> Result<()> {
    let target = destination.join(relative_path(path)?);
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            PumasError::io("creating scoring contract directory", parent, error)
        })?;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .await
        .map_err(|error| PumasError::io("writing scoring contract file", &target, error))?;
    file.write_all(bytes)
        .await
        .map_err(|error| PumasError::io("writing scoring contract file", &target, error))?;
    file.flush()
        .await
        .map_err(|error| PumasError::io("writing scoring contract file", &target, error))
}

impl HuggingFaceClient {
    /// Read one file at an immutable revision, bounded by Hugging Face's own
    /// size facts and checked against its LFS digest when it has one.
    async fn fetch_contract_file(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
        path: &str,
        lfs: Option<(u64, &str)>,
        cancel: &CancellationToken,
        stall_after: Duration,
    ) -> Result<Vec<u8>> {
        relative_path(path)?;
        let encoded = path
            .split('/')
            .map(|part| urlencoding::encode(part).into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let url = format!(
            "{}/{}/resolve/{}/{}",
            self.hub_base_url(),
            repo_id,
            revision.as_str(),
            encoded
        );
        // A file body is an artifact transfer: like downloads it has a connect
        // timeout but no total timeout, and cancellation is checked per chunk.
        let auth = self.auth_header_value().await;
        let mut request = self
            .download_client
            .for_request(&url, auth.is_some())?
            .get(&url);
        if let Some(auth) = auth {
            request = request.header("Authorization", auth);
        }
        let network = |message: String| PumasError::Network {
            message,
            cause: None,
        };
        let stalled = || {
            network(format!(
                "{repo_id}/{path} sent no data for {} seconds; the transfer stalled",
                stall_after.as_secs_f32()
            ))
        };
        // Each wait is bounded by inactivity and ends early on cancellation.
        let mut response = within(request.send(), cancel, stall_after)
            .await?
            .ok_or_else(stalled)?
            .map_err(|error| network(format!("Failed to fetch {path} from {repo_id}: {error}")))?;
        if !response.status().is_success() {
            return Err(network(format!(
                "Hugging Face returned {} for {repo_id}/{path}",
                response.status()
            )));
        }
        let limit = lfs.map_or(MAX_REGULAR_FILE_BYTES, |(size, _)| size);
        let mut bytes = Vec::new();
        while let Some(chunk) = within(response.chunk(), cancel, stall_after)
            .await?
            .ok_or_else(stalled)?
            .map_err(|error| network(format!("Failed to read {path} from {repo_id}: {error}")))?
        {
            cancel_check(cancel)?;
            let within = u64::try_from(bytes.len())
                .ok()
                .zip(u64::try_from(chunk.len()).ok())
                .and_then(|(held, more)| held.checked_add(more))
                .is_some_and(|total| total <= limit);
            if !within {
                return Err(PumasError::InvalidParams {
                    message: format!("{repo_id}/{path} is larger than Hugging Face lists it"),
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        if let Some((size, digest)) = lfs {
            let length = u64::try_from(bytes.len()).ok();
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if length != Some(size) || !actual.eq_ignore_ascii_case(digest) {
                return Err(PumasError::InvalidParams {
                    message: format!("{repo_id}/{path} does not match its listed size and digest"),
                });
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    async fn serve(listener: TcpListener, routes: Vec<(String, String)>) -> Vec<String> {
        let mut seen = Vec::new();
        for _ in 0..routes.len() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0_u8; 8192];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let line = request.lines().next().unwrap_or_default().to_string();
            let body = routes
                .iter()
                .find(|(prefix, _)| line.starts_with(prefix.as_str()))
                .map(|(_, body)| body.clone())
                .unwrap_or_else(|| panic!("unexpected request {line}"));
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            seen.push(line);
        }
        seen
    }

    fn tree(files: &[&str], lfs: &[(&str, &str)]) -> String {
        let mut entries: Vec<serde_json::Value> = files
            .iter()
            .map(|path| serde_json::json!({"type": "file", "path": path, "size": 10}))
            .collect();
        for (path, body) in lfs {
            entries.push(serde_json::json!({
                "type": "file", "path": path, "size": body.len(),
                "lfs": {"oid": format!("{:x}", Sha256::digest(body.as_bytes())), "size": body.len()}
            }));
        }
        serde_json::Value::Array(entries).to_string()
    }

    fn routes(repo: &str, tree_body: String, files: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut routes = vec![
            (
                format!("GET /api/models/{repo}/revision/main "),
                serde_json::json!({"id": repo, "modelId": repo, "sha": COMMIT}).to_string(),
            ),
            (format!("GET /api/models/{repo}/tree/{COMMIT}"), tree_body),
        ];
        for (path, body) in files {
            routes.push((
                format!("GET /{repo}/resolve/{COMMIT}/{path} "),
                body.to_string(),
            ));
        }
        routes
    }

    async fn client(listener: &TcpListener) -> (HuggingFaceClient, TempDir) {
        let cache = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(cache.path()).unwrap();
        client.set_test_download_base_url(format!("http://{}", listener.local_addr().unwrap()));
        (client, cache)
    }

    #[tokio::test]
    async fn fetches_declared_module_configs_and_listed_package_files_at_one_commit() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let modules = r#"[{"path":""},{"path":"1_LogitScore"}]"#;
        let score = r#"{"true_token_id":9454,"false_token_id":null}"#;
        let tokenizer = r#"{"model":{"vocab":{}}}"#;
        let files = [
            ("modules.json", modules),
            ("1_LogitScore/config.json", score),
            ("chat_template.jinja", "{{ messages }}"),
            ("tokenizer.json", tokenizer),
        ];
        let tree_body = tree(
            &[
                "modules.json",
                "1_LogitScore/config.json",
                "chat_template.jinja",
                "README.md",
            ],
            &[("tokenizer.json", tokenizer)],
        );
        let server = tokio::spawn(serve(listener, routes("acme/reranker", tree_body, &files)));
        let destination = TempDir::new().unwrap();

        let revision = client
            .fetch_sentence_transformers_package(
                "acme/reranker",
                None,
                destination.path(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(revision.as_deref(), Some(COMMIT));
        for (path, body) in files {
            assert_eq!(
                std::fs::read_to_string(destination.path().join(path)).unwrap(),
                body
            );
        }
        assert!(!destination.path().join("README.md").exists());
        let seen = server.await.unwrap();
        assert_eq!(seen.len(), 6);
    }

    #[tokio::test]
    async fn a_repository_without_modules_has_no_package_and_fetches_nothing() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let server = tokio::spawn(serve(
            listener,
            routes("acme/plain", tree(&["config.json"], &[]), &[]),
        ));
        let destination = TempDir::new().unwrap();
        let fetched = client
            .fetch_sentence_transformers_package(
                "acme/plain",
                None,
                destination.path(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(fetched, None);
        assert_eq!(server.await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn an_lfs_file_that_differs_from_its_digest_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let tree_body = tree(&["modules.json"], &[("tokenizer.json", "expected")]);
        let files = [("modules.json", "[]"), ("tokenizer.json", "tampered")];
        let server = tokio::spawn(serve(listener, routes("acme/lfs", tree_body, &files)));
        let destination = TempDir::new().unwrap();
        let error = client
            .fetch_sentence_transformers_package(
                "acme/lfs",
                None,
                destination.path(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, PumasError::InvalidParams { message } if message.contains("listed size and digest"))
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn the_recorded_revision_is_read_instead_of_main() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let mut pinned = routes("acme/pinned", tree(&["config.json"], &[]), &[]);
        pinned[0].0 = format!("GET /api/models/acme/pinned/revision/{COMMIT} ");
        let server = tokio::spawn(serve(listener, pinned));
        let destination = TempDir::new().unwrap();
        client
            .fetch_sentence_transformers_package(
                "acme/pinned",
                Some(COMMIT),
                destination.path(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        let seen = server.await.unwrap();
        assert!(
            seen[0].contains(&format!("/revision/{COMMIT} ")),
            "{seen:?}"
        );
    }

    /// Serves the revision and tree, then starts `modules.json` and stops
    /// sending without closing the connection.
    async fn serve_then_stall(listener: TcpListener, repo: &str) {
        let replies = [
            serde_json::json!({"id": repo, "modelId": repo, "sha": COMMIT}).to_string(),
            tree(&["modules.json"], &[]),
        ];
        for body in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0_u8; 8192];
            let _ = socket.read(&mut buffer).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0_u8; 8192];
        let _ = socket.read(&mut buffer).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n[{\"path\"")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }

    #[tokio::test]
    async fn a_transfer_that_stops_sending_data_is_abandoned_as_stalled() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let server = tokio::spawn(serve_then_stall(listener, "acme/stall"));
        let destination = TempDir::new().unwrap();
        let started = std::time::Instant::now();
        let error = client
            .fetch_sentence_transformers_package_within(
                "acme/stall",
                None,
                destination.path(),
                &CancellationToken::new(),
                Duration::from_millis(300),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stalled"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(10));
        server.abort();
    }

    #[tokio::test]
    async fn cancellation_stops_a_transfer_waiting_for_data() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let server = tokio::spawn(serve_then_stall(listener, "acme/stop"));
        let destination = TempDir::new().unwrap();
        let cancel = CancellationToken::new();
        let stopper = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            stopper.cancel();
        });
        let error = client
            .fetch_sentence_transformers_package_within(
                "acme/stop",
                None,
                destination.path(),
                &cancel,
                Duration::from_secs(60),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, PumasError::ConversionCancelled),
            "{error:?}"
        );
        server.abort();
    }

    #[test]
    fn module_paths_must_stay_inside_the_repository() {
        for bad in [
            r#"[{"path":"../x"}]"#,
            r#"[{"path":"/abs"}]"#,
            r#"[{"path":"a\\b"}]"#,
        ] {
            assert!(module_config_paths(bad.as_bytes()).is_err(), "{bad}");
        }
        assert_eq!(
            module_config_paths(br#"[{"path":""},{"path":"1_LogitScore"},{"type":"x"}]"#).unwrap(),
            vec!["1_LogitScore/config.json"]
        );
    }

    #[tokio::test]
    async fn a_cancelled_fetch_stops_before_the_next_file() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, _cache) = client(&listener).await;
        let server = tokio::spawn(serve(
            listener,
            routes("acme/cancel", tree(&["modules.json"], &[]), &[]),
        ));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let destination = TempDir::new().unwrap();
        let error = client
            .fetch_sentence_transformers_package("acme/cancel", None, destination.path(), &cancel)
            .await
            .unwrap_err();
        assert!(matches!(error, PumasError::ConversionCancelled));
        server.await.unwrap();
    }
}

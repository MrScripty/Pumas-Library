//! Read a remote GGUF's metadata header before downloading it.
//!
//! A llama.cpp decision model declares `<arch>.decision.type` among its model
//! keys, ahead of the tokenizer arrays. Classification after download reads
//! that key from the package's top-level primary GGUFs; the download
//! preflight reads the same key from the same files with HTTP Range requests,
//! so the model is placed by the same evidence.

use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::types::{DownloadRequest, RepoFileTree};
use crate::model_library::{
    is_primary_gguf_name, probe_gguf_decision_head, GgufDecisionHeadProbe,
    MAX_GGUF_METADATA_PREFIX_BYTES,
};
use futures::StreamExt;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use std::future::Future;
use std::pin::Pin;

/// The first range covers the model keys of every GGUF seen so far; a longer
/// header is read in successively doubled ranges until its keys end, never
/// past [`MAX_GGUF_METADATA_PREFIX_BYTES`].
const FIRST_RANGE_BYTES: u64 = 256 * 1024;

/// What a remote GGUF header shows about a decision head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GgufHeaderEvidence {
    DecisionHead,
    NoDecisionHead,
    /// The header could not be read as evidence; the reason says why.
    Unreadable(String),
}

pub(crate) type GgufHeaderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<GgufHeaderEvidence>> + Send + 'a>>;

/// Reads one pinned GGUF's header. The client reads over HTTP; tests inject
/// a reader for scripted sources.
pub(crate) trait GgufHeaderReader: Send + Sync {
    fn read<'a>(
        &'a self,
        client: &'a HuggingFaceClient,
        repo_id: &'a str,
        path: &'a str,
        revision: &'a DownloadRevision,
    ) -> GgufHeaderFuture<'a>;
}

/// Range requests against the Hub's `resolve` endpoint.
pub(crate) struct HttpGgufHeaderReader;

impl GgufHeaderReader for HttpGgufHeaderReader {
    fn read<'a>(
        &'a self,
        client: &'a HuggingFaceClient,
        repo_id: &'a str,
        path: &'a str,
        revision: &'a DownloadRevision,
    ) -> GgufHeaderFuture<'a> {
        Box::pin(read_header_over_http(client, repo_id, path, revision))
    }
}

impl HuggingFaceClient {
    /// The decision-head evidence of the primary GGUFs the request selects:
    /// a decision head in any of them decides. `NoDecisionHead` when the
    /// selection has no primary GGUF.
    pub(crate) async fn selected_gguf_decision_head(
        &self,
        request: &DownloadRequest,
        revision: &DownloadRevision,
    ) -> Result<GgufHeaderEvidence> {
        let explicit = explicit_selected_files(request);
        let tree = match explicit {
            Some(_) => None,
            None => Some(
                self.get_repo_files_at_revision(&request.repo_id, revision)
                    .await?,
            ),
        };
        let candidates = primary_gguf_candidates(request, explicit, tree.as_ref());
        let reader = self.gguf_header_reader.clone();
        let pending: Vec<GgufHeaderFuture<'_>> = candidates
            .iter()
            .map(|path| reader.read(self, &request.repo_id, path, revision))
            .collect();
        let mut reads = futures::stream::iter(pending)
            .buffer_unordered(crate::config::NetworkConfig::HF_METADATA_REQUEST_CONCURRENCY);
        let mut failure = None;
        let mut unreadable = Vec::new();
        while let Some(evidence) = reads.next().await {
            match evidence {
                // Dropping the stream cancels the reads still in flight.
                Ok(GgufHeaderEvidence::DecisionHead) => {
                    return Ok(GgufHeaderEvidence::DecisionHead)
                }
                Ok(GgufHeaderEvidence::NoDecisionHead) => {}
                Ok(GgufHeaderEvidence::Unreadable(reason)) => unreadable.push(reason),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        if !unreadable.is_empty() {
            return Ok(GgufHeaderEvidence::Unreadable(unreadable.join("; ")));
        }
        Ok(GgufHeaderEvidence::NoDecisionHead)
    }
}

/// Read only as much of one pinned GGUF as its model keys occupy.
async fn read_header_over_http(
    client: &HuggingFaceClient,
    repo_id: &str,
    path: &str,
    revision: &DownloadRevision,
) -> Result<GgufHeaderEvidence> {
    super::metadata::validate_hf_repo_id(repo_id)?;
    let url = header_url(client.hub_base_url(), repo_id, revision, path)?;
    let authorization = client.auth_header_value().await;
    let network = |error: &dyn std::fmt::Display| PumasError::Network {
        message: format!("Failed to read the GGUF header of {repo_id}/{path}: {error}"),
        cause: Some(error.to_string()),
    };
    let mut prefix: Vec<u8> = Vec::new();
    let mut range = FIRST_RANGE_BYTES;
    loop {
        let start = prefix.len() as u64;
        if start >= MAX_GGUF_METADATA_PREFIX_BYTES {
            return Ok(GgufHeaderEvidence::Unreadable(format!(
                "the model keys of {repo_id}/{path} do not end within the \
                 {MAX_GGUF_METADATA_PREFIX_BYTES}-byte GGUF metadata bound"
            )));
        }
        let end = start
            .saturating_add(range)
            .min(MAX_GGUF_METADATA_PREFIX_BYTES)
            - 1;
        let mut request = client
            .client
            .get(url.clone())
            .header(RANGE, format!("bytes={start}-{end}"));
        if let Some(auth) = authorization.as_deref() {
            request = request.header("Authorization", auth);
        }
        let mut response = request.send().await.map_err(|error| network(&error))?;
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(PumasError::Network {
                message: format!(
                    "HuggingFace Hub returned {} for a byte range of {repo_id}/{path}; reading \
                     its GGUF header without downloading the file needs a partial-content \
                     response",
                    response.status()
                ),
                cause: None,
            });
        }
        let (first, last, total) = crate::acquisition::parse_content_range(&response)?;
        if first != start || last > end {
            return Err(PumasError::Network {
                message: format!(
                    "HuggingFace Hub answered a GGUF header range of {repo_id}/{path} with \
                     bytes {first}-{last}, not the requested {start}-{end}"
                ),
                cause: None,
            });
        }
        let expected = last - first + 1;
        let mut received = 0_u64;
        while let Some(chunk) = response.chunk().await.map_err(|error| network(&error))? {
            received = received
                .checked_add(chunk.len() as u64)
                .filter(|received| *received <= expected)
                .ok_or_else(|| PumasError::Network {
                    message: format!(
                        "HuggingFace Hub sent more than the {expected} bytes it announced for \
                         a GGUF header range of {repo_id}/{path}"
                    ),
                    cause: None,
                })?;
            prefix.extend_from_slice(&chunk);
        }
        if received != expected {
            return Err(PumasError::Network {
                message: format!(
                    "HuggingFace Hub sent {received} of the {expected} bytes it announced for \
                     a GGUF header range of {repo_id}/{path}"
                ),
                cause: None,
            });
        }
        let complete_file = prefix.len() as u64 >= total;
        match probe_gguf_decision_head(&prefix, complete_file) {
            GgufDecisionHeadProbe::Declared(true) => return Ok(GgufHeaderEvidence::DecisionHead),
            GgufDecisionHeadProbe::Declared(false) => {
                return Ok(GgufHeaderEvidence::NoDecisionHead)
            }
            GgufDecisionHeadProbe::Unreadable(reason) => {
                return Ok(GgufHeaderEvidence::Unreadable(format!(
                    "{repo_id}/{path}: {reason}"
                )))
            }
            GgufDecisionHeadProbe::NeedMore => {}
        }
        range = range.saturating_mul(2);
    }
}

/// `<base>/<repo>/resolve/<revision>/<path>`, each segment percent-encoded.
fn header_url(
    base: &str,
    repo_id: &str,
    revision: &DownloadRevision,
    path: &str,
) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).map_err(|_| PumasError::Config {
        message: "Hugging Face base URL is invalid".into(),
    })?;
    url.path_segments_mut()
        .map_err(|_| PumasError::Config {
            message: "Hugging Face base URL cannot contain path segments".into(),
        })?
        .pop_if_empty()
        .extend(repo_id.split('/'))
        .push("resolve")
        .push(revision.as_str())
        .extend(path.split('/'));
    Ok(url)
}

/// Files the request names itself, before any repository tree is needed.
fn explicit_selected_files(request: &DownloadRequest) -> Option<Vec<String>> {
    if let Some(filenames) = &request.filenames {
        return Some(filenames.clone());
    }
    request.filename.as_ref().map(|name| vec![name.clone()])
}

/// The primary GGUFs a download selects, in the same priority as the
/// download's own selection (explicit names, then the quantization substring,
/// then every LFS file). Only top-level files carry the evidence, as after
/// download.
fn primary_gguf_candidates(
    request: &DownloadRequest,
    explicit: Option<Vec<String>>,
    tree: Option<&RepoFileTree>,
) -> Vec<String> {
    let selected = match (explicit, tree) {
        (Some(files), _) => files,
        (None, Some(tree)) => tree
            .lfs_files
            .iter()
            .filter(|file| {
                request
                    .quant
                    .as_deref()
                    .is_none_or(|quant| file.filename.contains(quant))
            })
            .map(|file| file.filename.clone())
            .collect(),
        (None, None) => Vec::new(),
    };
    let mut candidates: Vec<String> = selected
        .into_iter()
        .filter(|path| !path.contains('/') && is_primary_gguf_name(path))
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates
}

#[cfg(test)]
type ScriptedAnswer = Box<dyn FnMut(&str) -> Result<GgufHeaderEvidence> + Send>;

/// A reader that answers from a script instead of the network.
#[cfg(test)]
pub(crate) struct ScriptedGgufHeaderReader {
    pub(crate) answer: std::sync::Mutex<ScriptedAnswer>,
    pub(crate) reads: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl ScriptedGgufHeaderReader {
    pub(crate) fn new(
        answer: impl FnMut(&str) -> Result<GgufHeaderEvidence> + Send + 'static,
    ) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            answer: std::sync::Mutex::new(Box::new(answer)),
            reads: std::sync::Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn reads(&self) -> Vec<String> {
        self.reads.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl GgufHeaderReader for ScriptedGgufHeaderReader {
    fn read<'a>(
        &'a self,
        _client: &'a HuggingFaceClient,
        _repo_id: &'a str,
        path: &'a str,
        _revision: &'a DownloadRevision,
    ) -> GgufHeaderFuture<'a> {
        self.reads.lock().unwrap().push(path.to_string());
        let answer = (self.answer.lock().unwrap())(path);
        Box::pin(async move { answer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::test_gguf_with_string_keys as gguf_header;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    /// Serve byte ranges of `file`, recording each request line and range.
    async fn serve_ranges(
        file: Vec<u8>,
        responses: usize,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for _ in 0..responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = socket.read(&mut buffer).await.unwrap();
                    assert!(read > 0);
                    request.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8(request).unwrap();
                let range = request
                    .lines()
                    .find_map(|line| line.strip_prefix("range: bytes="))
                    .unwrap()
                    .to_string();
                let (first, last) = range.split_once('-').unwrap();
                let first: usize = first.parse().unwrap();
                let last: usize = last.parse::<usize>().unwrap().min(file.len() - 1);
                let body = &file[first..=last];
                seen.push(format!("{} {range}", request.lines().next().unwrap()));
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {first}-{last}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            file.len(),
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                socket.write_all(body).await.unwrap();
            }
            seen
        });
        (base_url, server)
    }

    fn client(temp: &TempDir, base_url: String) -> HuggingFaceClient {
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        client
    }

    async fn read(client: &HuggingFaceClient, path: &str) -> Result<GgufHeaderEvidence> {
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        read_header_over_http(client, "acme/kev", path, &revision).await
    }

    #[tokio::test]
    async fn decision_head_is_read_from_one_pinned_range_without_the_tensors() {
        // Kev declares its head after its general keys; the tensors that
        // follow the header are never requested.
        let file = gguf_header(
            &[
                ("general.architecture", "qwen35"),
                ("general.name", "Kev-4B"),
                ("qwen35.decision.type", "kev"),
                ("tokenizer.ggml.model", "gpt2"),
            ],
            4 * FIRST_RANGE_BYTES as usize,
        );
        let (base_url, server) = serve_ranges(file, 1).await;
        let temp = TempDir::new().unwrap();
        assert_eq!(
            read(&client(&temp, base_url), "Kev 4B Q8_0.gguf")
                .await
                .unwrap(),
            GgufHeaderEvidence::DecisionHead
        );
        let seen = server.await.unwrap();
        // Each path segment is percent-encoded.
        assert_eq!(
            seen,
            vec![format!(
                "GET /acme/kev/resolve/{COMMIT}/Kev%204B%20Q8_0.gguf HTTP/1.1 0-{}",
                FIRST_RANGE_BYTES - 1
            )]
        );
    }

    #[tokio::test]
    async fn long_model_keys_are_read_in_further_ranges() {
        let long = "x".repeat(FIRST_RANGE_BYTES as usize);
        let file = gguf_header(
            &[
                ("general.architecture", "qwen35"),
                ("general.description", &long),
                ("qwen35.decision.type", "kev"),
            ],
            0,
        );
        let (base_url, server) = serve_ranges(file, 2).await;
        let temp = TempDir::new().unwrap();
        assert_eq!(
            read(&client(&temp, base_url), "kev.gguf").await.unwrap(),
            GgufHeaderEvidence::DecisionHead
        );
        let seen = server.await.unwrap();
        assert!(seen[1].ends_with(&format!(
            "{}-{}",
            FIRST_RANGE_BYTES,
            3 * FIRST_RANGE_BYTES - 1
        )));
    }

    #[tokio::test]
    async fn plain_causal_gguf_has_no_decision_head() {
        // Mica v0.5's GGUF: qwen35 keys straight into the tokenizer, no head.
        let file = gguf_header(
            &[
                ("general.architecture", "qwen35"),
                ("general.name", "Mica v0.5 4B"),
                ("tokenizer.ggml.model", "gpt2"),
            ],
            FIRST_RANGE_BYTES as usize,
        );
        let (base_url, server) = serve_ranges(file, 1).await;
        let temp = TempDir::new().unwrap();
        assert_eq!(
            read(&client(&temp, base_url), "mica.gguf").await.unwrap(),
            GgufHeaderEvidence::NoDecisionHead
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn oversized_metadata_string_is_unreadable_with_a_reason_not_allocated() {
        // A string claiming 4 GiB would otherwise be allocated or read.
        let mut file = b"GGUF".to_vec();
        file.extend_from_slice(&3_u32.to_le_bytes());
        file.extend_from_slice(&0_u64.to_le_bytes());
        file.extend_from_slice(&1_u64.to_le_bytes());
        file.extend_from_slice(&(4_u64 << 30).to_le_bytes());
        file.resize(FIRST_RANGE_BYTES as usize, 0);
        let (base_url, server) = serve_ranges(file, 1).await;
        let temp = TempDir::new().unwrap();
        match read(&client(&temp, base_url), "huge.gguf").await.unwrap() {
            GgufHeaderEvidence::Unreadable(reason) => {
                assert!(reason.contains("exceeds bounded limit"), "{reason}")
            }
            other => panic!("expected an unreadable header, got {other:?}"),
        }
        server.await.unwrap();
    }

    #[tokio::test]
    async fn full_response_to_a_range_is_refused_with_a_reason() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nGGUF")
                .await
                .unwrap();
        });
        let temp = TempDir::new().unwrap();
        let error = read(&client(&temp, base_url), "kev.gguf")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("partial-content response"));
        server.await.unwrap();
    }

    fn tree(files: &[&str]) -> RepoFileTree {
        RepoFileTree {
            repo_id: "acme/kev".into(),
            lfs_files: files
                .iter()
                .map(|filename| crate::model_library::types::LfsFileInfo {
                    filename: (*filename).into(),
                    size: 1,
                    sha256: "0".repeat(64),
                })
                .collect(),
            regular_files: vec!["config.json".into()],
            cached_at: String::new(),
            last_modified: None,
            cache_version: 0,
            regular_file_details: None,
        }
    }

    fn request() -> DownloadRequest {
        DownloadRequest {
            repo_id: "acme/kev".into(),
            family: "qwen35".into(),
            official_name: "Kev".into(),
            model_type: None,
            quant: Some("Q8_0".into()),
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
    fn candidates_are_the_top_level_primary_ggufs_the_download_selects() {
        let tree = tree(&[
            "Kev-4B-BF16.gguf",
            "Kev-4B-Q8_0.gguf",
            "mmproj-Kev-Q8_0.gguf",
            "nested/Kev-4B-Q8_0.gguf",
            "Big-Q8_0-00001-of-00002.gguf",
            "Big-Q8_0-00002-of-00002.gguf",
            "model.safetensors",
        ]);
        let mut request = request();
        assert_eq!(
            primary_gguf_candidates(&request, None, Some(&tree)),
            vec![
                "Big-Q8_0-00001-of-00002.gguf".to_string(),
                "Kev-4B-Q8_0.gguf".to_string()
            ]
        );
        request.quant = None;
        assert_eq!(
            primary_gguf_candidates(&request, None, Some(&tree)),
            vec![
                "Big-Q8_0-00001-of-00002.gguf".to_string(),
                "Kev-4B-BF16.gguf".to_string(),
                "Kev-4B-Q8_0.gguf".to_string()
            ]
        );
        request.filenames = Some(vec!["config.json".into(), "Kev-4B-BF16.gguf".into()]);
        assert_eq!(
            primary_gguf_candidates(&request, explicit_selected_files(&request), None),
            vec!["Kev-4B-BF16.gguf".to_string()]
        );
    }

    #[tokio::test]
    async fn the_first_decision_head_decides_and_errors_surface_without_one() {
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        let mut request = request();
        request.filenames = Some(vec!["a.gguf".into(), "b.gguf".into()]);
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        client.set_gguf_header_reader(ScriptedGgufHeaderReader::new(|path| {
            Ok(if path == "b.gguf" {
                GgufHeaderEvidence::DecisionHead
            } else {
                GgufHeaderEvidence::NoDecisionHead
            })
        }));
        assert_eq!(
            client
                .selected_gguf_decision_head(&request, &revision)
                .await
                .unwrap(),
            GgufHeaderEvidence::DecisionHead
        );
        client.set_gguf_header_reader(ScriptedGgufHeaderReader::new(|path| {
            if path == "b.gguf" {
                Err(PumasError::Network {
                    message: "fixture network failure".into(),
                    cause: None,
                })
            } else {
                Ok(GgufHeaderEvidence::NoDecisionHead)
            }
        }));
        assert!(client
            .selected_gguf_decision_head(&request, &revision)
            .await
            .unwrap_err()
            .to_string()
            .contains("fixture network failure"));
    }
}

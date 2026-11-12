//! Read a remote GGUF's metadata header before downloading it.
//!
//! A llama.cpp decision model declares `<arch>.decision.type` among its model
//! keys, ahead of the tokenizer arrays. Classification after download reads
//! that key from the local file; the download preflight reads the same key
//! with HTTP Range requests, so the model is placed by the same evidence.

use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::types::{DownloadRequest, RepoFileTree};
use crate::model_library::{probe_gguf_decision_head, GgufDecisionProbe};
use reqwest::header::{CONTENT_RANGE, RANGE};
use reqwest::StatusCode;

/// The first range covers the model keys of every GGUF seen so far; a longer
/// header is read in successively doubled ranges until its keys end.
const FIRST_RANGE_BYTES: u64 = 256 * 1024;

impl HuggingFaceClient {
    /// Whether any primary GGUF the request selects declares a decision head.
    /// `Ok(false)` when the selection has no GGUF.
    pub(crate) async fn selected_gguf_declares_decision_head(
        &self,
        request: &DownloadRequest,
        revision: &DownloadRevision,
    ) -> Result<bool> {
        #[cfg(test)]
        if !self.test_gguf_header_probe {
            return Ok(false);
        }
        let explicit = explicit_selected_files(request);
        let tree = if explicit.is_none() {
            Some(
                self.get_repo_files_at_revision(&request.repo_id, revision)
                    .await?,
            )
        } else {
            None
        };
        let candidates = primary_gguf_candidates(request, explicit, tree.as_ref());
        for path in candidates {
            if self
                .gguf_declares_decision_head_at_revision(&request.repo_id, &path, revision)
                .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Read only as much of one pinned GGUF as its model keys occupy.
    pub(crate) async fn gguf_declares_decision_head_at_revision(
        &self,
        repo_id: &str,
        path: &str,
        revision: &DownloadRevision,
    ) -> Result<bool> {
        super::metadata::validate_hf_repo_id(repo_id)?;
        let url = format!(
            "{}/{}/resolve/{}/{}",
            self.hub_base_url(),
            repo_id,
            revision.as_str(),
            path
        );
        let authorization = self.auth_header_value().await;
        let mut prefix: Vec<u8> = Vec::new();
        let mut range = FIRST_RANGE_BYTES;
        loop {
            let start = prefix.len() as u64;
            let end = start.saturating_add(range - 1);
            let mut request = self
                .client
                .get(&url)
                .header(RANGE, format!("bytes={start}-{end}"));
            if let Some(auth) = authorization.as_deref() {
                request = request.header("Authorization", auth);
            }
            let mut response = request.send().await.map_err(|error| PumasError::Network {
                message: format!("Failed to read the GGUF header of {repo_id}/{path}: {error}"),
                cause: Some(error.to_string()),
            })?;
            let total = match response.status() {
                StatusCode::PARTIAL_CONTENT => {
                    content_range_total(&response, start, repo_id, path)?
                }
                // The previous range ended exactly at the end of the file.
                StatusCode::RANGE_NOT_SATISFIABLE if start > 0 => return Ok(false),
                status => {
                    return Err(PumasError::Network {
                        message: format!(
                        "HuggingFace Hub returned {status} for a byte range of {repo_id}/{path}; \
                             reading its GGUF header without downloading the file needs a \
                             partial-content response"
                    ),
                        cause: None,
                    })
                }
            };
            let mut received = 0_u64;
            while received < range {
                let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|error| PumasError::Network {
                        message: format!(
                            "Failed to read the GGUF header of {repo_id}/{path}: {error}"
                        ),
                        cause: Some(error.to_string()),
                    })?
                else {
                    break;
                };
                let take = usize::try_from((range - received).min(chunk.len() as u64))
                    .unwrap_or(chunk.len());
                prefix.extend_from_slice(&chunk[..take]);
                received += take as u64;
            }
            drop(response);
            match probe_gguf_decision_head(&prefix) {
                GgufDecisionProbe::Declared(declared) => return Ok(declared),
                GgufDecisionProbe::NeedMore => {}
            }
            if received == 0 || total.is_some_and(|total| prefix.len() as u64 >= total) {
                // The whole file is read and its keys never ended.
                return Ok(false);
            }
            range = range.saturating_mul(2);
        }
    }
}

/// A `206` must answer the range that was asked for. Returns the file length
/// when the server states it.
fn content_range_total(
    response: &reqwest::Response,
    start: u64,
    repo_id: &str,
    path: &str,
) -> Result<Option<u64>> {
    let mismatch = || PumasError::Network {
        message: format!(
            "HuggingFace Hub answered a GGUF header range of {repo_id}/{path} with a \
             different range than the one requested"
        ),
        cause: None,
    };
    let value = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(mismatch)?;
    let (span, total) = value
        .strip_prefix("bytes ")
        .and_then(|rest| rest.split_once('/'))
        .ok_or_else(mismatch)?;
    let first = span
        .split_once('-')
        .and_then(|(first, _)| first.parse::<u64>().ok())
        .ok_or_else(mismatch)?;
    if first != start {
        return Err(mismatch());
    }
    Ok(total.parse::<u64>().ok())
}

/// Files the request names itself, before any repository tree is needed.
fn explicit_selected_files(request: &DownloadRequest) -> Option<Vec<String>> {
    if let Some(filenames) = &request.filenames {
        return Some(filenames.clone());
    }
    request.filename.as_ref().map(|name| vec![name.clone()])
}

/// The primary GGUF files a download selects, in the same priority as the
/// download's own selection: explicit names, then the quantization substring,
/// then every LFS file. Projectors (`mmproj`) are not primary models.
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
        .filter(|path| {
            let lower = path.to_lowercase();
            lower.ends_with(".gguf") && !lower.contains("mmproj")
        })
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn gguf_header(keys: &[(&str, &str)], tail: usize) -> Vec<u8> {
        let mut bytes = b"GGUF".to_vec();
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&(keys.len() as u64).to_le_bytes());
        for (key, value) in keys {
            bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
            bytes.extend_from_slice(key.as_bytes());
            bytes.extend_from_slice(&8_u32.to_le_bytes());
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
        bytes.resize(bytes.len() + tail, 0);
        bytes
    }

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
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        assert!(client(&temp, base_url)
            .gguf_declares_decision_head_at_revision("acme/kev", "Kev-4B-Q8_0.gguf", &revision)
            .await
            .unwrap());
        let seen = server.await.unwrap();
        assert_eq!(
            seen,
            vec![format!(
                "GET /acme/kev/resolve/{COMMIT}/Kev-4B-Q8_0.gguf HTTP/1.1 0-{}",
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
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        assert!(client(&temp, base_url)
            .gguf_declares_decision_head_at_revision("acme/kev", "kev.gguf", &revision)
            .await
            .unwrap());
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
                ("qwen35.block_count", "32"),
                ("tokenizer.ggml.model", "gpt2"),
            ],
            FIRST_RANGE_BYTES as usize,
        );
        let (base_url, server) = serve_ranges(file, 1).await;
        let temp = TempDir::new().unwrap();
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        assert!(!client(&temp, base_url)
            .gguf_declares_decision_head_at_revision("acme/mica", "mica.gguf", &revision)
            .await
            .unwrap());
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
        let revision = DownloadRevision::from_commit(COMMIT).unwrap();
        let error = client(&temp, base_url)
            .gguf_declares_decision_head_at_revision("acme/kev", "kev.gguf", &revision)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("partial-content response"));
        server.await.unwrap();
    }

    #[test]
    fn candidates_follow_the_download_selection_and_skip_projectors() {
        let tree = RepoFileTree {
            repo_id: "acme/kev".into(),
            lfs_files: [
                "Kev-4B-BF16.gguf",
                "Kev-4B-Q8_0.gguf",
                "mmproj-Kev-Q8_0.gguf",
                "model.safetensors",
            ]
            .into_iter()
            .map(|filename| crate::model_library::types::LfsFileInfo {
                filename: filename.into(),
                size: 1,
                sha256: "0".repeat(64),
            })
            .collect(),
            regular_files: vec!["config.json".into()],
            cached_at: String::new(),
            last_modified: None,
            cache_version: 0,
        };
        let mut request = DownloadRequest {
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
        };
        assert_eq!(
            primary_gguf_candidates(&request, None, Some(&tree)),
            vec!["Kev-4B-Q8_0.gguf".to_string()]
        );
        request.quant = None;
        assert_eq!(
            primary_gguf_candidates(&request, None, Some(&tree)),
            vec![
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
}

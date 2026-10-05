//! Optional S3 protocol reader. The caller owns staging, verification, retries,
//! cancellation, and publication through the shared acquisition lifecycle.

use std::{ops::Range, sync::Arc, time::Duration};

use futures::StreamExt;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder, AwsCredential},
    client::{HttpClient, HttpConnector},
    path::Path,
    ClientOptions, GetOptions, ObjectStore, RetryConfig, StaticCredentialProvider,
};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use url::Url;

use super::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
};

mod manifest;
pub use manifest::{S3ManifestEntry, S3ManifestSelection};

/// Endpoint interpretation. Virtual-hosted endpoints already include the bucket.
#[derive(Clone, Copy, Debug)]
pub enum S3Addressing {
    Path,
    VirtualHosted,
}

/// Explicit caller-authorized source configuration, never read from model metadata.
///
/// This first reader supports anonymous general-purpose/versioned buckets only.
/// Credentials, environment discovery, prefix listing, and remote writes are absent.
pub struct S3ReaderConfig {
    /// Absolute HTTP(S) origin, without credentials, query, fragment, or path.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub addressing: S3Addressing,
    /// Explicit opt-in for caller-authorized HTTP sources, including local fixtures.
    pub allow_http: bool,
    /// Budget for each complete selection/read operation, including destination writes.
    pub operation_timeout: Duration,
}

/// One configured protocol reader. It owns no tasks, runtime, or durable state.
pub struct S3Reader {
    store: Arc<AmazonS3>,
    endpoint: Url,
    bucket: String,
    addressing: S3Addressing,
    timeout: Duration,
}

/// Version, validator, and byte-size evidence observed from a selected object.
/// A selection binds its reads to the exact reader and cannot be forged by callers.
#[derive(Clone)]
pub struct S3ObjectSelection {
    store: Arc<AmazonS3>,
    key: Path,
    version: String,
    etag: String,
    size: u64,
    timeout: Duration,
    manifest: ArtifactManifest,
}

/// Reader failures do not publish or verify an artifact. The caller must retain
/// custody of any partially written destination and decide cleanup or retry.
#[derive(Debug, thiserror::Error)]
pub enum S3ReaderError {
    #[error("invalid S3 source configuration: {0}")]
    Configuration(&'static str),
    #[error("invalid S3 selection: {0}")]
    Manifest(#[from] ManifestValidationError),
    #[error("S3 client construction failed: {0}")]
    Client(#[from] reqwest::Error),
    #[error("selected S3 object version, validator, size, or range changed")]
    Changed,
    #[error("S3 object or selected version is unavailable")]
    Unavailable,
    #[error("S3 protocol request failed: {0}")]
    Protocol(String),
    #[error("S3 range must be nonempty and within the selected object")]
    InvalidRange,
    #[error("S3 body did not match the requested byte count")]
    BodyLength,
    #[error("S3 operation exceeded its caller-supplied budget")]
    TimedOut,
    #[error("S3 destination write failed: {0}")]
    Write(#[from] std::io::Error),
}

#[derive(Debug)]
struct ScopedConnector(reqwest::Client);

impl HttpConnector for ScopedConnector {
    fn connect(&self, _options: &ClientOptions) -> object_store::Result<HttpClient> {
        Ok(HttpClient::new(self.0.clone()))
    }
}

impl S3Reader {
    /// Construct a reader from explicit endpoint authority. Redirects, proxy
    /// discovery, automatic retries, and ambient credential discovery are disabled.
    pub fn new(config: S3ReaderConfig) -> Result<Self, S3ReaderError> {
        let endpoint = Url::parse(&config.endpoint)
            .map_err(|_| S3ReaderError::Configuration("endpoint must be an absolute origin"))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
        {
            return Err(S3ReaderError::Configuration(
                "endpoint must be a credential-free HTTP(S) origin",
            ));
        }
        if endpoint.scheme() == "http" && !config.allow_http {
            return Err(S3ReaderError::Configuration(
                "HTTP requires explicit source authority",
            ));
        }
        if config.region.is_empty() || config.region.chars().any(char::is_control) {
            return Err(S3ReaderError::Configuration("region must be nonempty text"));
        }
        // Only ordinary bucket names, not ARN/access-point or directory-bucket selectors.
        if !(3..=63).contains(&config.bucket.len())
            || !config
                .bucket
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
            || !config.bucket.as_bytes()[0].is_ascii_alphanumeric()
            || !config.bucket.as_bytes()[config.bucket.len() - 1].is_ascii_alphanumeric()
            || config.bucket.contains("..")
            || config.bucket.ends_with("--x-s3")
        {
            return Err(S3ReaderError::Configuration("unsupported bucket name"));
        }
        if config.operation_timeout.is_zero() {
            return Err(S3ReaderError::Configuration(
                "operation budget must be positive",
            ));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .https_only(!config.allow_http)
            .timeout(config.operation_timeout)
            .connect_timeout(config.operation_timeout)
            .build()?;
        let store = AmazonS3Builder::new()
            .with_bucket_name(&config.bucket)
            .with_region(config.region)
            .with_endpoint(endpoint.as_str().trim_end_matches('/'))
            .with_virtual_hosted_style_request(matches!(
                config.addressing,
                S3Addressing::VirtualHosted
            ))
            .with_allow_http(config.allow_http)
            .with_skip_signature(true)
            // Prevent even constructing an ambient/metadata credential provider.
            .with_credentials(Arc::new(StaticCredentialProvider::new(AwsCredential {
                key_id: String::new(),
                secret_key: String::new(),
                token: None,
            })))
            .with_http_connector(ScopedConnector(client))
            .with_retry(RetryConfig {
                max_retries: 0,
                ..RetryConfig::default()
            })
            .build()
            .map_err(protocol_error)?;
        Ok(Self {
            store: Arc::new(store),
            endpoint,
            bucket: config.bucket,
            addressing: config.addressing,
            timeout: config.operation_timeout,
        })
    }

    /// Resolve one explicitly selected immutable VersionId using HEAD. `null`
    /// versions are mutable and refused. The supplied SHA-256 remains declared
    /// evidence until the shared verifier checks complete downloaded bytes.
    pub async fn select(
        &self,
        source_key: &str,
        version: &str,
        logical_path: &str,
        expected_sha256: Sha256Evidence,
    ) -> Result<S3ObjectSelection, S3ReaderError> {
        let (key, source) = self.validated_object(source_key, version)?;
        let file = ArtifactFile::new(
            logical_path,
            source_key,
            None,
            Some(expected_sha256.clone()),
            FileVerificationRequirement::Sha256,
        )?;
        let result = tokio::time::timeout(
            self.timeout,
            self.store.get_opts(
                &key,
                GetOptions {
                    version: Some(version.to_owned()),
                    head: true,
                    ..GetOptions::default()
                },
            ),
        )
        .await
        .map_err(|_| S3ReaderError::TimedOut)?
        .map_err(protocol_error)?;
        if result.meta.version.as_deref() != Some(version) {
            return Err(S3ReaderError::Changed);
        }
        let etag = result
            .meta
            .e_tag
            .filter(|tag| {
                tag.len() >= 2
                    && tag.starts_with('"')
                    && tag.ends_with('"')
                    && tag[1..tag.len() - 1]
                        .bytes()
                        .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b) || b >= 0x80)
            })
            .ok_or(S3ReaderError::Changed)?;
        let manifest = ArtifactManifest::new(
            source,
            vec![ArtifactFile::new(
                file.logical_path(),
                source_key,
                Some(result.meta.size),
                Some(expected_sha256),
                FileVerificationRequirement::Sha256,
            )?],
        )?;
        Ok(S3ObjectSelection {
            store: Arc::clone(&self.store),
            key,
            version: version.to_owned(),
            etag,
            size: result.meta.size,
            timeout: self.timeout,
            manifest,
        })
    }
}

impl S3ObjectSelection {
    pub fn manifest(&self) -> &ArtifactManifest {
        &self.manifest
    }

    /// Stream a nonempty half-open range into caller-owned staging. VersionId
    /// and If-Match are sent together; response metadata is checked before writes.
    /// Dropping this future stops polling reader I/O; remote completion is not
    /// implied. Partial writes remain the caller's cleanup responsibility.
    pub async fn read_range<W: AsyncWrite + Unpin>(
        &self,
        range: Range<u64>,
        destination: &mut W,
    ) -> Result<u64, S3ReaderError> {
        if range.start >= range.end || range.end > self.size {
            return Err(S3ReaderError::InvalidRange);
        }
        tokio::time::timeout(self.timeout, async {
            let result = self.open_checked_range(range.clone()).await?;
            let expected = range.end - range.start;
            let mut written = 0;
            let mut stream = result.into_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(protocol_error)?;
                let count = chunk.len() as u64;
                if count > expected - written {
                    return Err(S3ReaderError::BodyLength);
                }
                destination.write_all(&chunk).await?;
                written += count;
            }
            if written != expected {
                return Err(S3ReaderError::BodyLength);
            }
            Ok(written)
        })
        .await
        .map_err(|_| S3ReaderError::TimedOut)?
    }

    async fn open_checked_range(
        &self,
        range: Range<u64>,
    ) -> Result<object_store::GetResult, S3ReaderError> {
        if range.start >= range.end || range.end > self.size {
            return Err(S3ReaderError::InvalidRange);
        }
        let result = self
            .store
            .get_opts(
                &self.key,
                GetOptions {
                    version: Some(self.version.clone()),
                    if_match: Some(self.etag.clone()),
                    range: Some(range.clone().into()),
                    ..GetOptions::default()
                },
            )
            .await
            .map_err(protocol_error)?;
        if result.meta.version.as_deref() != Some(&self.version)
            || result.meta.e_tag.as_deref() != Some(&self.etag)
            || result.meta.size != self.size
            || result.range != range
        {
            return Err(S3ReaderError::Changed);
        }
        Ok(result)
    }

    pub(crate) fn acquisition_identity(&self) -> String {
        format!(
            "{}:{}",
            self.manifest.source().source_id(),
            hex::encode(&self.version)
        )
    }

    /// Project checked protocol bytes into the existing lifecycle's streaming
    /// boundary. Continuation is supplied only by its live prefix owner.
    pub(crate) async fn open_acquisition(
        &self,
        resume: u64,
        continuation: Option<&super::http::HttpResumeEvidence>,
        transfer_deadline: Option<tokio::time::Instant>,
    ) -> crate::Result<super::http::HttpArtifactResponse> {
        let resource = self.acquisition_identity();
        if resume > 0
            && !continuation.is_some_and(|proof| {
                proof.resource == resource
                    && proof.etag == self.etag
                    && proof.total == Some(self.size)
            })
        {
            return Err(acquisition_error(S3ReaderError::Changed));
        }
        let attempt_deadline = tokio::time::Instant::now()
            .checked_add(self.timeout)
            .ok_or_else(|| {
                acquisition_error(S3ReaderError::Configuration(
                    "operation budget exceeds the supported clock range",
                ))
            })?;
        let deadline =
            transfer_deadline.map_or(attempt_deadline, |limit| limit.min(attempt_deadline));
        let result = tokio::time::timeout_at(deadline, self.open_checked_range(resume..self.size))
            .await
            .map_err(|_| acquisition_error(S3ReaderError::TimedOut))?
            .map_err(acquisition_error)?;
        Ok(super::http::HttpArtifactResponse {
            body: result
                .into_stream()
                .map(|chunk| chunk.map_err(protocol_error).map_err(acquisition_error))
                .boxed(),
            resumed: resume > 0,
            total_size: Some(self.size),
            resource,
            strong_etag: Some(self.etag.clone()),
            deadline: Some(deadline),
        })
    }
}

fn acquisition_error(error: S3ReaderError) -> crate::PumasError {
    match error {
        S3ReaderError::TimedOut | S3ReaderError::Protocol(_) | S3ReaderError::Client(_) => {
            crate::PumasError::Network {
                message: "S3 acquisition protocol failed".into(),
                cause: Some(error.to_string()),
            }
        }
        S3ReaderError::Unavailable => crate::PumasError::DownloadFailed {
            url: "S3 selected object".into(),
            message: error.to_string(),
        },
        _ => crate::PumasError::Validation {
            field: "acquisition.s3".into(),
            message: error.to_string(),
        },
    }
}

fn protocol_error(error: object_store::Error) -> S3ReaderError {
    match error {
        object_store::Error::Precondition { .. } => S3ReaderError::Changed,
        object_store::Error::NotFound { .. } => S3ReaderError::Unavailable,
        other => S3ReaderError::Protocol(other.to_string()),
    }
}

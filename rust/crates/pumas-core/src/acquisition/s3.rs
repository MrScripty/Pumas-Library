//! Optional S3 protocol reader. The caller owns staging, verification, retries,
//! cancellation, and publication through the shared acquisition lifecycle.

use std::{ops::Range, sync::Arc, time::Duration};

use aws_credential_types::Credentials;
use futures::StreamExt;
use sdk::{protocol_error, Path};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use url::Url;

use super::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
};

mod manifest;
mod sdk;
pub use manifest::{S3ManifestEntry, S3ManifestSelection};

#[cfg(test)]
mod auth_tests;

/// Endpoint interpretation. Virtual-hosted endpoints already include the bucket.
#[derive(Clone, Copy, Debug)]
pub enum S3Addressing {
    Path,
    VirtualHosted,
}

/// Explicit caller-authorized source configuration, never read from model metadata.
///
/// General-purpose/versioned buckets only. Environment discovery, prefix listing,
/// and remote writes are absent. Authentication is supplied separately in memory.
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

/// Explicit credentials for one bounded acquisition. No discovery, persistence,
/// serialization, or refresh is performed. Reader selections retain the credentials
/// only while their in-memory protocol capability remains alive.
pub struct S3Credentials(Credentials);

impl S3Credentials {
    /// Accept nonempty printable ASCII credentials without whitespace. Access-key
    /// IDs must also exclude SigV4 credential-field delimiters (`/`, `,`, `=`).
    /// Validation errors never include the supplied values.
    pub fn new(
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    ) -> Result<Self, S3ReaderError> {
        let valid = |value: &str| {
            !value.is_empty() && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        };
        if !valid(&access_key_id)
            || access_key_id.contains(['/', ',', '='])
            || !valid(&secret_access_key)
            || session_token.as_deref().is_some_and(|token| !valid(token))
        {
            return Err(S3ReaderError::Configuration("invalid explicit credentials"));
        }
        Ok(Self(Credentials::new(
            access_key_id,
            secret_access_key,
            session_token,
            None,
            "request-scoped",
        )))
    }
}

impl std::fmt::Debug for S3Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("S3Credentials([REDACTED])")
    }
}

enum Authentication {
    Anonymous,
    Explicit(S3Credentials),
    #[cfg(test)]
    LoopbackFixture(S3Credentials),
}

/// One configured protocol reader. It owns no tasks, runtime, or durable state.
pub struct S3Reader {
    store: Arc<aws_sdk_s3::Client>,
    endpoint: Url,
    bucket: String,
    addressing: S3Addressing,
    timeout: Duration,
}

/// Version, validator, and byte-size evidence observed from a selected object.
/// A selection binds its reads to the exact reader and cannot be forged by callers.
#[derive(Clone)]
pub struct S3ObjectSelection {
    store: Arc<aws_sdk_s3::Client>,
    key: Path,
    bucket: String,
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

impl S3Reader {
    /// Pure complete-set structural preflight, without HEAD, tasks or staging.
    /// Uses the same object identity and shared manifest validators as selection.
    /// This does not establish object existence, size, or digest correctness.
    pub fn validate_manifest_entries(
        &self,
        entries: &[S3ManifestEntry],
    ) -> Result<(), S3ReaderError> {
        let mut entries: Vec<_> = entries.iter().collect();
        entries.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        let mut pins = Vec::with_capacity(entries.len());
        let mut files = Vec::with_capacity(entries.len());
        for entry in entries {
            let (_, source) = self.validated_object(&entry.source_key, &entry.version)?;
            pins.push(source);
            files.push(ArtifactFile::new(
                &entry.logical_path,
                serde_json::to_string(&(&entry.source_key, &entry.version))
                    .map_err(|_| S3ReaderError::Configuration("explicit pin encoding failed"))?,
                None,
                Some(entry.expected_sha256.clone()),
                FileVerificationRequirement::Sha256,
            )?);
        }
        // Exact per-object pins enforce the existing encoded revision budget;
        // this temporary source label grants no selection or access authority.
        let source = ArtifactSourceIdentity::new(
            "s3",
            "explicit.preflight",
            ArtifactRevisionEvidence::new(
                "s3.explicit_versions",
                serde_json::to_string(&pins)
                    .map_err(|_| S3ReaderError::Configuration("explicit pin encoding failed"))?,
                RevisionStrength::Immutable,
            )?,
        )?;
        ArtifactManifest::new(source, files)
            .map(|_| ())
            .map_err(Into::into)
    }

    /// Construct a reader from explicit endpoint authority. Redirects, proxy
    /// discovery, automatic retries, and ambient credential discovery are disabled.
    pub fn new(config: S3ReaderConfig) -> Result<Self, S3ReaderError> {
        Self::build(config, Authentication::Anonymous)
    }

    /// Sign HEAD and conditional range GET with explicitly supplied credentials.
    /// HTTPS is mandatory even when `config.allow_http` is true. Credentials are
    /// consumed, remain in memory, and never participate in artifact identity.
    pub fn new_authenticated(
        config: S3ReaderConfig,
        credentials: S3Credentials,
    ) -> Result<Self, S3ReaderError> {
        Self::build(config, Authentication::Explicit(credentials))
    }

    fn build(
        config: S3ReaderConfig,
        authentication: Authentication,
    ) -> Result<Self, S3ReaderError> {
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
        let (credential, allow_http) = match authentication {
            Authentication::Anonymous => (None, config.allow_http),
            Authentication::Explicit(credentials) => {
                if endpoint.scheme() != "https" {
                    return Err(S3ReaderError::Configuration(
                        "authenticated S3 requires HTTPS",
                    ));
                }
                (Some(credentials.0), false)
            }
            #[cfg(test)]
            Authentication::LoopbackFixture(credentials) => {
                if endpoint.scheme() != "http"
                    || !endpoint.host().is_some_and(|host| match host {
                        url::Host::Ipv4(ip) => ip.is_loopback(),
                        url::Host::Ipv6(ip) => ip.is_loopback(),
                        url::Host::Domain(_) => false,
                    })
                {
                    return Err(S3ReaderError::Configuration(
                        "fixture requires literal loopback HTTP",
                    ));
                }
                (Some(credentials.0), true)
            }
        };
        if config.region.is_empty() || config.region.chars().any(char::is_control) {
            return Err(S3ReaderError::Configuration("region must be nonempty text"));
        }
        if credential.is_some()
            && !config
                .region
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(S3ReaderError::Configuration("invalid signing region"));
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
            .https_only(!allow_http)
            .timeout(config.operation_timeout)
            .connect_timeout(config.operation_timeout)
            .build()?;
        let store = sdk::build(&config, &endpoint, client, credential);
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
            sdk::scoped(async {
                self.store
                    .head_object()
                    .bucket(&self.bucket)
                    .key(key.as_ref())
                    .version_id(version)
                    .send()
                    .await
            }),
        )
        .await
        .map_err(|_| S3ReaderError::TimedOut)?
        .map_err(protocol_error)?;
        if result.version_id() != Some(version) {
            return Err(S3ReaderError::Changed);
        }
        let size = result
            .content_length()
            .filter(|size| *size >= 0)
            .ok_or_else(sdk::protocol_failure)? as u64;
        let etag = result
            .e_tag()
            .filter(|tag| {
                tag.len() >= 2
                    && tag.starts_with('"')
                    && tag.ends_with('"')
                    && tag[1..tag.len() - 1]
                        .bytes()
                        .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b) || b >= 0x80)
            })
            .ok_or(S3ReaderError::Changed)?
            .to_owned();
        let manifest = ArtifactManifest::new(
            source,
            vec![ArtifactFile::new(
                file.logical_path(),
                source_key,
                Some(size),
                Some(expected_sha256),
                FileVerificationRequirement::Sha256,
            )?],
        )?;
        Ok(S3ObjectSelection {
            store: Arc::clone(&self.store),
            key,
            bucket: self.bucket.clone(),
            version: version.to_owned(),
            etag,
            size,
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
            let mut stream = sdk::body_stream(result);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
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
    ) -> Result<aws_smithy_types::byte_stream::ByteStream, S3ReaderError> {
        if range.start >= range.end || range.end > self.size {
            return Err(S3ReaderError::InvalidRange);
        }
        let result = sdk::scoped(async {
            self.store
                .get_object()
                .bucket(&self.bucket)
                .key(self.key.as_ref())
                .version_id(&self.version)
                .if_match(&self.etag)
                .range(format!("bytes={}-{}", range.start, range.end - 1))
                .send()
                .await
        })
        .await
        .map_err(protocol_error)?;
        let (actual, size) = result
            .content_range()
            .and_then(parse_content_range)
            .ok_or_else(sdk::protocol_failure)?;
        if result.content_length().is_none_or(|size| size < 0) {
            return Err(sdk::protocol_failure());
        }
        if result.version_id() != Some(&self.version)
            || result.e_tag() != Some(&self.etag)
            || size != self.size
            || actual != range
        {
            return Err(S3ReaderError::Changed);
        }
        Ok(result.body)
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
        let body = if self.size == 0 && resume == 0 {
            // Selection already checked HEAD for this immutable VersionId and
            // a known size of zero. There is no valid byte range to request.
            // Still pass through the owner's writer and SHA-256 verifier: HEAD
            // is identity/length evidence, not a verified-file receipt.
            futures::stream::empty().boxed()
        } else {
            let result =
                tokio::time::timeout_at(deadline, self.open_checked_range(resume..self.size))
                    .await
                    .map_err(|_| acquisition_error(S3ReaderError::TimedOut))?
                    .map_err(acquisition_error)?;
            sdk::body_stream(result)
                .map(|chunk| chunk.map_err(acquisition_error))
                .boxed()
        };
        Ok(super::http::HttpArtifactResponse {
            body,
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

fn parse_content_range(value: &str) -> Option<(Range<u64>, u64)> {
    let (range, size) = value.trim().strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse().ok()?;
    let end: u64 = end.parse().ok()?;
    Some((start..end.checked_add(1)?, size.parse().ok()?))
}

#[cfg(test)]
mod bundle_preflight_tests {
    use super::*;
    fn reader() -> S3Reader {
        S3Reader::new(S3ReaderConfig {
            endpoint: "https://source.invalid".into(),
            region: "fixture-region".into(),
            bucket: "fixture-bucket".into(),
            addressing: S3Addressing::Path,
            allow_http: false,
            operation_timeout: Duration::from_secs(1),
        })
        .unwrap()
    }
    fn entry(path: &str, version: &str, hash: &str) -> S3ManifestEntry {
        S3ManifestEntry {
            source_key: "models/exact key".into(),
            version: version.into(),
            logical_path: path.into(),
            expected_sha256: Sha256Evidence::new("caller.sha256", hash).unwrap(),
        }
    }
    #[tokio::test]
    async fn complete_set_preflight_matches_selection_structural_refusal_without_io() {
        let reader = reader();
        let hash = "a".repeat(64);
        let good = entry("weights.gguf", "v1", &hash);
        for entries in [
            vec![],
            vec![good.clone(), good.clone()],
            vec![good.clone(), entry("WEIGHTS.GGUF", "v2", &hash)],
            vec![
                good.clone(),
                entry("weights.gguf.part/data.json", "v2", &hash),
            ],
            vec![good.clone(), entry("../data.json", "v2", &hash)],
            vec![good.clone(), entry("data.json", "v1", &"b".repeat(64))],
            vec![good.clone(), entry("data.json", &"x".repeat(16000), &hash)],
        ] {
            let preflight = reader.validate_manifest_entries(&entries).unwrap_err();
            let selection = reader.select_manifest(entries).await.err().unwrap();
            assert!(matches!(
                preflight,
                S3ReaderError::Configuration(_) | S3ReaderError::Manifest(_)
            ));
            assert_eq!(
                std::mem::discriminant(&preflight),
                std::mem::discriminant(&selection)
            );
        }
        assert!(reader
            .validate_manifest_entries(&[good, entry("data.json", "v2", &"b".repeat(64))])
            .is_ok());
    }
}

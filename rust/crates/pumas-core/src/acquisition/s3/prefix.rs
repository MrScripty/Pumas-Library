//! Bounded discovery, not package snapshot or artifact acquisition authority.
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{list_xml::ListXmlGuard, *};

const MAX_PAGE_BYTES: usize = 1024 * 1024;

/// Prefix failure never grants a partial selection. The existing reader error
/// and diagnostic policy stay intact; capacity refusal is distinguishable.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum S3PrefixError {
    #[error(transparent)]
    Reader(#[from] S3ReaderError),
    #[error("S3 prefix enumeration is incomplete: {0}")]
    Incomplete(&'static str),
}

/// Explicit capacity for one complete enumeration and immutable HEAD pinning.
/// The reader's existing operation timeout bounds the entire call. Work is at
/// most `max_pages` list requests plus `max_objects` sequential HEAD requests.
/// XML additionally retains the reviewed 4096-node/no-DTD guard.
#[derive(Clone, Copy, Debug)]
pub struct S3PrefixLimits {
    /// Requested keys per page, from 1 through S3's maximum of 1000.
    pub page_size: u16,
    pub max_pages: u32,
    pub max_objects: usize,
    /// Wire XML bytes per response, positive and at most 1 MiB.
    pub max_page_bytes: usize,
    /// Sum of wire XML bytes for all successfully decoded listing pages.
    pub max_total_bytes: usize,
}

/// Observed immutable version of one listed object, with no access capability.
/// The ETag is a conditional validator, not a digest. No serialization is offered.
pub struct S3PrefixObject {
    key: String,
    version: String,
    etag: String,
    size: u64,
}
impl S3PrefixObject {
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn etag(&self) -> &str {
        &self.etag
    }
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Declare a file for the existing explicit manifest owner. The caller
    /// chooses its logical path and SHA-256; this does not verify content or
    /// admit acquisition, and ordinary `select_manifest` validation still applies.
    pub fn manifest_entry(
        &self,
        logical_path: String,
        expected_sha256: Sha256Evidence,
    ) -> S3ManifestEntry {
        S3ManifestEntry {
            source_key: self.key.clone(),
            version: self.version.clone(),
            logical_path,
            expected_sha256,
        }
    }
}
impl std::fmt::Debug for S3PrefixObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("S3PrefixObject([REDACTED])")
    }
}

/// Complete pagination followed by checked immutable object observations.
/// This is not an atomic multi-object snapshot. Missing/racing objects, failed
/// pages and exhausted capacity return errors, never a shorter successful set.
pub struct S3PrefixListing {
    objects: Vec<S3PrefixObject>,
    pages: u32,
    xml_bytes: usize,
}
impl S3PrefixListing {
    /// Exact keys in lexicographical order, each with a conditional HEAD pin.
    pub fn objects(&self) -> &[S3PrefixObject] {
        &self.objects
    }
    pub fn pages(&self) -> u32 {
        self.pages
    }
    pub fn xml_bytes(&self) -> usize {
        self.xml_bytes
    }
}
impl std::fmt::Debug for S3PrefixListing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3PrefixListing")
            .field("objects", &self.objects.len())
            .field("pages", &self.pages)
            .field("xml_bytes", &self.xml_bytes)
            .finish()
    }
}

impl S3Reader {
    /// Enumerate an explicitly authorized raw prefix, preserving its bytes.
    /// Sequential SDK requests use the same endpoint, credentials, closed
    /// transport, diagnostic scope and caller timeout as existing reads.
    /// No request payer, delimiter, encoding conversion or automatic retry is
    /// enabled. Directory buckets/access points remain unsupported.
    ///
    /// Only an explicit final IsTruncated=false ends enumeration. Opaque tokens
    /// remain local and disappear on return/cancellation. Every listed object's
    /// current HEAD must match its size/ETag and report an immutable VersionId.
    /// No file is downloaded, selected manifest published or importer invoked.
    pub async fn enumerate_prefix(
        &self,
        prefix: &str,
        limits: S3PrefixLimits,
    ) -> Result<S3PrefixListing, S3PrefixError> {
        if prefix.len() > 1024 || prefix.chars().any(char::is_control) {
            return Err(S3ReaderError::Configuration("unsupported S3 prefix").into());
        }
        if limits.page_size == 0
            || limits.page_size > 1000
            || limits.max_pages == 0
            || limits.max_objects == 0
            || limits.max_page_bytes == 0
            || limits.max_page_bytes > MAX_PAGE_BYTES
            || limits.max_total_bytes == 0
        {
            return Err(S3ReaderError::Configuration("invalid S3 prefix capacity").into());
        }
        tokio::time::timeout(self.timeout, self.enumerate_checked(prefix, limits))
            .await
            .map_err(|_| S3ReaderError::TimedOut)?
    }

    async fn enumerate_checked(
        &self,
        prefix: &str,
        limits: S3PrefixLimits,
    ) -> Result<S3PrefixListing, S3PrefixError> {
        let mut objects = Vec::<S3PrefixObject>::new();
        let mut token = None;
        let mut tokens = std::collections::HashSet::new();
        let mut pages = 0;
        let mut xml_bytes = 0;
        loop {
            if pages == limits.max_pages {
                return Err(S3PrefixError::Incomplete("page bound exhausted"));
            }
            let remaining_objects = limits.max_objects - objects.len();
            if remaining_objects == 0 {
                return Err(S3PrefixError::Incomplete("object bound exhausted"));
            }
            let remaining_bytes = limits.max_total_bytes - xml_bytes;
            if remaining_bytes == 0 {
                return Err(S3PrefixError::Incomplete("XML byte bound exhausted"));
            }
            let max_keys = usize::from(limits.page_size).min(remaining_objects) as i32;
            let observed_bytes = Arc::new(AtomicUsize::new(0));
            let output = sdk::scoped(
                self.store
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(prefix)
                    .max_keys(max_keys)
                    .set_continuation_token(token.clone())
                    .customize()
                    .interceptor(ListXmlGuard {
                        max_bytes: limits.max_page_bytes.min(remaining_bytes),
                        observed_bytes: Arc::clone(&observed_bytes),
                    })
                    .send(),
            )
            .await
            .map_err(sdk::prefix_error)?;
            pages += 1;
            xml_bytes += observed_bytes.load(Ordering::Acquire);
            if output.name() != Some(self.bucket.as_str())
                || output.prefix() != Some(prefix)
                || output.continuation_token() != token.as_deref()
                || output.max_keys() != Some(max_keys)
                || output.key_count() != Some(output.contents().len() as i32)
                || output.contents().len() > max_keys as usize
                || !output.common_prefixes().is_empty()
                || output.delimiter().is_some()
                || output.start_after().is_some()
                || output.encoding_type().is_some()
            {
                return Err(sdk::protocol_failure().into());
            }
            for object in output.contents() {
                let key = object.key().ok_or_else(sdk::protocol_failure)?;
                let path = Path::parse(key).map_err(|_| sdk::protocol_failure())?;
                if !key.starts_with(prefix)
                    || key.is_empty()
                    || key.len() > 1024
                    || path.as_ref() != key
                {
                    return Err(sdk::protocol_failure().into());
                }
                if objects.last().is_some_and(|last| last.key.as_str() >= key) {
                    return Err(S3ReaderError::Changed.into());
                }
                let etag = object
                    .e_tag()
                    .filter(|etag| {
                        etag.len() >= 2
                            && etag.starts_with('"')
                            && etag.ends_with('"')
                            && etag[1..etag.len() - 1]
                                .bytes()
                                .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b) || b >= 0x80)
                    })
                    .ok_or(S3ReaderError::Changed)?;
                let size = object
                    .size()
                    .filter(|size| *size >= 0)
                    .ok_or_else(sdk::protocol_failure)? as u64;
                objects.push(S3PrefixObject {
                    key: key.into(),
                    version: String::new(),
                    etag: etag.into(),
                    size,
                });
            }
            match (output.is_truncated(), output.next_continuation_token()) {
                (Some(false), None) => break,
                (Some(true), Some(next)) if !next.is_empty() && tokens.insert(next.to_owned()) => {
                    token = Some(next.to_owned());
                }
                _ => return Err(sdk::protocol_failure().into()),
            }
        }
        // Do not pin any objects before pagination completes. A failed/incomplete
        // page must not produce a partial immutable selection or a ready handoff.
        for object in &mut objects {
            let head = sdk::scoped(
                self.store
                    .head_object()
                    .bucket(&self.bucket)
                    .key(&object.key)
                    .if_match(&object.etag)
                    .send(),
            )
            .await
            .map_err(protocol_error)?;
            if head.e_tag() != Some(object.etag.as_str())
                || head.content_length() != Some(object.size as i64)
            {
                return Err(S3ReaderError::Changed.into());
            }
            let version = head.version_id().ok_or(S3ReaderError::Changed)?;
            self.validated_object(&object.key, version)
                .map_err(|_| S3ReaderError::Changed)?;
            object.version = version.to_owned();
        }
        Ok(S3PrefixListing {
            objects,
            pages,
            xml_bytes,
        })
    }
}

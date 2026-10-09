//! Owner-materialized local input capabilities, without install/execute authority.
//! A cooperating input owner supplies the lease; this is not protection against
//! hostile same-user mutation. Actual selected bytes still require SHA256 verification.
use super::http::HttpArtifactResponse;
use super::service::{invalid, owned};
use super::task_custody::TaskContext;
use super::ArtifactFile;
use crate::{PumasError, Result};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::sync::Arc;

struct LocalInput {
    file: File,
    _lease: Arc<dyn Send + Sync>,
}

/// A held regular file and its cooperative owner lease. No ambient path, URL,
/// JSON manifest or submitted digest can construct file custody. This capability
/// remains owned by registered read effects even if their waiter is cancelled.
/// The lease is an input keepalive, not proof of immutability, a no-symlink
/// pathname origin, or an execution lease. No original pathname is consulted.
#[derive(Clone)]
pub struct AcquisitionLocalSource(Arc<LocalInput>);

impl std::fmt::Debug for AcquisitionLocalSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcquisitionLocalSource")
            .finish_non_exhaustive()
    }
}

impl AcquisitionLocalSource {
    pub fn new(file: File, lease: Arc<dyn Send + Sync>) -> Result<Self> {
        if !cfg!(target_os = "linux") {
            return Err(invalid("Local acquisition supports Linux only"));
        }
        if !file.metadata()?.is_file() {
            return Err(invalid("Local input capability is not a regular file"));
        }
        Ok(Self(Arc::new(LocalInput {
            file,
            _lease: lease,
        })))
    }

    pub(crate) async fn verify(
        &self,
        context: &TaskContext,
        selected: &ArtifactFile,
        deadline: tokio::time::Instant,
    ) -> Result<()> {
        let size = selected
            .expected_size()
            .ok_or_else(|| invalid("Local selection requires exact size"))?;
        let expected = selected
            .expected_sha256()
            .ok_or_else(|| invalid("Local selection requires SHA256 evidence"))?
            .value();
        let input = self.0.clone();
        owned(context, "validate local acquisition size", move || {
            require_budget(deadline)?;
            check_size(&input.file, size)
        })
        .await?;
        let mut sha = Sha256::new();
        let mut offset = 0u64;
        while offset < size {
            require_budget(deadline)?;
            let input = self.0.clone();
            let chunk = owned(context, "verify local acquisition input", move || {
                require_budget(deadline)?;
                check_size(&input.file, size)?;
                let capacity = (size - offset).min(65536) as usize;
                let mut bytes = vec![0; capacity];
                let count = read_at(&input.file, &mut bytes, offset, deadline)?;
                if count == 0 {
                    return Err(invalid("Local input ended before selected size"));
                }
                bytes.truncate(count);
                Ok(bytes)
            })
            .await?;
            sha.update(&chunk);
            offset = offset
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| invalid("Local read offset overflow"))?;
        }
        require_budget(deadline)?;
        let input = self.0.clone();
        owned(context, "revalidate local acquisition size", move || {
            require_budget(deadline)?;
            check_size(&input.file, size)
        })
        .await?;
        if hex::encode(sha.finalize()) != expected {
            return Err(invalid("Local input digest differs from selection"));
        }
        Ok(())
    }

    pub(crate) async fn open(
        &self,
        context: &TaskContext,
        selected: &ArtifactFile,
        deadline: tokio::time::Instant,
    ) -> Result<HttpArtifactResponse> {
        let size = selected
            .expected_size()
            .ok_or_else(|| invalid("Local selection requires exact size"))?;
        let digest = selected
            .expected_sha256()
            .ok_or_else(|| invalid("Local selection requires SHA256 evidence"))?
            .value();
        let input = self.0.clone();
        owned(context, "validate local acquisition input", move || {
            require_budget(deadline)?;
            check_size(&input.file, size)
        })
        .await?;
        let context = context.clone();
        let input = self.0.clone();
        let body = futures::stream::try_unfold(
            (input, context, 0u64),
            move |(input, context, offset)| async move {
                let read_input = input.clone();
                let chunk = owned(&context, "read local acquisition input", move || {
                    require_budget(deadline)?;
                    check_size(&read_input.file, size)?;
                    // One byte beyond selection can detect growth without an unbounded read.
                    let count = size.saturating_sub(offset).saturating_add(1).min(65536) as usize;
                    let mut bytes = vec![0; count];
                    let count = read_at(&read_input.file, &mut bytes, offset, deadline)?;
                    bytes.truncate(count);
                    Ok(bytes)
                })
                .await?;
                if chunk.is_empty() {
                    return Ok::<_, PumasError>(None);
                }
                let next = offset
                    .checked_add(chunk.len() as u64)
                    .ok_or_else(|| invalid("Local read offset overflow"))?;
                Ok(Some((bytes::Bytes::from(chunk), (input, context, next))))
            },
        )
        .boxed();
        Ok(HttpArtifactResponse {
            body,
            resumed: false, // Local retries verify/restart the whole file; no HTTP validator is invented.
            total_size: Some(size),
            resource: format!("local-sha256:{digest}"),
            strong_etag: None,
            deadline: None, // The shared owner enforces the biased source deadline.
        })
    }
}

fn require_budget(deadline: tokio::time::Instant) -> Result<()> {
    if tokio::time::Instant::now() >= deadline {
        return Err(invalid("Local acquisition elapsed budget exhausted"));
    }
    Ok(())
}

fn check_size(file: &File, size: u64) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != size {
        return Err(invalid(
            "Local input size or file kind differs from selection",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_at(
    file: &File,
    bytes: &mut [u8],
    offset: u64,
    deadline: tokio::time::Instant,
) -> Result<usize> {
    use std::os::unix::fs::FileExt;
    loop {
        require_budget(deadline)?;
        match file.read_at(bytes, offset) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => return result.map_err(PumasError::from),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn read_at(_: &File, _: &mut [u8], _: u64, _: tokio::time::Instant) -> Result<usize> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Local acquisition supports Linux only",
    )
    .into())
}

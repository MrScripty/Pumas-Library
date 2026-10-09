//! Bounded final-file accounting for this cache family only. No download custody.
use super::{invalid, valid_observation, Observation, CACHE_LIMIT};
use crate::{
    metadata::{AtomicJsonTarget, AtomicPublication},
    PumasError, Result,
};
use cap_std::fs::{Dir, OpenOptions};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub(super) const RECORD_LIMIT: usize = 256;
pub(super) const BYTE_LIMIT: u64 = 64 * 1024 * 1024;
const SCAN_ENTRY_LIMIT: usize = 4096;
const SCAN_BYTE_LIMIT: u64 = BYTE_LIMIT;

struct Entry {
    name: PathBuf,
    bytes: u64,
    evictable: bool,
    known: bool,
    observed: Option<DateTime<Utc>>,
}

fn owned_name(name: &str) -> bool {
    name.strip_prefix("hf_")
        .and_then(|name| name.strip_suffix("_metadata_v1.json"))
        .is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn known_observation(name: &str, observation: &Observation) -> bool {
    let expected = format!(
        "hf_{}_metadata_v1.json",
        hex::encode(Sha256::digest(observation.url.as_bytes()))
    );
    if name != expected {
        return false;
    }
    let Ok(url) = url::Url::parse(&observation.url) else {
        return false;
    };
    let Some((_, repo)) = observation.url.rsplit_once("/models/") else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && super::validate_hf_repo_id(repo).is_ok()
        && valid_observation(observation, &observation.url, repo)
        && (observation.body.is_some() || observation.blocked_until.is_some())
        && observation.validated_at.is_none_or(|at| at <= Utc::now())
}

fn read_observation(root: &Dir, name: &Path, expected_bytes: u64) -> Result<Option<Observation>> {
    if expected_bytes > CACHE_LIMIT {
        return Ok(None);
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        ._cap_fs_ext_follow(cap_primitives::fs::FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = root.open_with(name, &options)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("HF metadata cache entry changed kind"));
    }
    let mut bytes = Vec::new();
    file.take(expected_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected_bytes {
        return Err(invalid("HF metadata cache entry changed size"));
    }
    Ok(serde_json::from_slice::<Observation>(&bytes).ok())
}

fn scan(root: &Dir, classify: bool) -> Result<Vec<Entry>> {
    let now = Utc::now();
    let mut entries = Vec::new();
    let mut read_bytes = 0_u64;
    for (index, item) in root.entries()?.enumerate() {
        if index == SCAN_ENTRY_LIMIT {
            return Err(invalid("HF metadata cache inventory exceeds scan capacity"));
        }
        let item = item?;
        let name = item.file_name();
        let Some(text) = name.to_str().filter(|name| owned_name(name)) else {
            continue;
        };
        let metadata = root.symlink_metadata(&name)?;
        if !metadata.is_file() {
            return Err(invalid(
                "HF metadata cache contains a nonregular detail entry",
            ));
        }
        let mut entry = Entry {
            name: PathBuf::from(&name),
            bytes: metadata.len(),
            evictable: false,
            known: false,
            observed: None,
        };
        if classify && entry.bytes <= CACHE_LIMIT {
            if read_bytes
                .checked_add(entry.bytes)
                .is_none_or(|bytes| bytes > SCAN_BYTE_LIMIT)
            {
                return Err(invalid(
                    "HF metadata cache classification exceeds scan byte capacity",
                ));
            }
            read_bytes += entry.bytes;
            if let Some(observation) = read_observation(root, &entry.name, entry.bytes)? {
                if known_observation(text, &observation) {
                    entry.known = true;
                    entry.observed = observation.validated_at;
                    entry.evictable = observation.blocked_until.is_none_or(|until| until <= now);
                }
            }
        }
        entries.push(entry);
    }
    Ok(entries)
}

pub(super) fn usage(path: &Path) -> Result<(usize, u64)> {
    let root = crate::platform::capability_fs::open_directory(path)?;
    let entries = scan(&root, false)?;
    Ok((
        entries.len(),
        entries.iter().try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.bytes)
                .ok_or_else(|| invalid("HF metadata cache byte accounting overflow"))
        })?,
    ))
}

// Read-only discovery reuses this family's filename, handle and identity admission.
// Refuse an incomplete inventory rather than silently returning a partial catalog.
pub(super) fn visit_observations(
    path: &Path,
    mut visit: impl FnMut(Observation) -> Result<()>,
) -> Result<()> {
    let root = crate::platform::capability_fs::open_directory(path)?;
    let entries = scan(&root, false)?;
    if entries.len() > RECORD_LIMIT
        || entries
            .iter()
            .try_fold(0_u64, |bytes, entry| bytes.checked_add(entry.bytes))
            .is_none_or(|bytes| bytes > BYTE_LIMIT)
    {
        return Err(invalid("HF detail discovery exceeds cache capacity"));
    }
    for entry in entries {
        if let Some(observation) = read_observation(&root, &entry.name, entry.bytes)? {
            if entry
                .name
                .to_str()
                .is_some_and(|name| known_observation(name, &observation))
            {
                visit(observation)?;
            }
        }
    }
    Ok(())
}

pub(super) fn publish(path: &Path, observation: &Observation) -> Result<()> {
    publish_with_limits(path, observation, RECORD_LIMIT, BYTE_LIMIT)
}

pub(super) fn discard(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("HF metadata cache needs a parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| owned_name(name))
        .ok_or_else(|| invalid("Invalid HF metadata cache filename"))?;
    let root = crate::platform::capability_fs::open_directory(parent)?;
    let metadata = match root.symlink_metadata(name) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        other => other?,
    };
    if !metadata.is_file() {
        return Err(invalid("HF metadata cache target is nonregular"));
    }
    // Invalidation needs only its exact admitted target, not a directory-wide
    // capacity scan: a full/foreign directory cannot suppress no-store/denial policy.
    if !read_observation(&root, Path::new(name), metadata.len())?
        .is_some_and(|observation| known_observation(name, &observation))
    {
        return Err(invalid(
            "HF metadata cache target is a protected observation",
        ));
    }
    root.remove_file(name)?;
    Ok(())
}

pub(super) fn publish_with_limits(
    path: &Path,
    observation: &Observation,
    max_records: usize,
    max_bytes: u64,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("HF metadata cache needs a parent"))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| owned_name(name))
        .ok_or_else(|| invalid("Invalid HF metadata cache filename"))?;
    if !known_observation(name, observation) {
        return Err(invalid(
            "HF metadata cache publication is not an admitted observation",
        ));
    }
    let bytes = serde_json::to_vec_pretty(observation)?.len() as u64;
    if bytes > CACHE_LIMIT || bytes > max_bytes || max_records == 0 {
        return Err(invalid("HF metadata cache record exceeds storage budget"));
    }
    let root = crate::platform::capability_fs::open_directory(parent)?;
    let mut entries = scan(&root, true)?;
    let old = entries.iter().find(|entry| entry.name == Path::new(name));
    if old.is_some_and(|entry| !entry.known || !entry.evictable) {
        return Err(invalid(
            "HF metadata cache target is a protected observation",
        ));
    }
    let mut count = entries.len() + usize::from(old.is_none());
    let mut total = entries
        .iter()
        .try_fold(0_u64, |total, entry| {
            total
                .checked_add(entry.bytes)
                .ok_or_else(|| invalid("HF metadata cache byte accounting overflow"))
        })?
        .checked_sub(old.map_or(0, |entry| entry.bytes))
        .and_then(|total| total.checked_add(bytes))
        .ok_or_else(|| invalid("HF metadata cache byte accounting overflow"))?;
    entries.sort_by(|left, right| {
        left.observed
            .cmp(&right.observed)
            .then(left.name.cmp(&right.name))
    });
    let mut victims = Vec::new();
    for entry in &entries {
        if count <= max_records && total <= max_bytes {
            break;
        }
        if entry.evictable && entry.name != Path::new(name) {
            count -= 1;
            total -= entry.bytes;
            victims.push(&entry.name);
        }
    }
    if count > max_records || total > max_bytes {
        return Err(invalid(
            "HF metadata cache budget is occupied by protected observations",
        ));
    }
    // The held directory grants only this cache's validated leaf names. No
    // acquisition record, model output, foreign cache family or ambient leaf is removed.
    for victim in victims {
        root.remove_file(victim)?;
    }
    // Reuse the existing held-directory atomic publisher. This directly held
    // cache directory is the authority; its display path grants no extra access.
    let target = AtomicJsonTarget::from_capability(
        root,
        std::ffi::OsStr::new(name),
        path.to_path_buf(),
        || Ok(true),
    )?;
    match target
        .publish_json(observation)
        .map_err(|failure| failure.into_error())?
    {
        AtomicPublication::Durable => Ok(()),
        AtomicPublication::PublishedDurabilityUnknown { error }
        | AtomicPublication::VisibilityUnknown { error, .. } => Err(PumasError::Other(format!(
            "HF metadata cache publication uncertain: {error}"
        ))),
    }
}

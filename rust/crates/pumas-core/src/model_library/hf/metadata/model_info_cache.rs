//! Persistent, advisory model-detail observations. Never revision admission.
//! Reuses the HF file-cache directory and existing atomic publication primitive.
use super::{validate_hf_repo_id, HfModelInfoResponse, HuggingFaceClient, REPO_CACHE_TTL_SECS};
#[cfg(test)]
use crate::metadata::atomic_write_json;
use crate::{models::HuggingFaceModel, PumasError, Result};
use chrono::{DateTime, Utc};
use reqwest::{header, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};

const BODY_LIMIT: usize = 4 * 1024 * 1024;
const CACHE_LIMIT: u64 = BODY_LIMIT as u64 + 64 * 1024;
const DEFAULT_COOLDOWN: u64 = 300;
// Bound untrusted reset times to a representable, useful admission deadline.
const MAX_COOLDOWN: u64 = 24 * 60 * 60;
const MAX_REFRESH_PARTICIPANTS: usize = 32;

mod budget;
mod local_discovery;

// A narrow registry for anonymous observation refreshes. Weak entries cannot
// retain a client, payload, credential or physical store lifetime. The bound
// applies to distinct in-flight cache paths, not repositories stored on disk.
struct RefreshSlots {
    capacity: usize,
    locks: Mutex<HashMap<PathBuf, Weak<tokio::sync::Mutex<()>>>>,
}
impl RefreshSlots {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            locks: Mutex::new(HashMap::new()),
        }
    }
    fn acquire(&self, path: &std::path::Path) -> Result<Arc<tokio::sync::Mutex<()>>> {
        let mut locks = self
            .locks
            .lock()
            .map_err(|_| invalid("HF metadata refresh registry unavailable"))?;
        locks.retain(|_, value| value.strong_count() > 0);
        if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
            // This temporary upgrade includes the prospective participant.
            if Arc::strong_count(&lock) > MAX_REFRESH_PARTICIPANTS {
                return Err(PumasError::RateLimited {
                    service: "hf-metadata-local-admission".into(),
                    retry_after_secs: Some(1),
                });
            }
            return Ok(lock);
        }
        if locks.len() >= self.capacity {
            return Err(PumasError::RateLimited {
                service: "hf-metadata-local-admission".into(),
                retry_after_secs: Some(1),
            });
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(path.to_owned(), Arc::downgrade(&lock));
        Ok(lock)
    }
}
fn refresh_slots() -> &'static RefreshSlots {
    static SLOTS: std::sync::OnceLock<RefreshSlots> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| RefreshSlots::new(64))
}

fn budget_slots() -> &'static RefreshSlots {
    static SLOTS: std::sync::OnceLock<RefreshSlots> = std::sync::OnceLock::new();
    SLOTS.get_or_init(|| RefreshSlots::new(64))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    version: u32,
    url: String,
    // RawValue avoids pretty-print expansion of nested provider payloads.
    body: Option<Box<RawValue>>,
    etag: Option<String>,
    last_modified: Option<String>,
    validated_at: Option<DateTime<Utc>>,
    ttl_seconds: u64,
    blocked_until: Option<DateTime<Utc>>,
}

fn invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "hf.metadata".into(),
        message: message.into(),
    }
}
fn parse_body(body: &RawValue, repo: &str) -> Result<HfModelInfoResponse> {
    let parsed: HfModelInfoResponse = serde_json::from_str(body.get())
        .map_err(|_| invalid("Invalid HuggingFace model metadata"))?;
    if parsed.model.model_id != repo {
        return Err(invalid(
            "HuggingFace model metadata identified a different repository",
        ));
    }
    if parsed.sha.as_ref().is_some_and(|sha| {
        crate::model_library::artifact_identity::DownloadRevision::from_commit(sha).is_err()
    }) {
        return Err(invalid(
            "HuggingFace model metadata contains an invalid commit observation",
        ));
    }
    Ok(parsed)
}

fn valid_observation(cached: &Observation, url: &str, repo: &str) -> bool {
    cached.version == 1
        && cached.url == url
        && cached.ttl_seconds <= REPO_CACHE_TTL_SECS
        && cached
            .body
            .as_ref()
            .is_none_or(|body| body.get().len() <= BODY_LIMIT && parse_body(body, repo).is_ok())
        && cached
            .etag
            .as_ref()
            .is_none_or(|value| value.len() <= 1024 && valid_etag(value))
        && cached
            .last_modified
            .as_ref()
            .is_none_or(|value| value.len() <= 1024 && DateTime::parse_from_rfc2822(value).is_ok())
}

fn cache_path(client: &HuggingFaceClient, url: &str) -> PathBuf {
    // Exact endpoint and repository identity; legacy slash-to-underscore names
    // must not alias different repositories or source overrides.
    client.get_cache_path(&hex::encode(Sha256::digest(url.as_bytes())), "metadata_v1")
}
fn bounded_header(headers: &header::HeaderMap, key: header::HeaderName) -> Option<String> {
    let mut values = headers.get_all(key).iter();
    let first = values.next()?;
    if values.next().is_some() {
        return None;
    }
    first
        .to_str()
        .ok()
        .filter(|value| value.len() <= 1024)
        .map(str::to_owned)
}
fn valid_etag(value: &str) -> bool {
    let value = value.strip_prefix("W/").unwrap_or(value);
    value.len() >= 2
        && value.starts_with('"')
        && value.ends_with('"')
        && value[1..value.len() - 1]
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && byte != b'"')
}

fn retry_seconds(headers: &header::HeaderMap, now: DateTime<Utc>) -> Option<u64> {
    if let Some(value) = bounded_header(headers, header::RETRY_AFTER) {
        if let Some(seconds) = unsigned_seconds(&value) {
            return Some(seconds);
        }
        if let Ok(date) = DateTime::parse_from_rfc2822(&value) {
            return Some(remaining_seconds(date.with_timezone(&Utc), now));
        }
    }
    // HF's documented API bucket uses e.g. "api|pages|resolvers";r=0;t=56.
    let value = headers.get("RateLimit")?.to_str().ok()?;
    if value.len() > 1024 {
        return None;
    }
    value
        .split(',')
        .filter_map(|entry| {
            let mut parts = entry.trim().split(';');
            let bucket = parts.next()?.trim();
            if bucket.len() < 2
                || !bucket.starts_with('"')
                || !bucket.ends_with('"')
                || !bucket[1..bucket.len() - 1]
                    .split('|')
                    .any(|part| part == "api")
            {
                return None;
            }
            let mut remaining = None;
            let mut reset = None;
            for part in parts {
                let (key, value) = part.trim().split_once('=')?;
                match key {
                    "r" if remaining.is_none() => remaining = Some(unsigned_seconds(value)?),
                    "t" if reset.is_none() => reset = Some(unsigned_seconds(value)?),
                    _ => return None,
                }
            }
            (remaining == Some(0)).then_some(reset).flatten()
        })
        .max()
}
fn remaining_seconds(until: DateTime<Utc>, now: DateTime<Utc>) -> u64 {
    let delta = until.signed_duration_since(now);
    let seconds = delta.num_seconds().max(0) as u64;
    seconds + u64::from(delta > chrono::TimeDelta::seconds(seconds as i64))
}

fn unsigned_seconds(value: &str) -> Option<u64> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}
fn cache_policy(headers: &header::HeaderMap) -> (bool, u64) {
    let mut ttl = REPO_CACHE_TTL_SECS;
    let mut total = 0usize;
    for value in headers.get_all(header::CACHE_CONTROL) {
        let Ok(value) = value.to_str() else {
            return (false, 0);
        };
        total = total.saturating_add(value.len());
        if total > 4096 {
            return (false, 0);
        }
        for directive in value.split(',').map(str::trim) {
            let (key, value) = directive.split_once('=').unwrap_or((directive, ""));
            match key.to_ascii_lowercase().as_str() {
                "private" | "no-store" => return (false, 0),
                "no-cache" => ttl = 0,
                "max-age" => ttl = ttl.min(unsigned_seconds(value.trim_matches('"')).unwrap_or(0)),
                _ => {}
            }
        }
    }
    for value in headers.get_all(header::VARY) {
        let Ok(value) = value.to_str() else {
            return (false, 0);
        };
        total = total.saturating_add(value.len());
        if total > 4096
            || value.split(',').any(|part| {
                ["*", "authorization"].contains(&part.trim().to_ascii_lowercase().as_str())
            })
        {
            return (false, 0);
        }
    }
    let mut ages = headers.get_all(header::AGE).iter();
    let age = match ages.next() {
        None => 0,
        Some(value) if ages.next().is_none() => value
            .to_str()
            .ok()
            .and_then(unsigned_seconds)
            .unwrap_or(ttl),
        Some(_) => ttl,
    };
    (true, ttl.saturating_sub(age))
}

impl HuggingFaceClient {
    /// Maximum complete anonymous model-detail observation files per cache directory.
    pub const MODEL_DETAIL_CACHE_RECORD_LIMIT: usize = budget::RECORD_LIMIT;
    /// Maximum logical bytes in complete anonymous model-detail observation files.
    /// Atomic staging overhead and other HF cache families are accounted separately.
    pub const MODEL_DETAIL_CACHE_BYTE_LIMIT: u64 = budget::BYTE_LIMIT;

    /// Read the anonymous detail cache's `(record_count, logical_bytes)` without HTTP.
    /// Counts exact detail filenames only, including corrupt/future regular files.
    /// Nonregular entries and incomplete bounded scans return errors without cleanup.
    pub async fn model_detail_cache_usage(&self) -> Result<(usize, u64)> {
        let slot = budget_slots().acquire(&self.cache_dir)?;
        let guard = slot.lock_owned().await;
        let root = self.cache_dir.clone();
        self.store_lifetime
            .spawn_blocking(move || {
                let _admission = guard;
                budget::usage(&root)
            })
            .await
            .map_err(|_| invalid("HF metadata cache inventory effect failed"))?
    }

    pub(super) async fn get_model_info_observation(
        &self,
        repo: &str,
        force_refresh: bool,
    ) -> Result<HuggingFaceModel> {
        validate_hf_repo_id(repo)?;
        if self.auth_header_value().await.is_some() {
            return self.get_model_info_live(repo).await;
        }
        let url = format!("{}/models/{repo}", self.api_base_url());
        let slot = refresh_slots().acquire(&cache_path(self, &url))?;
        let guard = slot.lock_owned().await;
        // Recheck authentication and disk freshness after admission. Dropping a
        // waiting/HTTP caller releases its slot; blocking mutations retain the
        // owned guard through settlement, even after caller cancellation.
        self.get_model_info_observation_unlocked(repo, force_refresh, guard)
            .await
    }

    async fn get_model_info_observation_unlocked(
        &self,
        repo: &str,
        force_refresh: bool,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<HuggingFaceModel> {
        // Never read/write anonymous observations using an authenticated request.
        // Capture this decision once; the anonymous request below has no bearer.
        if self.auth_header_value().await.is_some() {
            drop(guard);
            return self.get_model_info_live(repo).await;
        }
        let url = format!("{}/models/{repo}", self.api_base_url());
        let path = cache_path(self, &url);
        let read_path = path.clone();
        let cached = self
            .store_lifetime
            .spawn_blocking(move || {
                let mut options = std::fs::OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
                }
                let file = options.open(read_path).ok()?;
                if !file.metadata().ok()?.is_file() {
                    return None;
                }
                let mut bytes = Vec::new();
                file.take(CACHE_LIMIT + 1).read_to_end(&mut bytes).ok()?;
                if bytes.len() as u64 > CACHE_LIMIT {
                    return None;
                }
                serde_json::from_slice::<Observation>(&bytes).ok()
            })
            .await
            .ok()
            .flatten()
            .filter(|cached| valid_observation(cached, &url, repo));
        let now = Utc::now();
        if let Some(cached) = &cached {
            if !force_refresh
                && cached.validated_at.is_some_and(|at| {
                    at <= now
                        && now.signed_duration_since(at).num_seconds() < cached.ttl_seconds as i64
                })
            {
                if let Some(body) = &cached.body {
                    return Ok(Self::convert_search_result(parse_body(body, repo)?.model));
                }
            }
            if let Some(until) = cached.blocked_until.filter(|until| *until > now) {
                return Err(PumasError::RateLimited {
                    service: "huggingface".into(),
                    retry_after_secs: Some(remaining_seconds(until, now)),
                });
            }
        }
        let mut request = self.client.get(&url);
        let mut conditional = false;
        if let Some(cached) = &cached {
            if cached.body.is_some() {
                if let Some(etag) = &cached.etag {
                    request = request.header(header::IF_NONE_MATCH, etag);
                    conditional = true;
                } else if let Some(modified) = &cached.last_modified {
                    request = request.header(header::IF_MODIFIED_SINCE, modified);
                    conditional = true;
                }
            }
        }
        let mut response = request.send().await.map_err(|error| PumasError::Network {
            message: "HuggingFace metadata request failed".into(),
            cause: Some(error.to_string()),
        })?;
        let now = Utc::now();
        let mut observation = cached.unwrap_or(Observation {
            version: 1,
            url: url.clone(),
            body: None,
            etag: None,
            last_modified: None,
            validated_at: None,
            ttl_seconds: 0,
            blocked_until: None,
        });
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry =
                retry_seconds(response.headers(), now).map(|seconds| seconds.min(MAX_COOLDOWN));
            let seconds = retry.unwrap_or(DEFAULT_COOLDOWN);
            observation.blocked_until = i64::try_from(seconds)
                .ok()
                .and_then(chrono::TimeDelta::try_seconds)
                .and_then(|duration| now.checked_add_signed(duration));
            if response.url().as_str() == url {
                self.save_metadata_observation(path, observation, guard)
                    .await;
            }
            return Err(PumasError::RateLimited {
                service: "huggingface".into(),
                retry_after_secs: retry,
            });
        }
        if response.status() != StatusCode::OK && response.status() != StatusCode::NOT_MODIFIED {
            if matches!(response.status().as_u16(), 401 | 403 | 404) {
                self.discard_metadata_observation(path, guard).await;
            }
            return Err(PumasError::Network {
                message: format!("HuggingFace metadata API returned {}", response.status()),
                cause: None,
            });
        }
        let (store, mut ttl) = cache_policy(response.headers());
        let store = store && response.url().as_str() == url;
        let etag =
            bounded_header(response.headers(), header::ETAG).filter(|value| valid_etag(value));
        let modified = bounded_header(response.headers(), header::LAST_MODIFIED)
            .filter(|value| DateTime::parse_from_rfc2822(value).is_ok());
        if response.status() == StatusCode::NOT_MODIFIED {
            if response.url().as_str() != url {
                return Err(invalid("HuggingFace 304 came from a different source URL"));
            }
            if !response.headers().contains_key(header::CACHE_CONTROL) {
                ttl = ttl.min(observation.ttl_seconds);
            }
            if !conditional || observation.body.is_none() {
                return Err(invalid(
                    "HuggingFace returned 304 without a matching cached representation",
                ));
            }
            if observation.etag.is_none()
                && modified
                    .as_ref()
                    .zip(observation.last_modified.as_ref())
                    .is_some_and(|(new, old)| {
                        DateTime::parse_from_rfc2822(new).ok()
                            != DateTime::parse_from_rfc2822(old).ok()
                    })
            {
                return Err(invalid(
                    "HuggingFace 304 contradicted the cached Last-Modified validator",
                ));
            }
            if etag
                .as_ref()
                .zip(observation.etag.as_ref())
                .is_some_and(|(new, old)| {
                    new.trim_start_matches("W/") != old.trim_start_matches("W/")
                })
            {
                return Err(invalid("HuggingFace 304 contradicted the cached validator"));
            }
        } else {
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| invalid("Failed to read HuggingFace model metadata"))?
            {
                if bytes.len().saturating_add(chunk.len()) > BODY_LIMIT {
                    return Err(invalid("HuggingFace model metadata exceeds 4 MiB limit"));
                }
                bytes.extend_from_slice(&chunk);
            }
            observation.body = Some(
                RawValue::from_string(
                    String::from_utf8(bytes)
                        .map_err(|_| invalid("HuggingFace model metadata is not UTF-8"))?,
                )
                .map_err(|_| invalid("Invalid HuggingFace model metadata JSON"))?,
            );
            observation.etag = None;
            observation.last_modified = None;
        }
        let model = parse_body(
            observation
                .body
                .as_ref()
                .ok_or_else(|| invalid("Missing HuggingFace metadata body"))?,
            repo,
        )?
        .model;
        if etag.is_some() {
            observation.etag = etag;
        }
        if modified.is_some() {
            observation.last_modified = modified;
        }
        observation.validated_at = Some(now);
        observation.ttl_seconds = ttl;
        observation.blocked_until = None;
        if store {
            self.save_metadata_observation(path, observation, guard)
                .await;
        } else {
            self.discard_metadata_observation(path, guard).await;
        }
        Ok(Self::convert_search_result(model))
    }

    // This path cannot touch observations, even if credentials disappear
    // between the routing decision and the actual snapshot request.
    async fn get_model_info_live(&self, repo: &str) -> Result<HuggingFaceModel> {
        #[cfg(test)]
        {
            let url = format!("{}/models/{repo}", self.api_base_url());
            metadata_fixture_live_gate(&cache_path(self, &url).with_extension("live-fixture"))
                .await;
        }
        let (model, _) = self.get_model_snapshot(repo).await?;
        if model.repo_id != repo {
            return Err(invalid(
                "HuggingFace model metadata identified a different repository",
            ));
        }
        Ok(model)
    }

    #[cfg(test)]
    pub(crate) fn hold_metadata_fixture_live(&self, repo: &str) -> Arc<MetadataEffectGate> {
        let gate = self.hold_metadata_fixture_effect(repo);
        let url = format!("{}/models/{repo}", self.api_base_url());
        let path = cache_path(self, &url);
        let mut gates = metadata_effect_gates().lock().unwrap();
        let weak = gates.remove(&path).unwrap();
        gates.insert(path.with_extension("live-fixture"), weak);
        gate
    }

    #[cfg(test)]
    pub(crate) fn hold_metadata_fixture_effect(&self, repo: &str) -> Arc<MetadataEffectGate> {
        let url = format!("{}/models/{repo}", self.api_base_url());
        let gate = Arc::new(MetadataEffectGate {
            entered: tokio::sync::Notify::new(),
            released: Mutex::new(false),
            wake: std::sync::Condvar::new(),
            async_wake: tokio::sync::Notify::new(),
        });
        metadata_effect_gates()
            .lock()
            .unwrap()
            .insert(cache_path(self, &url), Arc::downgrade(&gate));
        gate
    }

    #[cfg(test)]
    pub(crate) fn metadata_fixture_participants(&self, repo: &str) -> usize {
        let url = format!("{}/models/{repo}", self.api_base_url());
        refresh_slots()
            .locks
            .lock()
            .unwrap()
            .get(&cache_path(self, &url))
            .map(Weak::strong_count)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) async fn set_metadata_fixture_auth(&self, token: Option<String>) {
        *self.auth_token.write().await = token;
    }

    async fn save_metadata_observation(
        &self,
        path: PathBuf,
        observation: Observation,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) {
        let slot = match budget_slots().acquire(&self.cache_dir) {
            Ok(slot) => slot,
            Err(_) => {
                tracing::warn!(
                    "HuggingFace metadata cache budget unavailable; live metadata remains usable"
                );
                return;
            }
        };
        let budget_guard = slot.lock_owned().await;
        let result = self
            .store_lifetime
            .spawn_blocking(move || {
                let _admission = guard;
                let _budget_admission = budget_guard;
                #[cfg(test)]
                metadata_fixture_effect_gate(&path);
                budget::publish(&path, &observation)
            })
            .await;
        if !matches!(result, Ok(Ok(()))) {
            tracing::warn!(
                "HuggingFace metadata cache write unavailable; live metadata remains usable"
            );
        }
    }
    async fn discard_metadata_observation(
        &self,
        path: PathBuf,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) {
        let slot = match budget_slots().acquire(&self.cache_dir) {
            Ok(slot) => slot,
            Err(_) => {
                tracing::warn!("HuggingFace metadata cache invalidation budget unavailable");
                return;
            }
        };
        let budget_guard = slot.lock_owned().await;
        let result = self
            .store_lifetime
            .spawn_blocking(move || {
                let _admission = guard;
                let _budget_admission = budget_guard;
                #[cfg(test)]
                metadata_fixture_effect_gate(&path);
                budget::discard(&path)
            })
            .await;
        if !matches!(result, Ok(Ok(()))) {
            tracing::warn!("HuggingFace metadata cache invalidation unavailable");
        }
    }
}

#[cfg(test)]
pub(crate) struct MetadataEffectGate {
    entered: tokio::sync::Notify,
    released: Mutex<bool>,
    wake: std::sync::Condvar,
    async_wake: tokio::sync::Notify,
}
#[cfg(test)]
impl MetadataEffectGate {
    pub(crate) async fn entered(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), self.entered.notified())
            .await
            .unwrap();
    }
    pub(crate) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
        self.async_wake.notify_one();
    }
}
#[cfg(test)]
fn metadata_effect_gates() -> &'static Mutex<HashMap<PathBuf, Weak<MetadataEffectGate>>> {
    static GATES: std::sync::OnceLock<Mutex<HashMap<PathBuf, Weak<MetadataEffectGate>>>> =
        std::sync::OnceLock::new();
    GATES.get_or_init(Default::default)
}
#[cfg(test)]
fn metadata_fixture_effect_gate(path: &std::path::Path) {
    let gate = metadata_effect_gates()
        .lock()
        .unwrap()
        .remove(path)
        .and_then(|gate| gate.upgrade());
    if let Some(gate) = gate {
        gate.entered.notify_one();
        let (released, _) = gate
            .wake
            .wait_timeout_while(
                gate.released.lock().unwrap(),
                std::time::Duration::from_secs(5),
                |released| !*released,
            )
            .unwrap();
        assert!(
            *released,
            "metadata effect gate exceeded owned fixture deadline"
        );
    }
}

#[cfg(test)]
async fn metadata_fixture_live_gate(path: &std::path::Path) {
    let gate = metadata_effect_gates()
        .lock()
        .unwrap()
        .remove(path)
        .and_then(|gate| gate.upgrade());
    if let Some(gate) = gate {
        gate.entered.notify_one();
        let wait = gate.async_wake.notified();
        let released = *gate.released.lock().unwrap();
        if !released {
            tokio::time::timeout(std::time::Duration::from_secs(5), wait)
                .await
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    mod budget_tests;
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct Fixture {
        base: String,
        requests: Arc<Mutex<Vec<String>>>,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    impl Fixture {
        async fn new(responses: Vec<(&str, &str, String)>) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let mut responses: VecDeque<_> = responses
                .into_iter()
                .map(|(status, headers, body)| (status.to_owned(), headers.to_owned(), body))
                .collect();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let seen = requests.clone();
            let task = tokio::spawn(async move {
                loop {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    tokio::time::timeout(Duration::from_secs(3), async {
                        while !request.ends_with(b"\r\n\r\n") {
                            assert!(request.len() < 8192);
                            request.push(socket.read_u8().await.unwrap());
                        }
                    })
                    .await
                    .unwrap();
                    seen.lock()
                        .unwrap()
                        .push(String::from_utf8(request).unwrap());
                    let (status, headers, body) =
                        responses.pop_front().expect("unexpected metadata request");
                    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len()).as_bytes()).await.unwrap();
                    let _ = socket.write_all(body.as_bytes()).await;
                }
            });
            Self {
                base,
                requests,
                task,
            }
        }
        fn count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
        fn request(&self, index: usize) -> String {
            self.requests.lock().unwrap()[index].to_ascii_lowercase()
        }
        async fn client(&self, root: &std::path::Path) -> HuggingFaceClient {
            let mut client = HuggingFaceClient::new(root).unwrap();
            *client.auth_token.write().await = None; // No ambient token in fixtures.
            client.set_test_download_base_url(self.base.clone());
            client.client = reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap();
            client
        }
        fn path(&self, client: &HuggingFaceClient, repo: &str) -> PathBuf {
            cache_path(client, &format!("{}/models/{repo}", self.base))
        }
        fn observation(&self, client: &HuggingFaceClient) -> Observation {
            serde_json::from_slice(&std::fs::read(self.path(client, "acme/model")).unwrap())
                .unwrap()
        }
        fn expire(&self, client: &HuggingFaceClient) {
            let mut cached = self.observation(client);
            cached.validated_at = Some(Utc::now() - chrono::TimeDelta::days(2));
            atomic_write_json(&self.path(client, "acme/model"), &cached, false).unwrap();
        }
    }
    fn body(repo: &str, commit: char, downloads: u64) -> String {
        format!(
            r#"{{"modelId":"{repo}","sha":"{}","downloads":{downloads},"pipeline_tag":"text-generation","tags":["safetensors"],"cardData":{{"license":"apache-2.0"}},"config":{{"model_type":"llama","architectures":["LlamaForCausalLM"]}}}}"#,
            commit.to_string().repeat(40)
        )
    }

    #[tokio::test]
    async fn persistent_fresh_detail_reuse_preserves_model_card_without_http() {
        let fixture = Fixture::new(vec![(
            "200 OK",
            "ETag: \"a\"\r\n",
            body("acme/model", 'a', 12),
        )])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        let first = client.get_model_info_cached("acme/model").await.unwrap();
        assert_eq!(first.downloads, Some(12));
        assert_eq!(first.license.as_deref(), Some("apache-2.0"));
        drop(client);
        let reopened = fixture.client(root.path()).await;
        let repeated = reopened.get_model_info_cached("acme/model").await.unwrap();
        assert_eq!(first.model_card, repeated.model_card);
        assert_eq!(first.kind, repeated.kind);
        assert_eq!(fixture.count(), 1);
    }

    #[tokio::test]
    async fn etag_304_revalidates_exact_body_and_200_replaces_it() {
        let fixture = Fixture::new(vec![
            ("200 OK", "ETag: \"a\"\r\n", body("acme/model", 'a', 12)),
            ("304 Not Modified", "ETag: W/\"a\"\r\n", String::new()),
            ("200 OK", "ETag: \"b\"\r\n", body("acme/model", 'b', 42)),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        client.get_model_info_cached("acme/model").await.unwrap();
        fixture.expire(&client);
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(12)
        );
        assert!(fixture.request(1).contains("if-none-match: \"a\"\r\n"));
        assert!(!fixture.request(1).contains("if-modified-since:"));
        assert!(
            fixture.observation(&client).validated_at.unwrap()
                > Utc::now() - chrono::TimeDelta::seconds(5)
        );
        fixture.expire(&client);
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(42)
        );
        assert!(fixture.request(2).contains("if-none-match: w/\"a\"\r\n"));
        assert_eq!(fixture.observation(&client).etag.as_deref(), Some("\"b\""));
    }

    #[tokio::test]
    async fn last_modified_fallback_and_no_cache_policy_survive_304() {
        let headers = "Last-Modified: Wed, 07 Oct 2026 00:00:00 GMT\r\nCache-Control: no-cache\r\n";
        let fixture = Fixture::new(vec![
            ("200 OK", headers, body("acme/model", 'a', 1)),
            ("304 Not Modified", "", String::new()),
            ("304 Not Modified", "", String::new()),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        for _ in 0..3 {
            assert_eq!(
                client
                    .get_model_info_cached("acme/model")
                    .await
                    .unwrap()
                    .downloads,
                Some(1)
            );
        }
        assert_eq!(fixture.count(), 3);
        assert!(fixture
            .request(1)
            .contains("if-modified-since: wed, 07 oct 2026 00:00:00 gmt\r\n"));
        assert_eq!(fixture.observation(&client).ttl_seconds, 0);
    }

    #[tokio::test]
    async fn source_urls_and_slash_aliases_do_not_share_observations() {
        let first = Fixture::new(vec![
            ("200 OK", "", body("a/b_c", 'a', 1)),
            ("200 OK", "", body("a_b/c", 'b', 2)),
        ])
        .await;
        let second = Fixture::new(vec![("200 OK", "", body("a/b_c", 'c', 3))]).await;
        let root = tempfile::tempdir().unwrap();
        let client = first.client(root.path()).await;
        assert_eq!(
            client
                .get_model_info_cached("a/b_c")
                .await
                .unwrap()
                .downloads,
            Some(1)
        );
        assert_eq!(
            client
                .get_model_info_cached("a_b/c")
                .await
                .unwrap()
                .downloads,
            Some(2)
        );
        let other = second.client(root.path()).await;
        assert_eq!(
            other
                .get_model_info_cached("a/b_c")
                .await
                .unwrap()
                .downloads,
            Some(3)
        );
        assert_ne!(first.path(&client, "a/b_c"), first.path(&client, "a_b/c"));
        assert_ne!(first.path(&client, "a/b_c"), second.path(&other, "a/b_c"));
    }

    #[tokio::test]
    async fn stale_failure_does_not_publish_cached_data_as_fresh() {
        for status in [
            "500 Internal Server Error",
            "403 Forbidden",
            "404 Not Found",
        ] {
            let fixture = Fixture::new(vec![
                ("200 OK", "ETag: \"a\"\r\n", body("acme/model", 'a', 1)),
                (status, "", "{}".into()),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            fixture.expire(&client);
            assert!(client.get_model_info_cached("acme/model").await.is_err());
            if status.starts_with("500") {
                assert!(
                    fixture.observation(&client).validated_at.unwrap()
                        < Utc::now() - chrono::TimeDelta::days(1)
                );
            } else {
                assert!(!fixture.path(&client, "acme/model").exists());
            }
        }
    }

    #[tokio::test]
    async fn rate_limit_cooldown_is_persistent_repository_scoped_and_expires() {
        let fixture = Fixture::new(vec![
            (
                "429 Too Many Requests",
                "RateLimit: \"api|pages|resolvers\";r=0;t=30\r\n",
                "{}".into(),
            ),
            ("200 OK", "", body("other/model", 'b', 2)),
            ("200 OK", "", body("acme/model", 'a', 3)),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        assert!(matches!(
            client.get_model_info_cached("acme/model").await,
            Err(PumasError::RateLimited {
                retry_after_secs: Some(30),
                ..
            })
        ));
        drop(client);
        let reopened = fixture.client(root.path()).await;
        assert!(matches!(
            reopened.get_model_info_cached("acme/model").await,
            Err(PumasError::RateLimited {
                retry_after_secs: Some(1..=30),
                ..
            })
        ));
        assert_eq!(fixture.count(), 1);
        assert_eq!(
            reopened
                .get_model_info_cached("other/model")
                .await
                .unwrap()
                .downloads,
            Some(2)
        );
        let mut cached = fixture.observation(&reopened);
        cached.blocked_until = Some(Utc::now() - chrono::TimeDelta::seconds(1));
        atomic_write_json(&fixture.path(&reopened, "acme/model"), &cached, false).unwrap();
        assert_eq!(
            reopened
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(3)
        );
        assert!(fixture.observation(&reopened).blocked_until.is_none());
    }

    #[tokio::test]
    async fn authenticated_requests_bypass_anonymous_cache_and_never_persist_private_payloads() {
        let fixture = Fixture::new(vec![
            ("200 OK", "", body("acme/model", 'a', 1)),
            ("200 OK", "", body("acme/model", 'b', 999)),
            ("200 OK", "", body("private/model", 'c', 888)),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        client.get_model_info_cached("acme/model").await.unwrap();
        let original = std::fs::read(fixture.path(&client, "acme/model")).unwrap();
        *client.auth_token.write().await = Some("owned-fixture-token".into());
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(999)
        );
        assert_eq!(
            client
                .get_model_info_cached("private/model")
                .await
                .unwrap()
                .downloads,
            Some(888)
        );
        assert_eq!(
            std::fs::read(fixture.path(&client, "acme/model")).unwrap(),
            original
        );
        assert!(!fixture.path(&client, "private/model").exists());
        assert!(fixture
            .request(1)
            .contains("authorization: bearer owned-fixture-token\r\n"));
        assert!(!fixture.request(0).contains("authorization:"));
        *client.auth_token.write().await = None;
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(1)
        );
        assert_eq!(fixture.count(), 3);
    }

    #[tokio::test]
    async fn model_detail_cache_does_not_resolve_or_authorize_download_revisions() {
        let fixture = Fixture::new(vec![
            ("200 OK", "", body("acme/model", 'a', 1)),
            ("200 OK", "", body("acme/model", 'b', 2)),
            ("200 OK", "", body("acme/model", 'c', 3)),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        client.get_model_info_cached("acme/model").await.unwrap();
        assert_eq!(
            client
                .get_model_snapshot("acme/model")
                .await
                .unwrap()
                .0
                .downloads,
            Some(2)
        );
        let revision = client
            .resolve_download_revision("acme/model", None)
            .await
            .unwrap();
        assert_eq!(revision.as_str(), "c".repeat(40));
        assert!(fixture
            .request(2)
            .starts_with("get /api/models/acme/model/revision/main "));
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(1)
        );
        assert_eq!(fixture.count(), 3);
    }

    #[tokio::test]
    async fn corrupt_future_wrong_source_and_future_timestamp_entries_are_cache_misses() {
        for fault in ["json", "version", "url", "repo", "future"] {
            let fixture = Fixture::new(vec![
                ("200 OK", "", body("acme/model", 'a', 1)),
                ("200 OK", "", body("acme/model", 'b', 2)),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            let path = fixture.path(&client, "acme/model");
            let mut cached = fixture.observation(&client);
            match fault {
                "json" => {
                    std::fs::write(&path, b"not json").unwrap();
                }
                "version" => {
                    cached.version = 2;
                    atomic_write_json(&path, &cached, false).unwrap();
                }
                "url" => {
                    cached.url = "https://other.invalid/models/acme/model".into();
                    atomic_write_json(&path, &cached, false).unwrap();
                }
                "repo" => {
                    cached.body = Some(RawValue::from_string(body("other/model", 'a', 1)).unwrap());
                    atomic_write_json(&path, &cached, false).unwrap();
                }
                "future" => {
                    cached.validated_at = Some(Utc::now() + chrono::TimeDelta::days(1));
                    atomic_write_json(&path, &cached, false).unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                client
                    .get_model_info_cached("acme/model")
                    .await
                    .unwrap()
                    .downloads,
                Some(2),
                "{fault}"
            );
            assert_eq!(fixture.count(), 2);
        }
    }

    #[tokio::test]
    async fn no_store_and_unavailable_cache_storage_leave_live_metadata_usable() {
        for headers in [
            "Cache-Control: no-store\r\n",
            "Cache-Control: private\r\n",
            "Vary: Authorization\r\n",
        ] {
            let fixture = Fixture::new(vec![
                ("200 OK", headers, body("acme/model", 'a', 1)),
                ("200 OK", headers, body("acme/model", 'a', 1)),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            client.get_model_info_cached("acme/model").await.unwrap();
            assert_eq!(fixture.count(), 2);
            assert!(!fixture.path(&client, "acme/model").exists());
        }
        let fixture = Fixture::new(vec![("200 OK", "", body("acme/model", 'a', 1))]).await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        std::fs::create_dir(fixture.path(&client, "acme/model")).unwrap();
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(1)
        );
        assert!(fixture.path(&client, "acme/model").is_dir());
    }

    #[tokio::test]
    async fn orphan_contradictory_and_nonconditional_304_do_not_extend_observations() {
        let fixture = Fixture::new(vec![("304 Not Modified", "", String::new())]).await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        assert!(client.get_model_info_cached("acme/model").await.is_err());
        assert!(!fixture.path(&client, "acme/model").exists());
        for first_headers in ["ETag: \"a\"\r\n", ""] {
            let fixture = Fixture::new(vec![
                ("200 OK", first_headers, body("acme/model", 'a', 1)),
                ("304 Not Modified", "ETag: \"b\"\r\n", String::new()),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            fixture.expire(&client);
            let before = std::fs::read(fixture.path(&client, "acme/model")).unwrap();
            assert!(client.get_model_info_cached("acme/model").await.is_err());
            assert_eq!(
                before,
                std::fs::read(fixture.path(&client, "acme/model")).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn wrong_repository_invalid_revision_and_oversized_bodies_never_enter_cache() {
        for bytes in [
            body("other/model", 'a', 1),
            r#"{"modelId":"acme/model","sha":"bad"}"#.into(),
            "x".repeat(BODY_LIMIT + 1),
        ] {
            let fixture = Fixture::new(vec![("200 OK", "", bytes)]).await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            assert!(client.get_model_info_cached("acme/model").await.is_err());
            assert!(!fixture.path(&client, "acme/model").exists());
        }
    }

    #[tokio::test]
    async fn existing_refetch_getter_always_revalidates_even_fresh_observations() {
        let fixture = Fixture::new(vec![
            ("200 OK", "ETag: \"a\"\r\n", body("acme/model", 'a', 1)),
            ("304 Not Modified", "ETag: \"a\"\r\n", String::new()),
            ("200 OK", "ETag: \"b\"\r\n", body("acme/model", 'b', 2)),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        client.get_model_info_cached("acme/model").await.unwrap();
        assert_eq!(
            client.get_model_info("acme/model").await.unwrap().downloads,
            Some(1)
        );
        assert!(fixture.request(1).contains("if-none-match: \"a\"\r\n"));
        assert_eq!(
            client.get_model_info("acme/model").await.unwrap().downloads,
            Some(2)
        );
        assert_eq!(
            client
                .get_model_info_cached("acme/model")
                .await
                .unwrap()
                .downloads,
            Some(2)
        );
        assert_eq!(fixture.count(), 3);
    }

    #[tokio::test]
    async fn repeated_header_fields_cannot_hide_storage_or_revalidation_restrictions() {
        for headers in [
            "Cache-Control: max-age=90\r\nCache-Control: no-store\r\n",
            "Cache-Control: max-age=90\r\nCache-Control: private\r\n",
            "Vary: Accept\r\nVary: Authorization\r\n",
            "Vary: Accept\r\nVary: *\r\n",
        ] {
            let fixture = Fixture::new(vec![
                ("200 OK", headers, body("acme/model", 'a', 1)),
                ("200 OK", headers, body("acme/model", 'a', 1)),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            assert!(!fixture.path(&client, "acme/model").exists());
            client.get_model_info_cached("acme/model").await.unwrap();
            assert_eq!(fixture.count(), 2);
        }
        for headers in [
            "Cache-Control: max-age=90\r\nCache-Control: no-cache\r\n",
            "Cache-Control: max-age=90\r\nAge: 0\r\nAge: 100\r\n",
        ] {
            let fixture = Fixture::new(vec![
                ("200 OK", headers, body("acme/model", 'a', 1)),
                ("200 OK", headers, body("acme/model", 'a', 1)),
            ])
            .await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            client.get_model_info_cached("acme/model").await.unwrap();
            assert_eq!(fixture.observation(&client).ttl_seconds, 0);
            client.get_model_info_cached("acme/model").await.unwrap();
            assert_eq!(fixture.count(), 2);
        }
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::VARY,
            header::HeaderValue::from_bytes(b"\xff").unwrap(),
        );
        assert_eq!(cache_policy(&headers), (false, 0));
    }

    #[tokio::test]
    async fn last_modified_only_304_cannot_replace_validator_for_old_body() {
        let fixture = Fixture::new(vec![
            (
                "200 OK",
                "Last-Modified: Wed, 07 Oct 2026 00:00:00 GMT\r\n",
                body("acme/model", 'a', 1),
            ),
            (
                "304 Not Modified",
                "Last-Modified: Thu, 08 Oct 2026 00:00:00 GMT\r\n",
                String::new(),
            ),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        client.get_model_info_cached("acme/model").await.unwrap();
        let before = std::fs::read(fixture.path(&client, "acme/model")).unwrap();
        assert!(client.get_model_info("acme/model").await.is_err());
        assert!(fixture.request(1).contains("if-modified-since:"));
        assert_eq!(
            before,
            std::fs::read(fixture.path(&client, "acme/model")).unwrap()
        );
    }

    #[tokio::test]
    async fn huge_reset_values_preserve_bounded_cooldown_across_reopen() {
        for headers in [
            "Retry-After: 18446744073709551615\r\n",
            "RateLimit: \"api\";r=0;t=18446744073709551615\r\n",
        ] {
            let fixture =
                Fixture::new(vec![("429 Too Many Requests", headers, String::new())]).await;
            let root = tempfile::tempdir().unwrap();
            let client = fixture.client(root.path()).await;
            let started_at = Utc::now();
            assert!(matches!(
                client.get_model_info_cached("acme/model").await,
                Err(PumasError::RateLimited {
                    retry_after_secs: Some(MAX_COOLDOWN),
                    ..
                })
            ));
            drop(client);
            let reopened = fixture.client(root.path()).await;
            let result = reopened.get_model_info("acme/model").await;
            let elapsed = Utc::now()
                .signed_duration_since(started_at)
                .num_seconds()
                .max(0) as u64;
            let minimum_remaining = MAX_COOLDOWN.saturating_sub(elapsed.saturating_add(1));
            assert!(matches!(
                result,
                Err(PumasError::RateLimited {
                    retry_after_secs: Some(seconds),
                    ..
                }) if (minimum_remaining..=MAX_COOLDOWN).contains(&seconds)
            ));
            assert_eq!(fixture.count(), 1);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fifo_cache_entry_does_not_block_live_lookup() {
        let fixture = Fixture::new(vec![(
            "200 OK",
            "Cache-Control: no-store\r\n",
            body("acme/model", 'a', 1),
        )])
        .await;
        let root = tempfile::tempdir().unwrap();
        let client = fixture.client(root.path()).await;
        nix::unistd::mkfifo(
            &fixture.path(&client, "acme/model"),
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .unwrap();
        assert!(tokio::time::timeout(
            Duration::from_secs(3),
            client.get_model_info_cached("acme/model")
        )
        .await
        .unwrap()
        .is_ok());
        assert_eq!(fixture.count(), 1);
    }

    #[test]
    fn refresh_registry_is_bounded_and_reclaims_dropped_slots() {
        let slots = RefreshSlots::new(1);
        let first = slots.acquire(std::path::Path::new("first")).unwrap();
        assert!(Arc::ptr_eq(
            &first,
            &slots.acquire(std::path::Path::new("first")).unwrap()
        ));
        assert!(
            matches!(slots.acquire(std::path::Path::new("second")), Err(PumasError::RateLimited { service, retry_after_secs: Some(1) }) if service == "hf-metadata-local-admission")
        );
        drop(first);
        assert!(slots.acquire(std::path::Path::new("second")).is_ok());
    }

    #[test]
    fn refresh_registry_bounds_same_path_participants() {
        let slots = RefreshSlots::new(1);
        let held: Vec<_> = (0..MAX_REFRESH_PARTICIPANTS)
            .map(|_| slots.acquire(std::path::Path::new("first")).unwrap())
            .collect();
        assert!(matches!(
            slots.acquire(std::path::Path::new("first")),
            Err(PumasError::RateLimited {
                retry_after_secs: Some(1),
                ..
            })
        ));
        drop(held);
        assert!(slots.acquire(std::path::Path::new("first")).is_ok());
    }

    #[test]
    fn response_freshness_and_retry_headers_are_bounded_and_nonblocking() {
        let now = Utc::now();
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CACHE_CONTROL, "max-age=90".parse().unwrap());
        headers.insert(header::AGE, "20".parse().unwrap());
        assert_eq!(cache_policy(&headers), (true, 70));
        headers.insert(header::RETRY_AFTER, "10".parse().unwrap());
        assert_eq!(retry_seconds(&headers, now), Some(10));
        headers.insert(
            header::RETRY_AFTER,
            (now + chrono::TimeDelta::seconds(20))
                .format("%a, %d %b %Y %H:%M:%S GMT")
                .to_string()
                .parse()
                .unwrap(),
        );
        assert!(matches!(retry_seconds(&headers, now), Some(19..=20)));
        headers.remove(header::RETRY_AFTER);
        for value in [
            "\"",
            "garbage",
            "\"api\";r=0;t=18446744073709551616",
            "\"resolvers\";r=0;t=20",
            "\"api\";r=0;t=1;t=2",
        ] {
            headers.insert("RateLimit", value.parse().unwrap());
            assert_eq!(retry_seconds(&headers, now), None, "{value}");
        }
    }
}

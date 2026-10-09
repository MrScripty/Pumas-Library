//! Version state tracking.
//!
//! Manages the state of installed, active, and default versions.
//! Handles state persistence and validation.

use super::installer::{NativeVersionsLock, TorchVersionsLock};
use super::operation_receipt::OperationReceipt;
use crate::version_manager::ValidationResult;
use futures::FutureExt;
use pumas_library::config::AppId;
use pumas_library::metadata::{InstalledVersionMetadata, MetadataManager};
use pumas_library::{PumasError, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::fs;
use tracing::{debug, info, warn};

const LLAMA_CPP_BINARY_SEARCH_LIMIT: usize = 4096;

fn legacy_llama_cpp_sycl_replacements(
    installed: &HashMap<String, InstalledVersionMetadata>,
) -> Vec<(String, String)> {
    installed
        .iter()
        .filter_map(|(tag, metadata)| {
            let base_tag = tag.strip_suffix("+sycl")?;
            let download_url = metadata.download_url.as_deref()?.to_ascii_lowercase();
            let precision = if download_url.contains("sycl-fp16") {
                "sycl-fp16"
            } else if download_url.contains("sycl-fp32") {
                "sycl-fp32"
            } else {
                return None;
            };
            Some((tag.clone(), format!("{base_tag}+{precision}")))
        })
        .collect()
}

#[derive(Default)]
pub(crate) struct StateMutationTasks {
    state: StdMutex<StateMutationLifecycle>,
}

#[derive(Default)]
struct StateMutationLifecycle {
    closed: bool,
    tasks: super::InstallationTasks,
    receipts: Vec<OperationReceipt>,
    completion: Option<super::InstallationShutdown>,
}

impl StateMutationTasks {
    #[cfg(test)]
    pub(crate) fn has_active_tasks(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .tasks
            .tasks
            .iter()
            .any(|task| !task.is_finished())
    }
    fn ensure_open(&self) -> Result<()> {
        let state = self
            .state
            .lock()
            .map_err(|_| PumasError::Other("State mutation registry poisoned".into()))?;
        if state.closed {
            return Err(PumasError::Other(
                "Version state mutation admission closed".into(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn leased_transaction<T: Send + 'static, L: Clone + Send + 'static>(
        &self,
        lock: &L,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let receipt = OperationReceipt::default();
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| PumasError::Other("State mutation registry poisoned".into()))?;
            if state.closed {
                return Err(PumasError::Other(
                    "Version state mutation admission closed".into(),
                ));
            }
            state.tasks.harvest_finished();
            state.receipts.retain(OperationReceipt::retain);
            state.receipts.push(receipt.clone());
            let worker_receipt = receipt.clone();
            let lease = lock.clone();
            // Register before any suspension. The receiver belongs to this
            // caller; the lifecycle owns the worker's independent receipt.
            let task = tokio::task::spawn_blocking(move || {
                let result = work();
                drop(lease);
                worker_receipt.complete(&result);
                let _ = sender.send(result);
                Ok(())
            });
            state.tasks.tasks.push(task);
        }
        let result = receiver.await.map_err(|error| {
            PumasError::Other(format!("Version metadata task lost its result: {error}"))
        })?;
        // Only receiver consumption acknowledges the result; a successful send
        // can still leave an error buffered in an abandoned operation future.
        receipt.observe();
        result
    }

    async fn shutdown(&self) -> Result<()> {
        let completion = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| PumasError::Other("State mutation registry poisoned".into()))?;
            state.closed = true;
            if let Some(completion) = &state.completion {
                completion.clone()
            } else {
                let registered = std::mem::take(&mut state.tasks);
                let receipts = std::mem::take(&mut state.receipts);
                let supervisor = tokio::spawn(async move {
                    let mut failures = registered.failures;
                    for task in registered.tasks {
                        match task.await {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => failures.push(error),
                            Err(error) => failures.push(error.to_string()),
                        }
                    }
                    failures.extend(
                        receipts
                            .iter()
                            .filter_map(OperationReceipt::unobserved_error),
                    );
                    if failures.is_empty() {
                        Ok(())
                    } else {
                        Err(Arc::new(failures.join("; ")))
                    }
                });
                let completion = async move {
                    supervisor
                        .await
                        .unwrap_or_else(|error| Err(Arc::new(error.to_string())))
                }
                .boxed()
                .shared();
                state.completion = Some(completion.clone());
                completion
            }
        };
        completion.await.map_err(|error| {
            PumasError::Other(format!("Version state mutation drain failed: {error}"))
        })
    }
}

/// Tracks the state of all versions.
///
/// Native llama.cpp mutations and legacy normalization share the installation
/// lease. A competing mutation returns an error; started blocking workers keep
/// their lease if the caller stops waiting. Cached read accessors are snapshots.
/// Direct owners must retain this state and await `shutdown_mutations` before
/// dropping it or stopping its runtime. Construction must be awaited to completion.
pub struct VersionState {
    mutation_tasks: Arc<StateMutationTasks>,
    /// Root directory for launcher.
    launcher_root: PathBuf,
    /// Application ID.
    app_id: AppId,
    /// Metadata manager for persistence.
    metadata_manager: Arc<MetadataManager>,
    /// Set of installed version tags (cached).
    installed_tags: HashSet<String>,
    /// Metadata from the same read as the cached tags and selections.
    installed_metadata: HashMap<String, InstalledVersionMetadata>,
    /// Currently active version (session-specific).
    active_version: Option<String>,
    /// Default version from metadata.
    default_version: Option<String>,
}

impl VersionState {
    pub(crate) fn mutation_tasks(&self) -> Arc<StateMutationTasks> {
        self.mutation_tasks.clone()
    }

    /// Close mutation admission and observe registered mutation workers, including workers
    /// whose callers stopped waiting. Repeated calls retain the same outcome.
    pub async fn shutdown_mutations(&mut self) -> Result<()> {
        self.mutation_tasks.shutdown().await
    }

    fn torch_versions_lock(&self) -> Result<Option<TorchVersionsLock>> {
        if self.app_id != AppId::Torch {
            return Ok(None);
        }
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        std::fs::create_dir_all(&versions_dir).map_err(PumasError::from)?;
        Ok(Some(
            TorchVersionsLock::try_acquire(&versions_dir).map_err(PumasError::from)?,
        ))
    }
    async fn native_versions_lock(&self) -> Result<Option<NativeVersionsLock>> {
        if self.app_id != AppId::LlamaCpp {
            return Ok(None);
        }
        NativeVersionsLock::acquire(self.launcher_root.join(self.app_id.versions_dir_name()))
            .await
            .map(Some)
    }

    async fn load_versions_metadata(&self) -> Result<pumas_library::metadata::VersionsMetadata> {
        let metadata_manager = self.metadata_manager.clone();
        let app_id = self.app_id;
        tokio::task::spawn_blocking(move || metadata_manager.load_versions(Some(app_id)))
            .await
            .map_err(|err| {
                PumasError::Other(format!(
                    "Failed to join version-state metadata load task: {}",
                    err
                ))
            })?
    }

    async fn set_default_version_metadata(&self, tag: Option<String>) -> Result<()> {
        let metadata_manager = self.metadata_manager.clone();
        let app_id = self.app_id;
        self.mutation_tasks
            .leased_transaction(&(), move || {
                metadata_manager.set_default_version(tag.as_deref(), Some(app_id))
            })
            .await
    }

    /// Create a new version state tracker.
    pub async fn new(
        launcher_root: &Path,
        app_id: AppId,
        metadata_manager: Arc<MetadataManager>,
    ) -> Result<Self> {
        let mut state = Self {
            mutation_tasks: Arc::new(StateMutationTasks::default()),
            launcher_root: launcher_root.to_path_buf(),
            app_id,
            metadata_manager,
            installed_tags: HashSet::new(),
            installed_metadata: HashMap::new(),
            active_version: None,
            default_version: None,
        };

        state.initialize().await?;
        Ok(state)
    }

    /// Initialize state from metadata and filesystem.
    async fn initialize(&mut self) -> Result<()> {
        let native_lock = self.native_versions_lock().await?;
        if self.app_id == AppId::Torch {
            let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
            let metadata = self.metadata_manager.clone();
            match tokio::task::spawn_blocking(move || {
                super::installer::retry_pending_torch_cleanup(&versions_dir, &metadata)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => warn!(%error, "Torch cleanup recovery failed"),
                Err(error) => warn!(%error, "Torch cleanup recovery task failed"),
            }
        }
        self.normalize_llama_cpp_legacy_sycl_variants(native_lock.as_ref())
            .await?;

        // A selection writes the marker before its metadata commit. Startup
        // must not accept that marker while another backend owns the write.
        let snapshot_lock = if self.app_id == AppId::Torch {
            let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
            std::fs::create_dir_all(&versions_dir).map_err(PumasError::from)?;
            match TorchVersionsLock::try_acquire(&versions_dir) {
                Ok(lock) => Some(lock),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
                Err(error) => return Err(PumasError::from(error)),
            }
        } else {
            None
        };

        // Load metadata
        let versions = self.load_versions_metadata().await?;

        let installed_tags = versions.installed.keys().cloned().collect();
        let active_version = self
            .determine_active_version(
                &versions,
                &installed_tags,
                self.app_id != AppId::Torch || snapshot_lock.is_some(),
            )
            .await?;
        self.installed_metadata = versions.installed;
        self.installed_tags = installed_tags;
        self.default_version = versions.default_version;
        self.active_version = active_version;

        debug!(
            "Initialized version state: {} installed, active={:?}, default={:?}",
            self.installed_tags.len(),
            self.active_version,
            self.default_version
        );

        Ok(())
    }

    /// Determine which version should be active.
    ///
    /// Priority:
    /// 1. Read from .active-version file (if still valid)
    /// 2. Default version from metadata
    /// 3. Last selected version from metadata
    /// 4. Newest installed version
    async fn determine_active_version(
        &self,
        versions: &pumas_library::metadata::VersionsMetadata,
        installed_tags: &HashSet<String>,
        read_active_marker: bool,
    ) -> Result<Option<String>> {
        // 1. Check .active-version file
        let active_file = super::active_version_path(&self.launcher_root, self.app_id);
        if read_active_marker
            && fs::try_exists(&active_file)
                .await
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to check active version file: {}", e),
                    path: Some(active_file.clone()),
                    source: Some(e),
                })?
        {
            if let Ok(tag) = fs::read_to_string(&active_file).await {
                let tag = tag.trim().to_string();
                if !tag.is_empty() && installed_tags.contains(&tag) {
                    debug!("Active version from file: {}", tag);
                    return Ok(Some(tag));
                }
            }
        }

        // During a busy Torch transaction, the marker may be transitional.
        // The committed last selection is a closer substitute for it than the
        // configured default, which can intentionally point elsewhere.
        if self.app_id == AppId::Torch && !read_active_marker {
            if let Some(last) = &versions.last_selected_version {
                if installed_tags.contains(last) {
                    return Ok(Some(last.clone()));
                }
            }
        }

        // 2. Default version
        if let Some(ref default) = versions.default_version {
            if installed_tags.contains(default) {
                debug!("Active version from default: {}", default);
                return Ok(Some(default.clone()));
            }
        }

        // 3. Last selected version
        if let Some(ref last) = versions.last_selected_version {
            if installed_tags.contains(last) {
                debug!("Active version from last selected: {}", last);
                return Ok(Some(last.clone()));
            }
        }

        // Torch requires an explicit trial/selection before it becomes active.
        if self.app_id == AppId::Torch {
            return Ok(None);
        }

        // 4. Newest installed version (lexicographically, which works for semver with v prefix)
        if !installed_tags.is_empty() {
            let mut sorted: Vec<_> = installed_tags.iter().cloned().collect();
            sorted.sort();
            sorted.reverse(); // Newest first
            let newest = sorted.into_iter().next();
            debug!("Active version from newest: {:?}", newest);
            return Ok(newest);
        }

        Ok(None)
    }

    /// Refresh state from disk.
    pub async fn refresh(&mut self) -> Result<()> {
        self.mutation_tasks.ensure_open()?;
        let native_lock = self.native_versions_lock().await?;
        let lock = self.torch_versions_lock()?;
        self.refresh_inner(lock.as_ref(), native_lock.as_ref())
            .await
    }

    pub(crate) async fn refresh_with_lock(&mut self, lock: &TorchVersionsLock) -> Result<()> {
        debug_assert_eq!(self.app_id, AppId::Torch);
        self.refresh_inner(Some(lock), None).await
    }

    pub(crate) async fn refresh_with_native_lock(
        &mut self,
        lock: &NativeVersionsLock,
    ) -> Result<()> {
        self.refresh_inner(None, Some(lock)).await
    }

    async fn refresh_inner(
        &mut self,
        _lock: Option<&TorchVersionsLock>,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<()> {
        self.normalize_llama_cpp_legacy_sycl_variants(native_lock)
            .await?;

        let versions = self.load_versions_metadata().await?;
        let installed_tags: HashSet<String> = versions.installed.keys().cloned().collect();

        // Torch selection is shared between backends; a cached but still
        // installed tag can nevertheless be stale after another selection.
        let active_version = if self.app_id == AppId::Torch {
            self.determine_active_version(&versions, &installed_tags, true)
                .await?
        } else if self
            .active_version
            .as_ref()
            .is_some_and(|active| installed_tags.contains(active))
        {
            self.active_version.clone()
        } else {
            self.determine_active_version(&versions, &installed_tags, true)
                .await?
        };

        self.installed_metadata = versions.installed;
        self.installed_tags = installed_tags;
        self.default_version = versions.default_version;
        self.active_version = active_version;

        Ok(())
    }

    async fn normalize_llama_cpp_legacy_sycl_variants(
        &self,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<()> {
        if self.app_id != AppId::LlamaCpp {
            return Ok(());
        }
        let lock = native_lock.ok_or_else(|| PumasError::InstallationFailed {
            message: "Native metadata mutation lease absent".into(),
        })?;
        let metadata_manager = self.metadata_manager.clone();
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        let active_file = super::active_version_path(&self.launcher_root, self.app_id);
        self.mutation_tasks
            .leased_transaction(lock, move || {
                let mut versions = metadata_manager.load_versions(Some(AppId::LlamaCpp))?;
                let replacements = legacy_llama_cpp_sycl_replacements(&versions.installed);
                if replacements.is_empty() {
                    return Ok(());
                }
                for (old_tag, new_tag) in &replacements {
                    let Some(mut metadata) = versions.installed.remove(old_tag) else {
                        continue;
                    };
                    metadata.path = new_tag.clone();
                    metadata.release_tag = new_tag.clone();
                    versions
                        .installed
                        .entry(new_tag.clone())
                        .or_insert(metadata);
                    if versions.last_selected_version.as_ref() == Some(old_tag) {
                        versions.last_selected_version = Some(new_tag.clone());
                    }
                    if versions.default_version.as_ref() == Some(old_tag) {
                        versions.default_version = Some(new_tag.clone());
                    }
                }
                for (old_tag, new_tag) in replacements {
                    let old_path = versions_dir.join(&old_tag);
                    let new_path = versions_dir.join(&new_tag);
                    if old_path
                        .try_exists()
                        .map_err(|error| PumasError::io_with_path(error, &old_path))?
                        && !new_path
                            .try_exists()
                            .map_err(|error| PumasError::io_with_path(error, &new_path))?
                    {
                        std::fs::rename(&old_path, &new_path)
                            .map_err(|error| PumasError::io_with_path(error, &old_path))?;
                    }
                    match std::fs::read_to_string(&active_file) {
                        Ok(active) if active.trim() == old_tag => {
                            std::fs::write(&active_file, new_tag)
                                .map_err(|error| PumasError::io_with_path(error, &active_file))?;
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(PumasError::io_with_path(error, &active_file)),
                    }
                }
                // Keep the old tags durable until every rename and active
                // pointer update succeeds, so a partial migration can retry.
                metadata_manager.save_versions(&versions, Some(AppId::LlamaCpp))?;
                Ok(())
            })
            .await
    }

    // ========================================
    // Getters
    // ========================================

    /// Get list of installed version tags.
    pub fn get_installed_tags(&self) -> Vec<String> {
        let mut tags: Vec<_> = self.installed_tags.iter().cloned().collect();
        tags.sort();
        tags.reverse(); // Newest first
        tags
    }

    /// Get the currently active version.
    pub fn get_active_version(&self) -> Option<String> {
        self.active_version.clone()
    }

    /// Get the default version.
    pub fn get_default_version(&self) -> Option<String> {
        self.default_version.clone()
    }

    pub(crate) fn get_installed_metadata(&self, tag: &str) -> Option<&InstalledVersionMetadata> {
        self.installed_metadata.get(tag)
    }

    /// Check if a version is installed.
    pub fn is_installed(&self, tag: &str) -> bool {
        self.installed_tags.contains(tag)
    }

    /// Get the path to a version's directory.
    pub fn get_version_path(&self, tag: &str) -> Option<PathBuf> {
        if self.is_installed(tag) {
            Some(
                self.launcher_root
                    .join(self.app_id.versions_dir_name())
                    .join(tag),
            )
        } else {
            None
        }
    }

    // ========================================
    // Setters
    // ========================================

    /// Set the active version.
    pub async fn set_active_version(&mut self, tag: &str) -> Result<bool> {
        self.mutation_tasks.ensure_open()?;
        let native_lock = self.native_versions_lock().await?;
        let lock = self.torch_versions_lock()?;
        if lock.is_some() || native_lock.is_some() {
            self.refresh_inner(lock.as_ref(), native_lock.as_ref())
                .await?;
        }
        self.set_active_version_inner(tag, lock.as_ref(), native_lock.as_ref())
            .await
    }

    pub(crate) async fn set_active_version_with_lock(
        &mut self,
        tag: &str,
        lock: &TorchVersionsLock,
    ) -> Result<bool> {
        self.set_active_version_inner(tag, Some(lock), None).await
    }

    async fn set_active_version_inner(
        &mut self,
        tag: &str,
        lock: Option<&TorchVersionsLock>,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<bool> {
        if !self.is_installed(tag) {
            return Err(PumasError::VersionNotFound {
                tag: tag.to_string(),
            });
        }
        let active_file = super::active_version_path(&self.launcher_root, self.app_id);
        if self.app_id == AppId::Torch {
            let metadata = self.metadata_manager.clone();
            let selected = tag.to_owned();
            self.mutation_tasks
                .leased_transaction(lock.expect("Torch selection lock required"), move || {
                    let previous = match std::fs::read(&active_file) {
                        Ok(bytes) => Some(bytes),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        Err(error) => return Err(PumasError::io_with_path(error, &active_file)),
                    };
                    std::fs::write(&active_file, &selected)
                        .map_err(|error| PumasError::io_with_path(error, &active_file))?;
                    if let Err(error) =
                        metadata.set_last_selected_version(Some(&selected), Some(AppId::Torch))
                    {
                        match previous {
                            Some(bytes) => {
                                let _ = std::fs::write(&active_file, bytes);
                            }
                            None => {
                                let _ = std::fs::remove_file(&active_file);
                            }
                        }
                        return Err(error);
                    }
                    Ok(())
                })
                .await?;
        } else if let Some(lock) = native_lock {
            let metadata = self.metadata_manager.clone();
            let selected = tag.to_owned();
            self.mutation_tasks
                .leased_transaction(lock, move || {
                    std::fs::write(&active_file, &selected)
                        .map_err(|error| PumasError::io_with_path(error, &active_file))?;
                    metadata.set_last_selected_version(Some(&selected), Some(AppId::LlamaCpp))
                })
                .await?;
        } else {
            let selected = tag.to_owned();
            let metadata = self.metadata_manager.clone();
            let app_id = self.app_id;
            self.mutation_tasks
                .leased_transaction(&(), move || {
                    std::fs::write(&active_file, &selected)
                        .map_err(|error| PumasError::io_with_path(error, &active_file))?;
                    metadata.set_last_selected_version(Some(&selected), Some(app_id))
                })
                .await?;
        }
        self.active_version = Some(tag.to_string());

        info!("Set active version: {}", tag);
        Ok(true)
    }

    /// Set the default version.
    pub async fn set_default_version(&mut self, tag: Option<&str>) -> Result<bool> {
        self.mutation_tasks.ensure_open()?;
        let native_lock = self.native_versions_lock().await?;
        let lock = self.torch_versions_lock()?;
        if lock.is_some() || native_lock.is_some() {
            self.refresh_inner(lock.as_ref(), native_lock.as_ref())
                .await?;
        }
        self.set_default_version_inner(tag, lock.as_ref(), native_lock.as_ref())
            .await
    }

    pub(crate) async fn set_default_version_with_lock(
        &mut self,
        tag: Option<&str>,
        lock: &TorchVersionsLock,
    ) -> Result<bool> {
        self.set_default_version_inner(tag, Some(lock), None).await
    }

    async fn set_default_version_inner(
        &mut self,
        tag: Option<&str>,
        lock: Option<&TorchVersionsLock>,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<bool> {
        if let Some(t) = tag {
            if !self.is_installed(t) {
                return Err(PumasError::VersionNotFound { tag: t.to_string() });
            }
        }

        if self.app_id == AppId::Torch {
            let metadata = self.metadata_manager.clone();
            let selected = tag.map(str::to_owned);
            self.mutation_tasks
                .leased_transaction(lock.expect("Torch selection lock required"), move || {
                    metadata.set_default_version(selected.as_deref(), Some(AppId::Torch))
                })
                .await?;
        } else if let Some(lock) = native_lock {
            let metadata = self.metadata_manager.clone();
            let selected = tag.map(str::to_owned);
            self.mutation_tasks
                .leased_transaction(lock, move || {
                    metadata.set_default_version(selected.as_deref(), Some(AppId::LlamaCpp))
                })
                .await?;
        } else {
            self.set_default_version_metadata(tag.map(String::from))
                .await?;
        }
        self.default_version = tag.map(String::from);

        info!("Set default version: {:?}", tag);
        Ok(true)
    }

    pub(crate) async fn reset_torch_active_selection_with_lock(
        &mut self,
        lock: &TorchVersionsLock,
    ) -> Result<()> {
        debug_assert_eq!(self.app_id, AppId::Torch);
        let marker = super::active_version_path(&self.launcher_root, self.app_id);
        let metadata = self.metadata_manager.clone();
        self.mutation_tasks
            .leased_transaction(lock, move || {
                let previous = match std::fs::read(&marker) {
                    Ok(bytes) => Some(bytes),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(PumasError::io_with_path(error, &marker)),
                };
                match std::fs::remove_file(&marker) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(PumasError::io_with_path(error, &marker)),
                }
                if let Err(error) = metadata.set_last_selected_version(None, Some(AppId::Torch)) {
                    if let Some(bytes) = previous {
                        let _ = std::fs::write(&marker, bytes);
                    }
                    return Err(error);
                }
                Ok(())
            })
            .await?;
        self.refresh_inner(Some(lock), None).await
    }

    /// Add a new installed version.
    pub fn add_installed_version(
        &mut self,
        tag: &str,
        metadata: InstalledVersionMetadata,
    ) -> Result<()> {
        self.mutation_tasks.ensure_open()?;
        let _lock = self.torch_versions_lock()?;
        let _native_lock = if self.app_id == AppId::LlamaCpp {
            Some(NativeVersionsLock::try_acquire(
                &self.launcher_root.join(self.app_id.versions_dir_name()),
            )?)
        } else {
            None
        };
        if matches!(self.app_id, AppId::Torch | AppId::LlamaCpp) {
            let versions = self.metadata_manager.load_versions(Some(self.app_id))?;
            self.installed_tags = versions.installed.keys().cloned().collect();
            self.installed_metadata = versions.installed;
            self.default_version = versions.default_version;
        }
        let cached_metadata = metadata.clone();
        self.metadata_manager
            .update_installed_version(tag, metadata, Some(self.app_id))?;
        self.installed_tags.insert(tag.to_string());
        self.installed_metadata
            .insert(tag.to_string(), cached_metadata);
        debug!("Added installed version: {}", tag);
        Ok(())
    }

    /// Remove an installed version.
    pub async fn remove_installed_version(&mut self, tag: &str) -> Result<()> {
        self.mutation_tasks.ensure_open()?;
        let native_lock = self.native_versions_lock().await?;
        let lock = self.torch_versions_lock()?;
        if lock.is_some() || native_lock.is_some() {
            self.refresh_inner(lock.as_ref(), native_lock.as_ref())
                .await?;
        }
        self.remove_installed_version_inner(tag, lock.as_ref(), native_lock.as_ref())
            .await
    }

    async fn remove_installed_version_inner(
        &mut self,
        tag: &str,
        _lock: Option<&TorchVersionsLock>,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<()> {
        if let Some(lock) = native_lock {
            let metadata = self.metadata_manager.clone();
            let removed = tag.to_owned();
            let marker = super::active_version_path(&self.launcher_root, self.app_id);
            let remove_marker = self.active_version.as_deref() == Some(tag);
            self.mutation_tasks
                .leased_transaction(lock, move || {
                    metadata.remove_installed_version(&removed, Some(AppId::LlamaCpp))?;
                    if remove_marker {
                        match std::fs::remove_file(&marker) {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(PumasError::io_with_path(error, &marker)),
                        }
                    }
                    Ok(())
                })
                .await?;
        } else {
            self.metadata_manager
                .remove_installed_version(tag, Some(self.app_id))?;
        }
        self.installed_tags.remove(tag);
        self.installed_metadata.remove(tag);

        // Clear active if it was this version
        if self.active_version.as_deref() == Some(tag) {
            self.active_version = None;
            // Clear .active-version file
            let active_file = super::active_version_path(&self.launcher_root, self.app_id);
            if native_lock.is_some() {
                // Marker removal settled in the leased metadata worker.
            } else if self.app_id == AppId::Torch {
                match std::fs::remove_file(&active_file) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(PumasError::io_with_path(error, &active_file)),
                }
            } else {
                self.mutation_tasks
                    .leased_transaction(&(), move || match std::fs::remove_file(&active_file) {
                        Ok(()) => Ok(()),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        Err(error) => Err(PumasError::io_with_path(error, &active_file)),
                    })
                    .await?;
            }
        }

        // Clear default if it was this version
        if self.default_version.as_deref() == Some(tag) {
            self.default_version = None;
        }

        debug!("Removed installed version: {}", tag);
        Ok(())
    }

    // ========================================
    // Validation
    // ========================================

    /// Validate all installations and remove incomplete ones.
    pub async fn validate_installations(&mut self) -> Result<ValidationResult> {
        self.mutation_tasks.ensure_open()?;
        let native_lock = self.native_versions_lock().await?;
        let lock = self.torch_versions_lock()?;
        if lock.is_some() || native_lock.is_some() {
            self.refresh_inner(lock.as_ref(), native_lock.as_ref())
                .await?;
        }
        self.validate_installations_inner(lock.as_ref(), native_lock.as_ref())
            .await
    }

    async fn validate_installations_inner(
        &mut self,
        lock: Option<&TorchVersionsLock>,
        native_lock: Option<&NativeVersionsLock>,
    ) -> Result<ValidationResult> {
        let versions_dir = self.launcher_root.join(self.app_id.versions_dir_name());
        let mut removed_tags = Vec::new();
        let mut orphaned_dirs = Vec::new();

        // Check metadata entries against filesystem
        let metadata = self.load_versions_metadata().await?;
        for (tag, info) in &metadata.installed {
            // Handle both old full-path metadata and the current tag-relative format.
            // and new format (just tag like "v0.4.0")
            let version_path = if info.path.contains('/') || info.path.contains('\\') {
                // Old format: path includes directory, use just the last component
                let path = std::path::Path::new(&info.path);
                let tag_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| info.path.clone());
                versions_dir.join(&tag_name)
            } else {
                // New format: path is just the tag
                versions_dir.join(&info.path)
            };

            if !self.is_version_complete(&version_path).await? {
                warn!(
                    "Incomplete installation found: {} at {}",
                    tag,
                    version_path.display()
                );
                removed_tags.push(tag.clone());
            }
        }

        // Remove incomplete installations - ONLY remove metadata, NOT files
        // Files may be intentionally incomplete during installation or user may want to recover
        for tag in &removed_tags {
            warn!(
                "Removing stale metadata entry for incomplete installation: {}",
                tag
            );
            self.remove_installed_version_inner(tag, lock, native_lock)
                .await?;
            // NOTE: We no longer delete files automatically to prevent data loss
            // Orphaned directories will be reported but not deleted
        }

        // Check for orphaned directories (exist on disk but not in metadata)
        if fs::try_exists(&versions_dir)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to check versions directory: {}", e),
                path: Some(versions_dir.clone()),
                source: Some(e),
            })?
        {
            let mut entries = fs::read_dir(&versions_dir)
                .await
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to read versions directory: {}", e),
                    path: Some(versions_dir.clone()),
                    source: Some(e),
                })?;

            while let Some(entry) = entries.next_entry().await.map_err(|e| PumasError::Io {
                message: format!("Failed to iterate versions directory: {}", e),
                path: Some(versions_dir.clone()),
                source: Some(e),
            })? {
                let dir_name = entry.file_name().to_string_lossy().to_string();
                let entry_path = entry.path();
                if entry
                    .file_type()
                    .await
                    .map_err(|e| PumasError::Io {
                        message: format!("Failed to inspect version entry: {}", e),
                        path: Some(entry_path.clone()),
                        source: Some(e),
                    })?
                    .is_dir()
                    && !self.installed_tags.contains(&dir_name)
                {
                    warn!("Orphaned version directory found: {}", dir_name);
                    orphaned_dirs.push(entry_path);
                }
            }
        }

        let valid_count = self.installed_tags.len();
        info!(
            "Validation complete: {} valid, {} removed, {} orphaned",
            valid_count,
            removed_tags.len(),
            orphaned_dirs.len()
        );

        Ok(ValidationResult {
            removed_tags,
            orphaned_dirs,
            valid_count,
        })
    }

    /// Check if a version installation is complete.
    async fn is_version_complete(&self, version_path: &Path) -> Result<bool> {
        if !fs::try_exists(version_path)
            .await
            .map_err(|e| PumasError::Io {
                message: format!("Failed to check version path: {}", e),
                path: Some(version_path.to_path_buf()),
                source: Some(e),
            })?
        {
            return Ok(false);
        }

        // Required files/directories for a complete installation
        let required = match self.app_id {
            AppId::Ollama => {
                // Ollama binary could be at root or in bin/ subdirectory (from tar extraction)
                // Check both locations - if either exists, consider it complete
                let bin_path = version_path.join("bin").join("ollama");
                let root_path = version_path.join("ollama");
                if fs::try_exists(&bin_path)
                    .await
                    .map_err(|e| PumasError::Io {
                        message: format!("Failed to check binary path: {}", e),
                        path: Some(bin_path.clone()),
                        source: Some(e),
                    })?
                {
                    vec![bin_path]
                } else {
                    vec![root_path]
                }
            }
            AppId::LlamaCpp => {
                let binary_name = if cfg!(windows) {
                    "llama-server.exe"
                } else {
                    "llama-server"
                };
                let candidates = [
                    version_path.join("bin").join(binary_name),
                    version_path.join(binary_name),
                    version_path.join("build").join("bin").join(binary_name),
                ];
                for candidate in &candidates {
                    if fs::try_exists(candidate)
                        .await
                        .map_err(|e| PumasError::Io {
                            message: format!("Failed to check binary path: {}", e),
                            path: Some(candidate.clone()),
                            source: Some(e),
                        })?
                    {
                        return Ok(true);
                    }
                }
                return Self::contains_llama_cpp_server_binary(version_path).await;
            }
            AppId::Torch
                if version_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(super::torch_component::component_revision) =>
            {
                vec![
                    version_path.join("runtime.json"),
                    version_path.join("component-manifest.json"),
                    version_path.join("installed-files.json"),
                    version_path.join("venv/lib/python3.12/site-packages"),
                ]
            }
            AppId::Torch => vec![
                version_path.join("runtime.json"),
                version_path.join("serve.py"),
                pumas_library::platform::paths::venv_python(version_path),
                version_path.join("requirements.txt"),
            ],
            _ => {
                // Generic check - just need the directory to exist
                vec![version_path.to_path_buf()]
            }
        };

        for required_path in required {
            if !fs::try_exists(&required_path)
                .await
                .map_err(|e| PumasError::Io {
                    message: format!("Failed to check required version path: {}", e),
                    path: Some(required_path.clone()),
                    source: Some(e),
                })?
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn contains_llama_cpp_server_binary(version_path: &Path) -> Result<bool> {
        let binary_names = if cfg!(windows) {
            ["llama-server.exe", "llama-server", "server.exe", "server"]
        } else {
            ["llama-server", "server", "llama-server.exe", "server.exe"]
        };
        let mut directories = vec![version_path.to_path_buf()];
        let mut inspected_entries = 0usize;

        while let Some(directory) = directories.pop() {
            let mut entries = fs::read_dir(&directory).await.map_err(|e| PumasError::Io {
                message: format!("Failed to read version directory: {}", e),
                path: Some(directory.clone()),
                source: Some(e),
            })?;

            while let Some(entry) = entries.next_entry().await.map_err(|e| PumasError::Io {
                message: format!("Failed to iterate version directory: {}", e),
                path: Some(directory.clone()),
                source: Some(e),
            })? {
                inspected_entries += 1;
                if inspected_entries > LLAMA_CPP_BINARY_SEARCH_LIMIT {
                    warn!(
                        "Stopping llama.cpp binary search after {} entries under {}",
                        LLAMA_CPP_BINARY_SEARCH_LIMIT,
                        version_path.display()
                    );
                    return Ok(false);
                }

                let entry_path = entry.path();
                let file_type = entry.file_type().await.map_err(|e| PumasError::Io {
                    message: format!("Failed to inspect version entry: {}", e),
                    path: Some(entry_path.clone()),
                    source: Some(e),
                })?;

                if file_type.is_file() {
                    if let Some(name) = entry_path.file_name().and_then(|name| name.to_str()) {
                        if binary_names.contains(&name) {
                            return Ok(true);
                        }
                    }
                } else if file_type.is_dir() {
                    directories.push(entry_path);
                }
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_state_for_app(app_id: AppId) -> (VersionState, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create required directories
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join(app_id.versions_dir_name())).unwrap();

        let metadata_manager = Arc::new(MetadataManager::new(temp_dir.path()));
        metadata_manager.ensure_directories().unwrap();

        let state = VersionState::new(temp_dir.path(), app_id, metadata_manager)
            .await
            .unwrap();

        (state, temp_dir)
    }

    async fn create_test_state() -> (VersionState, TempDir) {
        create_test_state_for_app(AppId::Torch).await
    }

    #[tokio::test]
    async fn native_public_state_mutations_share_installation_admission() {
        let (mut state, root) = create_test_state_for_app(AppId::LlamaCpp).await;
        let versions = root.path().join(AppId::LlamaCpp.versions_dir_name());
        std::fs::create_dir(versions.join("b1234+cpu")).unwrap();
        std::fs::write(versions.join("b1234+cpu/llama-server"), "complete").unwrap();
        let metadata = InstalledVersionMetadata {
            path: "b1234+cpu".into(),
            release_tag: "b1234".into(),
            ..Default::default()
        };
        state
            .add_installed_version("b1234+cpu", metadata.clone())
            .unwrap();
        state.set_active_version("b1234+cpu").await.unwrap();
        state.set_default_version(Some("b1234+cpu")).await.unwrap();
        let before = state
            .metadata_manager
            .load_versions(Some(AppId::LlamaCpp))
            .unwrap();
        let marker = super::super::active_version_path(root.path(), AppId::LlamaCpp);
        let before_marker = std::fs::read(&marker).unwrap();
        let lease = NativeVersionsLock::try_acquire(&versions).unwrap();
        assert!(
            VersionState::new(root.path(), AppId::LlamaCpp, state.metadata_manager.clone())
                .await
                .is_err()
        );
        assert!(state.refresh().await.is_err());
        assert!(state.set_active_version("b1234+cpu").await.is_err());
        assert!(state.set_default_version(None).await.is_err());
        assert!(state.add_installed_version("b1235+cpu", metadata).is_err());
        assert!(state.remove_installed_version("b1234+cpu").await.is_err());
        assert!(state.validate_installations().await.is_err());
        let after = state
            .metadata_manager
            .load_versions(Some(AppId::LlamaCpp))
            .unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(after).unwrap()
        );
        assert_eq!(std::fs::read(&marker).unwrap(), before_marker);
        assert!(versions.join("b1234+cpu/llama-server").exists());
        drop(lease);
        state.set_default_version(None).await.unwrap();
        state.remove_installed_version("b1234+cpu").await.unwrap();
        assert!(state
            .metadata_manager
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn receipt_review_buffered_state_error_survives_unpolled_waiter_drop() {
        let owner = StateMutationTasks::default();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let mut operation = Box::pin(owner.leased_transaction(&(), move || {
            release_rx.recv().unwrap();
            Err::<(), _>(PumasError::Other("buffered state error".into()))
        }));
        assert!(futures::poll!(operation.as_mut()).is_pending());
        release_tx.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while owner.has_active_tasks() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // The worker has sent its result and finished. Never poll the receiver
        // again: sending into its buffer is not caller observation.
        {
            let state = owner.state.lock().unwrap();
            assert_eq!(state.tasks.tasks.len(), 1);
            assert!(state.tasks.tasks[0].is_finished());
        }
        // A later admission harvests the finished worker without losing its receipt.
        owner.leased_transaction(&(), || Ok(())).await.unwrap();
        drop(operation);
        let error = owner.shutdown().await.unwrap_err().to_string();
        assert!(error.contains("buffered state error"));
        assert_eq!(owner.shutdown().await.unwrap_err().to_string(), error);
    }

    #[tokio::test]
    async fn receipt_review_observed_state_error_is_not_reported_again() {
        let owner = StateMutationTasks::default();
        let error = owner
            .leased_transaction(&(), || {
                Err::<(), _>(PumasError::Other("observed state error".into()))
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("observed state error"));
        owner.shutdown().await.unwrap();
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn native_metadata_worker_holds_lease_after_waiter_is_cancelled() {
        let root = TempDir::new().unwrap();
        let versions = root.path().join(AppId::LlamaCpp.versions_dir_name());
        let lease = NativeVersionsLock::try_acquire(&versions).unwrap();
        let metadata = Arc::new(MetadataManager::new(root.path()));
        metadata.ensure_directories().unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let owner = Arc::new(StateMutationTasks::default());
        let worker_owner = owner.clone();
        let waiter = tokio::spawn(async move {
            worker_owner
                .leased_transaction(&lease, move || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    metadata.update_installed_version(
                        "b1234+cpu",
                        InstalledVersionMetadata::default(),
                        Some(AppId::LlamaCpp),
                    )?;
                    finished_tx.send(()).unwrap();
                    Ok(())
                })
                .await
        });
        entered_rx.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(NativeVersionsLock::try_acquire(&versions).is_err());
        release_tx.send(()).unwrap();
        finished_rx.await.unwrap();
        owner.shutdown().await.unwrap();
        // The notification precedes lease release; await actual admission.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let Ok(lease) = NativeVersionsLock::try_acquire(&versions) {
                    drop(lease);
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(MetadataManager::new(root.path())
            .get_installed_version("b1234+cpu", Some(AppId::LlamaCpp))
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn state_mutation_shutdown_does_not_replay_delivered_errors() {
        let owner = StateMutationTasks::default();
        for _ in 0..3 {
            let error = owner
                .leased_transaction(&(), || -> Result<()> {
                    Err(PumasError::Other("delivered mutation error".into()))
                })
                .await
                .unwrap_err();
            assert!(error.to_string().contains("delivered mutation error"));
        }
        owner.shutdown().await.unwrap();
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn state_mutation_shutdown_drains_cancelled_waiters_and_retains_failures() {
        for terminal in ["success", "error", "panic"] {
            let (mut state, root) = create_test_state_for_app(AppId::LlamaCpp).await;
            let versions = root.path().join(AppId::LlamaCpp.versions_dir_name());
            let lease = NativeVersionsLock::try_acquire(&versions).unwrap();
            let owner = state.mutation_tasks();
            let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let effect = root.path().join("terminal-effect");
            let worker_effect = effect.clone();
            let waiter = tokio::spawn(async move {
                owner
                    .leased_transaction(&lease, move || {
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                        std::fs::write(worker_effect, terminal).unwrap();
                        match terminal {
                            "error" => Err(PumasError::Other("owned mutation failure".into())),
                            "panic" => panic!("owned mutation panic"),
                            _ => Ok(()),
                        }
                    })
                    .await
            });
            entered_rx.await.unwrap();
            waiter.abort();
            assert!(waiter.await.unwrap_err().is_cancelled());
            let outcome = {
                let drain = state.shutdown_mutations();
                tokio::pin!(drain);
                assert!(
                    tokio::time::timeout(std::time::Duration::from_millis(30), &mut drain)
                        .await
                        .is_err()
                );
                assert!(!effect.exists());
                assert!(NativeVersionsLock::try_acquire(&versions).is_err());
                release_tx.send(()).unwrap();
                drain.await.map_err(|error| error.to_string())
            };
            assert_eq!(
                state
                    .shutdown_mutations()
                    .await
                    .map_err(|error| error.to_string()),
                outcome
            );
            assert!(state.set_default_version(None).await.is_err());
            assert!(state
                .add_installed_version("after-close", InstalledVersionMetadata::default())
                .is_err());
            assert_eq!(std::fs::read_to_string(&effect).unwrap(), terminal);
            if terminal == "success" {
                outcome.unwrap();
            } else {
                assert!(outcome
                    .unwrap_err()
                    .to_string()
                    .contains(if terminal == "error" {
                        "owned mutation failure"
                    } else {
                        "owned mutation panic"
                    }));
            }
        }
    }

    #[tokio::test]
    async fn test_empty_state() {
        let (state, _temp) = create_test_state().await;
        assert!(state.get_installed_tags().is_empty());
        assert!(state.get_active_version().is_none());
        assert!(state.get_default_version().is_none());
    }

    #[tokio::test]
    async fn torch_initialization_loads_versions_when_cleanup_marker_is_unreadable() {
        let root = TempDir::new().unwrap();
        let versions_dir = root.path().join(AppId::Torch.versions_dir_name());
        std::fs::create_dir_all(&versions_dir).unwrap();
        let metadata_manager = Arc::new(MetadataManager::new(root.path()));
        metadata_manager.ensure_directories().unwrap();
        metadata_manager
            .update_installed_version(
                "v1.0.0",
                InstalledVersionMetadata {
                    path: "v1.0.0".into(),
                    release_tag: "v1.0.0".into(),
                    ..Default::default()
                },
                Some(AppId::Torch),
            )
            .unwrap();
        metadata_manager
            .set_default_version(Some("v1.0.0"), Some(AppId::Torch))
            .unwrap();

        let marker = versions_dir.join(".torch-pending-cleanup-.torch-install-corrupt");
        std::fs::write(&marker, [0xff]).unwrap();
        assert!(super::super::installer::retry_pending_torch_cleanup(
            &versions_dir,
            &metadata_manager
        )
        .is_err());

        let state = VersionState::new(root.path(), AppId::Torch, metadata_manager)
            .await
            .unwrap();

        assert!(state.is_installed("v1.0.0"));
        assert_eq!(state.get_default_version(), Some("v1.0.0".to_string()));
        assert_eq!(state.get_active_version(), Some("v1.0.0".to_string()));
        assert!(marker.exists());
    }

    #[tokio::test]
    async fn test_add_installed_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory with required files
        let version_dir = temp.path().join("torch-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        let python = pumas_library::platform::paths::venv_python(&version_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(python, "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };

        state.add_installed_version("v1.0.0", metadata).unwrap();

        assert!(state.is_installed("v1.0.0"));
        assert_eq!(state.get_installed_tags(), vec!["v1.0.0".to_string()]);
    }

    #[tokio::test]
    async fn test_set_active_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("torch-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        let python = pumas_library::platform::paths::venv_python(&version_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(python, "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Torch activation must preserve the llama.cpp runtime marker.
        std::fs::write(temp.path().join(".active-version"), "llama-existing").unwrap();
        state.set_active_version("v1.0.0").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(temp.path().join(".active-version")).unwrap(),
            "llama-existing"
        );
        assert_eq!(state.get_active_version(), Some("v1.0.0".to_string()));

        // Check file was written
        let active_file = temp.path().join(".active-version-torch");
        assert!(active_file.exists());
        assert_eq!(std::fs::read_to_string(active_file).unwrap(), "v1.0.0");
    }

    #[tokio::test]
    async fn test_set_active_version_not_installed() {
        let (mut state, _temp) = create_test_state().await;

        let result = state.set_active_version("v1.0.0").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_default_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("torch-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        let python = pumas_library::platform::paths::venv_python(&version_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(python, "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Set default
        state.set_default_version(Some("v1.0.0")).await.unwrap();
        assert_eq!(state.get_default_version(), Some("v1.0.0".to_string()));

        // Clear default
        state.set_default_version(None).await.unwrap();
        assert_eq!(state.get_default_version(), None);
    }

    #[tokio::test]
    async fn test_remove_installed_version() {
        let (mut state, temp) = create_test_state().await;

        // Create version directory
        let version_dir = temp.path().join("torch-versions/v1.0.0");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("main.py"), "# main").unwrap();
        let python = pumas_library::platform::paths::venv_python(&version_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(python, "#!/bin/python").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();
        state.set_active_version("v1.0.0").await.unwrap();
        state.set_default_version(Some("v1.0.0")).await.unwrap();

        // Remove
        state.remove_installed_version("v1.0.0").await.unwrap();

        assert!(!state.is_installed("v1.0.0"));
        assert!(state.get_active_version().is_none());
        assert!(state.get_default_version().is_none());
    }

    #[tokio::test]
    async fn legacy_torch_source_install_is_unregistered_without_deleting_files() {
        let (mut state, root) = create_test_state().await;
        let legacy = root.path().join("torch-versions/v2.10.0");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("user-file"), "preserve me").unwrap();
        state
            .add_installed_version(
                "v2.10.0",
                InstalledVersionMetadata {
                    path: "v2.10.0".into(),
                    release_tag: "v2.10.0".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let result = state.validate_installations().await.unwrap();
        assert_eq!(result.removed_tags, vec!["v2.10.0"]);
        assert_eq!(
            std::fs::read_to_string(legacy.join("user-file")).unwrap(),
            "preserve me"
        );
    }

    #[tokio::test]
    async fn test_validate_installations() {
        let (mut state, _temp) = create_test_state().await;

        // Add a version in metadata but don't create files (incomplete)
        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        // Validate - should remove incomplete
        let result = state.validate_installations().await.unwrap();
        assert!(result.removed_tags.contains(&"v1.0.0".to_string()));
        assert!(!state.is_installed("v1.0.0"));
    }

    #[tokio::test]
    async fn test_llama_cpp_nested_server_binary_is_complete() {
        let (mut state, temp) = create_test_state_for_app(AppId::LlamaCpp).await;
        let version_dir = temp
            .path()
            .join("llama-cpp-versions/v1.0.0/llama-b999-bin-ubuntu-x64");
        std::fs::create_dir_all(&version_dir).unwrap();
        std::fs::write(version_dir.join("llama-server"), "#!/bin/sh").unwrap();

        let metadata = InstalledVersionMetadata {
            path: "v1.0.0".to_string(),
            installed_date: "2024-01-01T00:00:00Z".to_string(),
            release_tag: "v1.0.0".to_string(),
            ..Default::default()
        };
        state.add_installed_version("v1.0.0", metadata).unwrap();

        let result = state.validate_installations().await.unwrap();

        assert!(result.removed_tags.is_empty());
        assert!(state.is_installed("v1.0.0"));
    }

    #[tokio::test]
    async fn test_llama_cpp_legacy_sycl_tag_migrates_to_precision_variant() {
        assert_legacy_sycl_migration(false).await;
    }

    #[tokio::test]
    async fn legacy_sycl_migration_retries_after_renames_before_metadata_save() {
        assert_legacy_sycl_migration(true).await;
    }

    async fn assert_legacy_sycl_migration(interrupt_after_rename: bool) {
        let temp_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/metadata")).unwrap();
        std::fs::create_dir_all(temp_dir.path().join("launcher-data/cache")).unwrap();
        let legacy_dir = temp_dir.path().join("llama-cpp-versions/b9090+sycl/bin");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(legacy_dir.join("llama-server"), "#!/bin/sh").unwrap();
        std::fs::write(temp_dir.path().join(".active-version"), "b9090+sycl").unwrap();

        let metadata_manager = Arc::new(MetadataManager::new(temp_dir.path()));
        metadata_manager.ensure_directories().unwrap();
        metadata_manager
            .update_installed_version(
                "b9090+sycl",
                InstalledVersionMetadata {
                    path: "b9090+sycl".to_string(),
                    installed_date: "2026-05-09T00:00:00Z".to_string(),
                    release_tag: "b9090+sycl".to_string(),
                    download_url: Some(
                        "https://github.com/ggml-org/llama.cpp/releases/download/b9090/llama-b9090-bin-ubuntu-sycl-fp16-x64.tar.gz"
                            .to_string(),
                    ),
                    ..Default::default()
                },
                Some(AppId::LlamaCpp),
            )
            .unwrap();
        metadata_manager
            .set_last_selected_version(Some("b9090+sycl"), Some(AppId::LlamaCpp))
            .unwrap();
        metadata_manager
            .set_default_version(Some("b9090+sycl"), Some(AppId::LlamaCpp))
            .unwrap();

        if interrupt_after_rename {
            let active = temp_dir.path().join(".active-version");
            std::fs::remove_file(&active).unwrap();
            std::fs::create_dir(&active).unwrap();
            assert!(
                VersionState::new(temp_dir.path(), AppId::LlamaCpp, metadata_manager.clone())
                    .await
                    .is_err()
            );
            let retained = metadata_manager
                .load_versions(Some(AppId::LlamaCpp))
                .unwrap();
            assert!(retained.installed.contains_key("b9090+sycl"));
            assert!(!retained.installed.contains_key("b9090+sycl-fp16"));
            assert!(!temp_dir
                .path()
                .join("llama-cpp-versions/b9090+sycl")
                .exists());
            assert!(temp_dir
                .path()
                .join("llama-cpp-versions/b9090+sycl-fp16/bin/llama-server")
                .exists());
            std::fs::remove_dir(&active).unwrap();
            std::fs::write(&active, "b9090+sycl").unwrap();
        }

        let state = VersionState::new(temp_dir.path(), AppId::LlamaCpp, metadata_manager.clone())
            .await
            .unwrap();

        assert!(!state.is_installed("b9090+sycl"));
        assert!(state.is_installed("b9090+sycl-fp16"));
        assert_eq!(
            state.get_active_version(),
            Some("b9090+sycl-fp16".to_string())
        );
        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join(".active-version")).unwrap(),
            "b9090+sycl-fp16"
        );
        assert!(temp_dir
            .path()
            .join("llama-cpp-versions/b9090+sycl-fp16/bin/llama-server")
            .exists());

        let versions = metadata_manager
            .load_versions(Some(AppId::LlamaCpp))
            .unwrap();
        assert!(versions.installed.contains_key("b9090+sycl-fp16"));
        assert!(!versions.installed.contains_key("b9090+sycl"));
        assert_eq!(
            versions.last_selected_version,
            Some("b9090+sycl-fp16".to_string())
        );
        assert_eq!(
            versions.default_version,
            Some("b9090+sycl-fp16".to_string())
        );
    }
}

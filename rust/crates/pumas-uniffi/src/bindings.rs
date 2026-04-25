// Private imports — core types used in From impls, not exposed in API signatures
use pumas_library::ipc::IpcClient;
use pumas_library::PumasApi;
use serde::de::DeserializeOwned;
use std::path::Path;
use std::sync::Arc;

mod ffi_types;

pub use ffi_types::*;

// =============================================================================
// FfiError — Simplified error type for the FFI boundary
// =============================================================================

/// FFI-friendly error type.
///
/// This is a simplified version of `PumasError` that can cross the FFI boundary.
/// Complex error types with embedded `std::io::Error` or `rusqlite::Error` are
/// converted to string representations.
#[derive(Debug, Clone, uniffi::Error, thiserror::Error)]
pub enum FfiError {
    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Timeout: {message}")]
    Timeout { message: String },

    #[error("Rate limited: {message}")]
    RateLimited { message: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Version error: {message}")]
    Version { message: String },

    #[error("Model error: {message}")]
    Model { message: String },

    #[error("Download error: {message}")]
    Download { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Launch failed: {message}")]
    Launch { message: String },

    #[error("Process error: {message}")]
    Process { message: String },

    #[error("Cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl From<pumas_library::PumasError> for FfiError {
    fn from(err: pumas_library::PumasError) -> Self {
        use pumas_library::PumasError;

        match err {
            PumasError::Network { message, .. } => FfiError::Network { message },
            PumasError::Timeout(duration) => FfiError::Timeout {
                message: format!("Request timed out after {:?}", duration),
            },
            PumasError::RateLimited {
                service,
                retry_after_secs,
            } => FfiError::RateLimited {
                message: format!(
                    "Rate limited by {}, retry after {:?} seconds",
                    service, retry_after_secs
                ),
            },
            PumasError::CircuitBreakerOpen { domain } => FfiError::Network {
                message: format!("Circuit breaker open for {}", domain),
            },
            PumasError::Database { message, .. } => FfiError::Database { message },
            PumasError::Io { message, path, .. } => FfiError::Io {
                message: match path {
                    Some(p) => format!("{}: {}", p.display(), message),
                    None => message,
                },
            },
            PumasError::FileNotFound(path) => FfiError::NotFound {
                resource: format!("File: {}", path.display()),
            },
            PumasError::NotFound { resource } => FfiError::NotFound { resource },
            PumasError::PermissionDenied(path) => FfiError::Io {
                message: format!("Permission denied: {}", path.display()),
            },
            PumasError::NotADirectory(path) => FfiError::Io {
                message: format!("Not a directory: {}", path.display()),
            },
            PumasError::SymlinkFailed { src, dest, reason } => FfiError::Io {
                message: format!(
                    "Failed to create symlink from {} to {}: {}",
                    src.display(),
                    dest.display(),
                    reason
                ),
            },
            PumasError::Json { message, .. } => FfiError::Io {
                message: format!("JSON error: {}", message),
            },
            PumasError::VersionNotFound { tag } => FfiError::Version {
                message: format!("Version not found: {}", tag),
            },
            PumasError::VersionAlreadyInstalled { tag } => FfiError::Version {
                message: format!("Version already installed: {}", tag),
            },
            PumasError::InstallationFailed { message } => FfiError::Version { message },
            PumasError::InstallationCancelled => FfiError::Cancelled,
            PumasError::DependencyFailed { message } => FfiError::Version { message },
            PumasError::LaunchFailed { app, message } => FfiError::Launch {
                message: format!("{}: {}", app, message),
            },
            PumasError::ProcessNotRunning { app } => FfiError::Process {
                message: format!("Process not running: {}", app),
            },
            PumasError::ModelNotFound { model_id } => FfiError::Model {
                message: format!("Model not found: {}", model_id),
            },
            PumasError::ImportFailed { message } => FfiError::Model { message },
            PumasError::DownloadFailed { url, message } => FfiError::Download {
                message: format!("{}: {}", url, message),
            },
            PumasError::DownloadCancelled | PumasError::DownloadPaused => FfiError::Cancelled,
            PumasError::HashMismatch { expected, actual } => FfiError::Validation {
                message: format!("Hash mismatch: expected {}, got {}", expected, actual),
            },
            PumasError::InvalidFileType { expected, actual } => FfiError::Validation {
                message: format!("Invalid file type: expected {}, got {}", expected, actual),
            },
            PumasError::GitHubApi {
                message,
                status_code,
            } => FfiError::Network {
                message: format!(
                    "GitHub API error ({}): {}",
                    status_code.unwrap_or(0),
                    message
                ),
            },
            PumasError::ReleaseNotFound { tag } => FfiError::NotFound {
                resource: format!("Release: {}", tag),
            },
            PumasError::Config { message } => FfiError::Config { message },
            PumasError::InvalidAppId(id) => FfiError::Validation {
                message: format!("Invalid app ID: {}", id),
            },
            PumasError::Validation { field, message } => FfiError::Validation {
                message: format!("{}: {}", field, message),
            },
            PumasError::InvalidVersionTag { tag } => FfiError::Validation {
                message: format!("Invalid version tag: {}", tag),
            },
            PumasError::InvalidParams { message } => FfiError::Validation { message },
            PumasError::SharedInstanceLost { pid, port } => FfiError::Other(format!(
                "Shared instance lost (PID {} on port {})",
                pid, port
            )),
            PumasError::NoLibrariesRegistered => FfiError::Config {
                message: "No libraries registered".to_string(),
            },
            PumasError::PrimaryInstanceBusy {
                library_path,
                pid,
                status,
            } => FfiError::Config {
                message: format!(
                    "Primary instance already active for {} (PID {}, status {})",
                    library_path.display(),
                    pid,
                    status
                ),
            },
            PumasError::PrimaryInstanceStartupTimeout {
                library_path,
                timeout,
            } => FfiError::Timeout {
                message: format!(
                    "Timed out waiting {:?} for primary startup at {}",
                    timeout,
                    library_path.display()
                ),
            },
            PumasError::TorchInference { message } => FfiError::Process {
                message: format!("Torch inference: {}", message),
            },
            PumasError::SlotNotFound { slot_id } => FfiError::NotFound {
                resource: format!("Model slot: {}", slot_id),
            },
            PumasError::DeviceNotAvailable { device } => FfiError::Config {
                message: format!("Device not available: {}", device),
            },
            PumasError::ConversionFailed { message } => FfiError::Model { message },
            PumasError::ConversionCancelled => FfiError::Cancelled,
            PumasError::QuantizationEnvNotReady { message, .. } => FfiError::Config { message },
            PumasError::Other(message) => FfiError::Other(message),
        }
    }
}

/// Result type for FFI operations.
pub type FfiResult<T> = Result<T, FfiError>;

// UniFFI scaffolding - this generates the FFI glue code
uniffi::setup_scaffolding!();

fn validate_required_string(value: String, field: &str) -> FfiResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(FfiError::Validation {
            message: format!("{field} must not be empty"),
        });
    }
    Ok(trimmed.to_string())
}

fn validate_path_string(value: String, field: &str) -> FfiResult<String> {
    let path = validate_required_string(value, field)?;
    if path.contains('\0') {
        return Err(FfiError::Validation {
            message: format!("{field} must not contain NUL bytes"),
        });
    }
    Ok(path)
}

/// Get the version of the pumas-uniffi bindings.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// =============================================================================
// FFI Wrapper Types
//
// All types exposed across the FFI boundary are defined here in pumas-uniffi
// rather than re-exported from pumas-library. This avoids UniFFI "external
// type" issues (Ruby backend doesn't support lifting external types) and
// provides a clean FFI boundary with explicit conversions.
// =============================================================================

// ---- Utility wrapper types (HashMap → Vec<KV>) ----

/// A key-value pair for model hashes (e.g. sha256, blake3).
#[derive(uniffi::Object)]
pub struct FfiPumasApi {
    inner: FfiApiInner,
}

enum FfiApiInner {
    Primary(Arc<PumasApi>),
    Client(Arc<IpcClient>),
}

impl FfiPumasApi {
    async fn new_with_default_root(launcher_root: String) -> Result<Arc<Self>, FfiError> {
        let launcher_root = validate_path_string(launcher_root, "launcher_root")?;
        if let Some(client) = Self::try_connect_client(&launcher_root).await? {
            return Ok(Arc::new(Self {
                inner: FfiApiInner::Client(client),
            }));
        }

        let api = PumasApi::new(&launcher_root)
            .await
            .map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: FfiApiInner::Primary(Arc::new(api)),
        }))
    }

    async fn new_with_configured_root(config: FfiApiConfig) -> Result<Arc<Self>, FfiError> {
        let launcher_root = validate_path_string(config.launcher_root, "launcher_root")?;
        if let Some(client) = Self::try_connect_client(&launcher_root).await? {
            return Ok(Arc::new(Self {
                inner: FfiApiInner::Client(client),
            }));
        }

        let api = PumasApi::builder(&launcher_root)
            .auto_create_dirs(config.auto_create_dirs)
            .with_hf_client(config.enable_hf)
            .with_process_manager(false)
            .build()
            .await
            .map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: FfiApiInner::Primary(Arc::new(api)),
        }))
    }

    async fn try_connect_client(launcher_root: &str) -> Result<Option<Arc<IpcClient>>, FfiError> {
        let registry = match pumas_library::registry::LibraryRegistry::open() {
            Ok(registry) => registry,
            Err(_) => return Ok(None),
        };

        let _ = registry.cleanup_stale();
        let Some(instance) = registry
            .get_instance(Path::new(launcher_root))
            .map_err(FfiError::from)?
        else {
            return Ok(None);
        };

        if !pumas_library::platform::is_process_alive(instance.pid) {
            let _ = registry.unregister_instance(Path::new(launcher_root));
            return Ok(None);
        }

        if instance.status == pumas_library::registry::InstanceStatus::Claiming {
            return Ok(None);
        }

        let addr = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, instance.port));
        match IpcClient::connect(addr, instance.pid).await {
            Ok(client) => Ok(Some(Arc::new(client))),
            Err(_) => {
                let _ = registry.unregister_instance(Path::new(launcher_root));
                Ok(None)
            }
        }
    }

    async fn call_client_method<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, FfiError> {
        let FfiApiInner::Client(client) = &self.inner else {
            return Err(FfiError::Other(format!(
                "IPC method {method} requested on a primary instance"
            )));
        };

        let value = client.call(method, params).await.map_err(FfiError::from)?;
        serde_json::from_value(value).map_err(|err| {
            FfiError::Other(format!("Failed to decode IPC response for {method}: {err}"))
        })
    }

    fn call_client_method_blocking<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, FfiError> {
        let FfiApiInner::Client(client) = &self.inner else {
            return Err(FfiError::Other(format!(
                "Blocking IPC method {method} requested on a primary instance"
            )));
        };

        let client = client.clone();
        let method_name = method.to_string();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| FfiError::Other(format!("Failed to create IPC runtime: {err}")))?
            .block_on(async move {
                let value = client
                    .call(&method_name, params)
                    .await
                    .map_err(FfiError::from)?;
                serde_json::from_value(value).map_err(|err| {
                    FfiError::Other(format!(
                        "Failed to decode IPC response for {}: {}",
                        method_name, err
                    ))
                })
            })
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// Create a new API instance with default options.
    #[uniffi::constructor]
    pub async fn new(launcher_root: String) -> Result<Arc<Self>, FfiError> {
        Self::new_with_default_root(launcher_root).await
    }

    /// Create a new API instance with a configuration record.
    #[uniffi::constructor]
    pub async fn with_config(config: FfiApiConfig) -> Result<Arc<Self>, FfiError> {
        Self::new_with_configured_root(config).await
    }

    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<FfiModelRecord>, FfiError> {
        let models = match &self.inner {
            FfiApiInner::Primary(api) => api.list_models().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("list_models", serde_json::json!({}))
                    .await?
            }
        };
        Ok(models.into_iter().map(FfiModelRecord::from).collect())
    }

    /// Get a single model by its ID.
    pub async fn get_model(&self, model_id: String) -> Result<Option<FfiModelRecord>, FfiError> {
        let model = match &self.inner {
            FfiApiInner::Primary(api) => api.get_model(&model_id).await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("get_model", serde_json::json!({ "model_id": model_id }))
                    .await?
            }
        };
        Ok(model.map(FfiModelRecord::from))
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: String,
        limit: u64,
        offset: u64,
    ) -> Result<FfiSearchResult, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => api
                .search_models(&query, limit as usize, offset as usize)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "search_models",
                    serde_json::json!({
                        "query": query,
                        "limit": limit,
                        "offset": offset,
                    }),
                )
                .await?
            }
        };
        Ok(FfiSearchResult::from(result))
    }

    /// Delete a model and all its links.
    pub async fn delete_model(&self, model_id: String) -> Result<FfiDeleteModelResponse, FfiError> {
        let resp = match &self.inner {
            FfiApiInner::Primary(api) => api
                .delete_model_with_cascade(&model_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "delete_model_with_cascade",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await?
            }
        };
        Ok(FfiDeleteModelResponse::from(resp))
    }

    /// Import a model from a local file path.
    pub async fn import_model(
        &self,
        spec: FfiModelImportSpec,
    ) -> Result<FfiModelImportResult, FfiError> {
        let core_spec = spec.into_core()?;
        let result = match &self.inner {
            FfiApiInner::Primary(api) => {
                api.import_model(&core_spec).await.map_err(FfiError::from)?
            }
            FfiApiInner::Client(_) => {
                self.call_client_method("import_model", serde_json::json!({ "spec": core_spec }))
                    .await?
            }
        };
        Ok(FfiModelImportResult::from(result))
    }

    /// Import multiple models in a batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<FfiModelImportSpec>,
    ) -> Vec<FfiModelImportResult> {
        let mut core_specs = Vec::with_capacity(specs.len());
        for spec in specs {
            match spec.into_core() {
                Ok(core_spec) => core_specs.push(core_spec),
                Err(err) => {
                    return vec![FfiModelImportResult {
                        path: String::new(),
                        success: false,
                        model_path: None,
                        error: Some(err.to_string()),
                        security_tier: None,
                    }];
                }
            }
        }
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .import_models_batch(core_specs)
                .await
                .into_iter()
                .map(FfiModelImportResult::from)
                .collect(),
            FfiApiInner::Client(_) => match self
                .call_client_method::<Vec<pumas_library::model_library::ModelImportResult>>(
                    "import_models_batch",
                    serde_json::json!({ "specs": core_specs.clone() }),
                )
                .await
            {
                Ok(results) => results
                    .into_iter()
                    .map(FfiModelImportResult::from)
                    .collect(),
                Err(err) => core_specs
                    .into_iter()
                    .map(|spec| {
                        FfiModelImportResult::from(
                            pumas_library::model_library::ModelImportResult {
                                path: spec.path,
                                success: false,
                                model_id: None,
                                model_path: None,
                                error: Some(err.to_string()),
                                security_tier: None,
                            },
                        )
                    })
                    .collect(),
            },
        }
    }

    /// Rebuild the full-text search index for all models.
    pub async fn rebuild_model_index(&self) -> Result<u64, FfiError> {
        let count: usize = match &self.inner {
            FfiApiInner::Primary(api) => api.rebuild_model_index().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("rebuild_model_index", serde_json::json!({}))
                    .await?
            }
        };
        Ok(count as u64)
    }

    /// Re-detect a model's type and move it to the correct directory if misclassified.
    ///
    /// Returns the new model_id if the model was reclassified, None if unchanged.
    pub async fn reclassify_model(&self, model_id: String) -> Result<Option<String>, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .reclassify_model(&model_id)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "reclassify_model",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await
            }
        }
    }

    /// Re-detect and reclassify all models in the library.
    ///
    /// Scans every model, re-detects its type from file content, and moves
    /// any misclassified models to the correct directory.
    pub async fn reclassify_all_models(&self) -> Result<FfiReclassifyResult, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => {
                api.reclassify_all_models().await.map_err(FfiError::from)?
            }
            FfiApiInner::Client(_) => {
                self.call_client_method("reclassify_all_models", serde_json::json!({}))
                    .await?
            }
        };
        Ok(FfiReclassifyResult::from(result))
    }

    /// Get the inference settings schema for a model.
    ///
    /// Returns the stored settings if present, otherwise lazily computes
    /// defaults based on model type and format.
    pub async fn get_inference_settings(
        &self,
        model_id: String,
    ) -> Result<Vec<FfiInferenceParamSchema>, FfiError> {
        let settings = match &self.inner {
            FfiApiInner::Primary(api) => api
                .get_inference_settings(&model_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "get_inference_settings",
                    serde_json::json!({ "model_id": model_id }),
                )
                .await?
            }
        };
        Ok(settings
            .into_iter()
            .map(FfiInferenceParamSchema::from)
            .collect())
    }

    /// Replace the inference settings schema for a model.
    pub async fn update_inference_settings(
        &self,
        model_id: String,
        settings: Vec<FfiInferenceParamSchema>,
    ) -> Result<(), FfiError> {
        let core_settings: Vec<pumas_library::models::InferenceParamSchema> =
            settings.into_iter().map(Into::into).collect();
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .update_inference_settings(&model_id, core_settings)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                let _: serde_json::Value = self
                    .call_client_method(
                        "update_inference_settings",
                        serde_json::json!({
                            "model_id": model_id,
                            "settings": core_settings,
                        }),
                    )
                    .await?;
                Ok(())
            }
        }
    }

    // ========================================
    // HuggingFace Methods
    // ========================================

    /// Search for models on HuggingFace.
    pub async fn search_hf_models(
        &self,
        query: String,
        kind: Option<String>,
        limit: u64,
    ) -> Result<Vec<FfiHuggingFaceModel>, FfiError> {
        let models = match &self.inner {
            FfiApiInner::Primary(api) => api
                .search_hf_models(&query, kind.as_deref(), limit as usize)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "search_hf_models",
                    serde_json::json!({
                        "query": query,
                        "kind": kind,
                        "limit": limit,
                    }),
                )
                .await?
            }
        };
        Ok(models.into_iter().map(FfiHuggingFaceModel::from).collect())
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(&self, request: FfiDownloadRequest) -> Result<String, FfiError> {
        let core_req = request.into_core()?;
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .start_hf_download(&core_req)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "start_hf_download",
                    serde_json::json!({ "request": core_req }),
                )
                .await
            }
        }
    }

    /// Get the progress of an active HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: String,
    ) -> Option<FfiModelDownloadProgress> {
        let progress = match &self.inner {
            FfiApiInner::Primary(api) => api.get_hf_download_progress(&download_id).await,
            FfiApiInner::Client(_) => self
                .call_client_method(
                    "get_hf_download_progress",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await
                .ok()
                .flatten(),
        };
        progress.map(FfiModelDownloadProgress::from)
    }

    /// Cancel an active HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: String) -> Result<bool, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .cancel_hf_download(&download_id)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "cancel_hf_download",
                    serde_json::json!({ "download_id": download_id }),
                )
                .await
            }
        }
    }

    /// List interrupted downloads that lost their persistence state.
    pub async fn list_interrupted_downloads(&self) -> Vec<FfiInterruptedDownload> {
        let downloads: Vec<pumas_library::model_library::InterruptedDownload> = match &self.inner {
            FfiApiInner::Primary(api) => api.list_interrupted_downloads().await,
            FfiApiInner::Client(_) => self
                .call_client_method_blocking("list_interrupted_downloads", serde_json::json!({}))
                .unwrap_or_default(),
        };
        downloads
            .into_iter()
            .map(FfiInterruptedDownload::from)
            .collect()
    }

    /// Recover an interrupted download by providing the correct repo_id.
    pub async fn recover_download(
        &self,
        repo_id: String,
        dest_dir: String,
    ) -> Result<String, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api
                .recover_download(&repo_id, &dest_dir)
                .await
                .map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "recover_download",
                    serde_json::json!({
                        "repo_id": repo_id,
                        "dest_dir": dest_dir,
                    }),
                )
                .await
            }
        }
    }

    /// Look up HuggingFace metadata for a local model file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: String,
    ) -> Result<Option<FfiHfMetadataResult>, FfiError> {
        let result = match &self.inner {
            FfiApiInner::Primary(api) => api
                .lookup_hf_metadata_for_file(&file_path)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "lookup_hf_metadata_for_file",
                    serde_json::json!({ "file_path": file_path }),
                )
                .await?
            }
        };
        Ok(result.map(FfiHfMetadataResult::from))
    }

    /// Get the file tree for a HuggingFace repository.
    pub async fn get_hf_repo_files(&self, repo_id: String) -> Result<FfiRepoFileTree, FfiError> {
        let tree = match &self.inner {
            FfiApiInner::Primary(api) => api
                .get_hf_repo_files(&repo_id)
                .await
                .map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method(
                    "get_hf_repo_files",
                    serde_json::json!({ "repo_id": repo_id }),
                )
                .await?
            }
        };
        Ok(FfiRepoFileTree::from(tree))
    }

    // ========================================
    // System Info Methods
    // ========================================

    /// Check if the network is currently online.
    pub fn is_online(&self) -> bool {
        match &self.inner {
            FfiApiInner::Primary(api) => api.is_online(),
            FfiApiInner::Client(_) => self
                .call_client_method_blocking("is_online", serde_json::json!({}))
                .unwrap_or(false),
        }
    }

    /// Get disk space information for the launcher root.
    pub async fn get_disk_space(&self) -> Result<FfiDiskSpaceResponse, FfiError> {
        let resp = match &self.inner {
            FfiApiInner::Primary(api) => api.get_disk_space().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("get_disk_space", serde_json::json!({}))
                    .await?
            }
        };
        Ok(FfiDiskSpaceResponse::from(resp))
    }

    /// Get overall system status including running processes and resources.
    pub async fn get_status(&self) -> Result<FfiStatusResponse, FfiError> {
        let resp = match &self.inner {
            FfiApiInner::Primary(api) => api.get_status().await.map_err(FfiError::from)?,
            FfiApiInner::Client(_) => {
                self.call_client_method("get_status_response", serde_json::json!({}))
                    .await?
            }
        };
        Ok(FfiStatusResponse::from(resp))
    }

    /// Get current system resource usage (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<FfiSystemResourcesResponse, FfiError> {
        let resp = match &self.inner {
            FfiApiInner::Primary(api) => {
                api.get_system_resources().await.map_err(FfiError::from)?
            }
            FfiApiInner::Client(_) => {
                self.call_client_method("get_system_resources", serde_json::json!({}))
                    .await?
            }
        };
        Ok(FfiSystemResourcesResponse::from(resp))
    }

    // ========================================
    // Torch Inference Methods
    // ========================================

    /// Check if the Torch inference server is running.
    pub async fn is_torch_running(&self) -> bool {
        match &self.inner {
            FfiApiInner::Primary(api) => api.is_torch_running().await,
            FfiApiInner::Client(_) => self
                .call_client_method("is_torch_running", serde_json::json!({}))
                .await
                .unwrap_or(false),
        }
    }

    /// Stop the Torch inference server.
    pub async fn torch_stop(&self) -> Result<bool, FfiError> {
        match &self.inner {
            FfiApiInner::Primary(api) => api.stop_torch().await.map_err(FfiError::from),
            FfiApiInner::Client(_) => {
                self.call_client_method("stop_torch", serde_json::json!({}))
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::models::HuggingFaceModel;
    use pumas_library::{ModelRecord, SearchResult};

    #[test]
    fn test_ffi_error_conversion() {
        let err = pumas_library::PumasError::ModelNotFound {
            model_id: "test-model".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Model { .. }));
    }

    #[test]
    fn test_ffi_error_launch_variant() {
        let err = pumas_library::PumasError::LaunchFailed {
            app: "ollama".to_string(),
            message: "port in use".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Launch { .. }));
        if let FfiError::Launch { message } = ffi_err {
            assert!(message.contains("ollama"));
            assert!(message.contains("port in use"));
        }
    }

    #[test]
    fn test_ffi_error_process_variant() {
        let err = pumas_library::PumasError::ProcessNotRunning {
            app: "comfyui".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Process { .. }));
    }

    #[test]
    fn test_ffi_model_record_conversion() {
        let mut hashes = std::collections::HashMap::new();
        hashes.insert("sha256".to_string(), "abc123".to_string());

        let record = ModelRecord {
            id: "test-id".to_string(),
            path: "/models/test".to_string(),
            cleaned_name: "test-model".to_string(),
            official_name: "Test Model".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["tag1".to_string()],
            hashes,
            metadata: serde_json::json!({"key": "value"}),
            updated_at: "2025-01-01".to_string(),
        };

        let ffi_record = FfiModelRecord::from(record);
        assert_eq!(ffi_record.id, "test-id");
        assert_eq!(ffi_record.official_name, "Test Model");
        assert_eq!(ffi_record.hashes.len(), 1);
        assert_eq!(ffi_record.hashes[0].key, "sha256");
        assert_eq!(ffi_record.hashes[0].value, "abc123");
        assert!(ffi_record.metadata_json.contains("key"));
    }

    #[test]
    fn test_ffi_search_result_conversion() {
        let result = SearchResult {
            models: vec![],
            total_count: 42,
            query_time_ms: 1.5,
            query: "test".to_string(),
        };

        let ffi_result = FfiSearchResult::from(result);
        assert_eq!(ffi_result.total_count, 42);
        assert_eq!(ffi_result.query, "test");
    }

    #[test]
    fn test_ffi_huggingface_model_quant_sizes() {
        let mut quant_sizes = std::collections::HashMap::new();
        quant_sizes.insert("Q4_K_M".to_string(), 4_200_000_000u64);
        quant_sizes.insert("Q8_0".to_string(), 8_100_000_000u64);

        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            model_card: None,
            license: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: Some(quant_sizes),
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert_eq!(ffi_model.quant_sizes.len(), 2);

        let q4 = ffi_model.quant_sizes.iter().find(|qs| qs.quant == "Q4_K_M");
        assert!(q4.is_some());
        assert_eq!(q4.unwrap().size_bytes, 4_200_000_000);
    }

    #[test]
    fn test_ffi_huggingface_model_no_quant_sizes() {
        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            model_card: None,
            license: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert!(ffi_model.quant_sizes.is_empty());
    }

    #[test]
    fn test_ffi_download_request_conversion_defaults_unexposed_hf_metadata() {
        let ffi_request = FfiDownloadRequest {
            repo_id: "repo/model".to_string(),
            family: "diffusion".to_string(),
            official_name: "Model".to_string(),
            model_type: Some("diffusion".to_string()),
            quant: None,
            filename: Some("model.safetensors".to_string()),
            filenames: None,
            pipeline_tag: Some("text-to-image".to_string()),
        };

        let request = ffi_request.into_core().unwrap();
        assert_eq!(request.repo_id, "repo/model");
        assert_eq!(request.family, "diffusion");
        assert_eq!(request.official_name, "Model");
        assert_eq!(request.model_type.as_deref(), Some("diffusion"));
        assert_eq!(request.filename.as_deref(), Some("model.safetensors"));
        assert_eq!(request.pipeline_tag.as_deref(), Some("text-to-image"));
        assert!(request.bundle_format.is_none());
        assert!(request.pipeline_class.is_none());
        assert!(request.release_date.is_none());
        assert!(request.download_url.is_none());
        assert!(request.model_card_json.is_none());
        assert!(request.license_status.is_none());
    }

    #[test]
    fn test_ffi_download_request_rejects_empty_required_fields() {
        let ffi_request = FfiDownloadRequest {
            repo_id: " ".to_string(),
            family: "diffusion".to_string(),
            official_name: "Model".to_string(),
            model_type: None,
            quant: None,
            filename: None,
            filenames: None,
            pipeline_tag: None,
        };

        let error = ffi_request.into_core().unwrap_err();
        assert!(matches!(error, FfiError::Validation { .. }));
    }

    #[test]
    fn test_ffi_import_spec_rejects_empty_path() {
        let spec = FfiModelImportSpec {
            path: " ".to_string(),
            family: "llm".to_string(),
            official_name: "Model".to_string(),
            repo_id: None,
            model_type: None,
            subtype: None,
            tags: None,
            security_acknowledged: None,
        };

        let error = spec.into_core().unwrap_err();
        assert!(matches!(error, FfiError::Validation { .. }));
    }

    #[test]
    fn test_validate_path_string_rejects_nul_bytes() {
        let error = validate_path_string("abc\0def".to_string(), "launcher_root").unwrap_err();
        assert!(matches!(error, FfiError::Validation { .. }));
    }
}

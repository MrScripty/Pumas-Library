//! Installed audio producer: original library selection plus retained runtime bytes.
//! Public byte custody is not an execution grant; the source-owned policy is closed.
use crate::models::{RuntimeProfileId, ServeModelRequest, ServeModelResponse};
use crate::runtime_read_source::RetainedRuntimeReadSource;
use crate::serving::ServingLoadOperation;
use crate::{PumasApi, PumasError, Result};
use std::sync::Arc;

/// Observed local experiment facts; never a qualification or execution grant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExperimentalLocalCohereReport {
    pub experimental: bool,
    pub production_available: bool,
    pub model_id: String,
    pub selected_artifact_id: String,
    pub observed_model_manifest_sha256: String,
    pub runtime_recipe_sha256: String,
    pub runtime_code_manifest_sha256: String,
    pub device: &'static str,
    pub host_landlock_abi: i32,
    pub profile_generation: u64,
    pub child_drained: bool,
    pub warning: &'static str,
}

#[derive(Debug, serde::Serialize)]
pub struct ExperimentalLocalCohereLoadResponse {
    #[serde(flatten)]
    pub serving: ServeModelResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_local_cohere: Option<ExperimentalLocalCohereReport>,
}

#[derive(Clone, Copy)]
enum AudioMode {
    Shipping,
    ExperimentalLocalCohere,
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
type ReportSlot = Arc<std::sync::Mutex<Option<ExperimentalLocalCohereReport>>>;

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
struct CancelAudio {
    profiles: Arc<crate::runtime_profiles::audio_profile_owner::AudioProfileOwner>,
    profile: RuntimeProfileId,
    generation: u64,
    armed: bool,
}
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
impl Drop for CancelAudio {
    fn drop(&mut self) {
        if self.armed {
            self.profiles
                .cancel_generation(&self.profile, self.generation);
        }
    }
}

impl PumasApi {
    /// Load only the original indexed selection through the source-owned installed
    /// audio qualifier. No executable, manifest, source ID or approval flag is accepted.
    pub async fn serve_installed_audio_for_operation(
        &self,
        request: ServeModelRequest,
        operation: &ServingLoadOperation,
        installed: impl std::future::Future<Output = Result<Arc<RetainedRuntimeReadSource>>>
            + Send
            + 'static,
    ) -> Result<ServeModelResponse> {
        Ok(self
            .serve_audio_for_operation(request, operation, installed, AudioMode::Shipping)
            .await?
            .serving)
    }

    /// Explicit local experiment using only the fixed Cohere CPU policy. This
    /// does not qualify the runtime or model for production execution.
    pub async fn serve_experimental_local_cohere_for_operation(
        &self,
        request: ServeModelRequest,
        operation: &ServingLoadOperation,
        installed: impl std::future::Future<Output = Result<Arc<RetainedRuntimeReadSource>>>
            + Send
            + 'static,
    ) -> Result<ExperimentalLocalCohereLoadResponse> {
        self.serve_audio_for_operation(
            request,
            operation,
            installed,
            AudioMode::ExperimentalLocalCohere,
        )
        .await
    }

    async fn serve_audio_for_operation(
        &self,
        request: ServeModelRequest,
        operation: &ServingLoadOperation,
        installed: impl std::future::Future<Output = Result<Arc<RetainedRuntimeReadSource>>>
            + Send
            + 'static,
        mode: AudioMode,
    ) -> Result<ExperimentalLocalCohereLoadResponse> {
        self.try_primary()?;
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        {
            use crate::models::{
                RuntimeDeviceMode, RuntimeProviderId, ServedModelLoadState, ServedModelStatus,
            };
            if request.config.provider != RuntimeProviderId::Torch
                || !matches!(
                    request.config.device_mode,
                    RuntimeDeviceMode::Auto | RuntimeDeviceMode::Cpu
                )
                || request.config.device_id.is_some()
                || request.config.gpu_layers.is_some()
                || request.config.tensor_split.is_some()
                || request.config.context_size.is_some()
            {
                return Err(PumasError::InvalidParams {
                    message: "installed audio requires the source-pinned CPU Torch policy".into(),
                });
            }
            // Read-only kernel query precedes every manager/copy/native effect.
            let abi = crate::platform::audio_read_boundary::AudioReadBoundary::supported_abi()
                .map_err(|_| PumasError::InvalidParams { message: "Installed Cohere audio requires Linux x86_64 with working Landlock ABI >= 6. Use a kernel exposing that capability; confinement cannot be disabled or downgraded.".into() })?;
            let report: ReportSlot = Arc::default();
            let prepared_report = report.clone();
            let primary = self.primary();
            let library = primary.model_library.clone();
            let model = request.model_id.clone();
            let tasks = primary.runtime_tasks.clone();
            let (generation, _endpoint) = primary
                .runtime_profile_service
                .launch_installed_audio_profile(
                    primary.model_library.clone(),
                    request.config.profile_id.clone(),
                    request.model_id.clone(),
                    primary.serving_service.clone(),
                    move |guard| {
                        prepare_audio(
                            tasks,
                            library,
                            model,
                            installed,
                            guard,
                            (mode, abi, prepared_report),
                        )
                    },
                )
                .await?;
            let mut cancel = CancelAudio {
                profiles: primary.runtime_profile_service.audio_profiles.clone(),
                profile: request.config.profile_id.clone(),
                generation,
                armed: true,
            };
            let status = ServedModelStatus {
                model_id: request.model_id,
                model_alias: request.config.model_alias,
                provider: RuntimeProviderId::Torch,
                profile_id: request.config.profile_id.clone(),
                load_state: ServedModelLoadState::Loaded,
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: None,
                keep_loaded: request.config.keep_loaded,
                endpoint_url: None,
                memory_bytes: None,
                loaded_at: None,
                last_error: None,
            };
            let snapshot = primary
                .runtime_profile_service
                .audio_profiles
                .with_running_generation(&request.config.profile_id, generation, || {
                    primary.serving_service.record_loaded_model_for_operation(
                        operation,
                        status.clone(),
                        None,
                    )
                })?;
            let mut report = report
                .lock()
                .map_err(|_| PumasError::Other("experimental report owner lost".into()))?
                .take();
            if let Some(report) = &mut report {
                report.profile_generation = generation;
            }
            cancel.armed = false;
            Ok(ExperimentalLocalCohereLoadResponse {
                serving: ServeModelResponse {
                    success: true,
                    error: None,
                    loaded: true,
                    loaded_models_unchanged: false,
                    status: Some(status),
                    load_error: None,
                    snapshot: Some(snapshot),
                },
                experimental_local_cohere: report,
            })
        }
        #[cfg(not(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        )))]
        {
            let _ = (request, operation, installed, mode);
            Err(PumasError::Other(
                "Installed Cohere audio requires Linux x86_64 with working Landlock ABI >= 6; no weaker platform path is available".into(),
            ))
        }
    }

    /// Stop the original audio generation selected by this model, never a
    /// replacement HTTP process or a later audio generation.
    pub async fn stop_installed_audio_model(
        &self,
        profile: &RuntimeProfileId,
        model: &str,
    ) -> Result<bool> {
        self.try_primary()?;
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        {
            let owner = &self.primary().runtime_profile_service.audio_profiles;
            let Some(generation) = owner.generation_for_model(profile, model)? else {
                return Ok(false);
            };
            owner
                .stop(profile, Some(generation))
                .await?
                .unwrap_or(Ok(false))
        }
        #[cfg(not(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        )))]
        {
            let _ = (profile, model);
            Ok(false)
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
type PreparedAudio = (
    Arc<crate::runtime_profiles::audio_runtime::AudioRuntimeOwner>,
    Arc<crate::model_library::artifact_use::PreparedArtifactUse>,
);

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
async fn prepare_audio(
    tasks: super::runtime_tasks::RuntimeTasks,
    library: Arc<crate::model_library::ModelLibrary>,
    model: String,
    installed: impl std::future::Future<Output = Result<Arc<RetainedRuntimeReadSource>>>
        + Send
        + 'static,
    guard: Arc<crate::runtime_profiles::RuntimeProfileOperationGuard>,
    experiment: (AudioMode, i32, ReportSlot),
) -> Result<PreparedAudio> {
    let (mode, abi, report) = experiment;
    let installed = installed.await.map_err(|error| {
        tracing::warn!(%error, "Installed Cohere runtime preparation refused");
        PumasError::Other("Installed Cohere runtime preparation refused: install and select the source-pinned CPU cohere-asr runtime; the retained runtime/native recipe must match this build".into())
    })?;
    let prepared = tasks.start_owned(
        "prepare installed audio selection",
        move |context| async move {
            context
                .run_blocking("copy original audio model and retain runtime", move || {
                    let _profile_guard = guard;
                    prepare_selected_audio(library, model, installed, mode, abi, report)
                })
                .await
        },
    )?;
    prepared
        .await
        .map_err(|_| PumasError::Other("audio preparation owner lost".into()))??
}

#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
fn prepare_selected_audio(
    library: Arc<crate::model_library::ModelLibrary>,
    model: String,
    installed: Arc<RetainedRuntimeReadSource>,
    mode: AudioMode,
    abi: i32,
    report: ReportSlot,
) -> Result<PreparedAudio> {
    use crate::runtime_profiles::audio_runtime::installed::InstalledAudioRuntimeCandidate;
    use crate::runtime_read_source::RuntimeReadRole;
    let record = library
        .index()
        .get(&model)?
        .ok_or_else(|| PumasError::ModelNotFound {
            model_id: model.clone(),
        })?;
    let metadata: crate::models::ModelMetadata = serde_json::from_value(record.metadata)?;
    let artifact = metadata
        .selected_artifact_id
        .ok_or_else(|| PumasError::Other("audio model has no selected artifact".into()))?;
    let selected = Arc::new(library.prepare_cohere_artifact_use(&model, &artifact)?);
    // Select only the source-pinned interpreter member from held bytes.
    // The digest binds a candidate; it never changes qualification.
    let recipe: serde_json::Value =
        serde_json::from_str(crate::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE)?;
    let expected = recipe["interpreter_executable_sha256"]
        .as_str()
        .ok_or_else(|| PumasError::Other("source interpreter recipe missing".into()))?;
    let members: Vec<_> = installed
        .manifest()
        .filter(|(role, member)| {
            *role == RuntimeReadRole::Interpreter
                && member.path().ends_with("/bin/python3.12")
                && member.sha256() == expected
        })
        .map(|(_, member)| member.path().to_owned())
        .collect();
    let [interpreter] = members.as_slice() else {
        return Err(PumasError::Other(
            "source-pinned interpreter selection unavailable".into(),
        ));
    };
    let candidate =
        InstalledAudioRuntimeCandidate::capture(installed, interpreter, selected.clone())?;
    use crate::runtime_profiles::audio_runtime::AudioRuntimeOwner;
    let runtime = match mode {
        AudioMode::Shipping => AudioRuntimeOwner::for_installed_runtime(candidate, &selected),
        AudioMode::ExperimentalLocalCohere => AudioRuntimeOwner::for_experimental_local_cohere(candidate, &selected),
    }.map_err(|error| PumasError::Other(match error {
        crate::runtime_profiles::audio_custody::AudioCustodyError::ReadConfinementUnavailable => "Installed Cohere audio requires working Landlock ABI >= 6; confinement cannot be disabled",
        _ => "Installed Cohere runtime/model refused: use the source-pinned CPU runtime and a valid indexed LibraryOwned Cohere artifact; arbitrary runtimes and repository code are not accepted",
    }.into()))?;
    if matches!(mode, AudioMode::ExperimentalLocalCohere) {
        use sha2::{Digest, Sha256};
        *report.lock().map_err(|_| PumasError::Other("experimental report owner lost".into()))? = Some(ExperimentalLocalCohereReport {
            experimental: true, production_available: false,
            model_id: selected.model_id().to_owned(),
            selected_artifact_id: selected.selected_artifact_id().to_owned(),
            observed_model_manifest_sha256: selected.manifest_sha256().to_owned(),
            runtime_recipe_sha256: hex::encode(Sha256::digest(crate::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE.as_bytes())),
            runtime_code_manifest_sha256: runtime.manifest_sha256().to_owned(),
            device: "cpu", host_landlock_abi: abi, profile_generation: 0,
            child_drained: false,
            warning: "Experimental local Cohere execution. Real-model ASR and native disposal are not qualified; unload must stop and join the exact child.",
        });
    }
    Ok((runtime, selected))
}

#[cfg(all(
    test,
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
mod tests {
    use super::*;
    use crate::models::{ModelServingConfig, RuntimeDeviceMode, RuntimeProviderId};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn unsupported_host_refuses_before_polling_preparation() {
        if crate::platform::audio_read_boundary::AudioReadBoundary::supported_abi().is_ok() {
            // This regression covers the actual unavailable-host branch; supported
            // hosts exercise confined launch in the separate protocol tests.
            return;
        }
        let root = tempfile::TempDir::new().unwrap();
        let api = PumasApi::builder(root.path())
            .with_registry(
                crate::registry::LibraryRegistry::open_at(&root.path().join("registry.db"))
                    .unwrap(),
            )
            .with_connectivity_probe(false)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let request = ServeModelRequest {
            model_id: "audio/local/cohere".into(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::Torch,
                profile_id: RuntimeProfileId::parse("cohere-local").unwrap(),
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: None,
                keep_loaded: false,
                model_alias: None,
            },
        };
        let operation = api.begin_serving_load(&request).unwrap();
        let polled = Arc::new(AtomicBool::new(false));
        let observed = polled.clone();
        let result = api
            .serve_experimental_local_cohere_for_operation(request, &operation, async move {
                observed.store(true, Ordering::SeqCst);
                Err(PumasError::Other("preparation should not run".into()))
            })
            .await;
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Landlock ABI >= 6"));
        assert!(!polled.load(Ordering::SeqCst));
        drop(operation);
        api.shutdown_instance().await.unwrap();
    }
}

//! Installed audio producer: original library selection plus retained runtime bytes.
//! Public byte custody is not an execution grant; the source-owned policy is closed.
use crate::models::{RuntimeProfileId, ServeModelRequest, ServeModelResponse};
use crate::runtime_read_source::RetainedRuntimeReadSource;
use crate::serving::ServingLoadOperation;
use crate::{PumasApi, PumasError, Result};
use std::sync::Arc;

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
                    move |guard| prepare_audio(tasks, library, model, installed, guard),
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
            cancel.armed = false;
            Ok(ServeModelResponse {
                success: true,
                error: None,
                loaded: true,
                loaded_models_unchanged: false,
                status: Some(status),
                load_error: None,
                snapshot: Some(snapshot),
            })
        }
        #[cfg(not(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        )))]
        {
            let _ = (request, operation, installed);
            Err(PumasError::Other(
                "installed audio is not qualified on this platform".into(),
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
) -> Result<PreparedAudio> {
    let installed = installed.await?;
    let prepared = tasks.start_owned(
        "prepare installed audio selection",
        move |context| async move {
            context
                .run_blocking("copy original audio model and retain runtime", move || {
                    let _profile_guard = guard;
                    prepare_selected_audio(library, model, installed)
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
    let runtime = crate::runtime_profiles::audio_runtime::AudioRuntimeOwner::for_installed_runtime(
        candidate, &selected,
    )
    .map_err(|_| PumasError::Other("installed audio qualification unavailable".into()))?;
    Ok((runtime, selected))
}

//! Torch serving through canonical profile ownership and library-resolved assets.

use super::serving::{
    current_serving_snapshot, decorate_serving_snapshot, non_critical_failure_response,
    serving_error,
};
use crate::server::AppState;
use pumas_app_manager::torch_client::{
    ComputeDevice, ImageModelComponents, SlotState, TorchClient,
};
use pumas_library::models::{
    AssetValidationState, ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId,
    ServeModelRequest, ServeModelResponse, ServedModelLoadState, ServedModelStatus,
    UnserveModelRequest, UnserveModelResponse,
};
use serde_json::Value;
use std::path::PathBuf;

const NUNCHAKU_REPOSITORIES: &[&str] = &[
    "nunchaku-ai/nunchaku-z-image-turbo",
    "nunchaku-tech/nunchaku-z-image-turbo",
];
const Z_IMAGE_COMPONENTS_REPO: &str = "Tongyi-MAI/Z-Image-Turbo";
const FLUX_REPOSITORY: &str = "black-forest-labs/FLUX.2-klein-9b-kv-fp8";
const FLUX_CHECKPOINT: &str = "flux-2-klein-9b-kv-fp8.safetensors";
const NUNCHAKU_CHECKPOINT: &str = "svdq-fp4_r128-z-image-turbo.safetensors";

pub(super) async fn serve_torch_model(
    state: &AppState,
    request: ServeModelRequest,
    operation: &pumas_library::serving::ServingLoadOperation,
) -> pumas_library::Result<Value> {
    let fail = |code, message| serving_error(code, message, &request);
    let record = state
        .api
        .model_library()
        .get_model(&request.model_id)
        .await?;
    let repository = record
        .as_ref()
        .and_then(|record| record.metadata.get("repo_id").and_then(Value::as_str));
    let is_flux = repository == Some(FLUX_REPOSITORY);
    if !is_flux && !repository.is_some_and(|repository| NUNCHAKU_REPOSITORIES.contains(&repository))
    {
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::InvalidFormat,
                "Select a qualified Nunchaku Z-Image or Klein 9B KV FP8 library package",
            ),
        )
        .await;
    }
    let assets = async {
        let model = state
            .api
            .resolve_model_execution_descriptor(&request.model_id)
            .await?;
        let components = resolve_components(
            state,
            if is_flux {
                "Qwen/Qwen3-8B"
            } else {
                Z_IMAGE_COMPONENTS_REPO
            },
        )
        .await?;
        let vae = if is_flux {
            Some(resolve_components(state, "Comfy-Org/flux2-dev").await?)
        } else {
            None
        };
        if model.validation_state != AssetValidationState::Valid {
            return Err(pumas_library::PumasError::InvalidParams {
                message: "Image checkpoint is not validated".into(),
            });
        }
        let entry = PathBuf::from(model.entry_path);
        let root = if tokio::fs::metadata(&entry).await?.is_dir() {
            entry
        } else {
            entry
                .parent()
                .ok_or_else(|| pumas_library::PumasError::InvalidParams {
                    message: "Image checkpoint has no directory".into(),
                })?
                .to_path_buf()
        };
        let checkpoint = root.join(if is_flux {
            FLUX_CHECKPOINT
        } else {
            NUNCHAKU_CHECKPOINT
        });
        if !tokio::fs::metadata(&checkpoint).await?.is_file() {
            return Err(pumas_library::PumasError::InvalidParams {
                message: "Qualified image checkpoint is missing".into(),
            });
        }
        Ok::<_, pumas_library::PumasError>((checkpoint, components, vae))
    }
    .await;
    let (checkpoint, components, vae) = match assets {
        Ok(assets) => assets,
        Err(error) => {
            tracing::warn!(%error, "Torch pipeline asset resolution failed");
            return non_critical_failure_response(state, fail(ModelServeErrorCode::ModelNotExecutable, if is_flux { "Acquire Qwen/Qwen3-8B and the standalone Comfy-Org/flux2-dev FLUX.2 VAE in Pumas first" } else { "Acquire the complete Tongyi-MAI/Z-Image-Turbo pipeline and Nunchaku FP4 rank-128 checkpoint in Pumas first" })).await;
        }
    };
    let resources = state.api.get_system_resources().await?;
    if is_flux {
        let ram = &resources.resources.ram;
        let available = ram.total as f64 * (1.0 - f64::from(ram.usage) / 100.0);
        if !resources.success || available < (42_u64 * 1024 * 1024 * 1024) as f64 {
            return non_critical_failure_response(state, fail(ModelServeErrorCode::InsufficientMemory,
                "Klein's scaled FP8 to BF16 CPU-offload policy requires 42 GiB available system RAM in Pumas telemetry")).await;
        }
    }
    let gpu = resources.resources.gpu;
    if !resources.success || gpu.memory_total.saturating_sub(gpu.memory) < 4 * 1024 * 1024 * 1024 {
        return non_critical_failure_response(state, fail(ModelServeErrorCode::InsufficientMemory, "Pumas telemetry reports less than 4 GiB free GPU memory for the offloaded image pipeline")).await;
    }
    let Some(manager) = super::get_version_manager(state, "torch").await else {
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::MissingRuntime,
                "Torch runtime manager is unavailable",
            ),
        )
        .await;
    };
    let Some(tag) = manager.get_active_version().await? else {
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::MissingRuntime,
                "Install and activate a qualified Torch runtime first",
            ),
        )
        .await;
    };
    let existing = state
        .api
        .observe_owned_runtime_profile(&request.config.profile_id)?;
    let owned = match existing
        .filter(|owned| owned.state == pumas_library::models::RuntimeLifecycleState::Running)
    {
        Some(owned) => owned,
        None => {
            let receipt = state
                .api
                .launch_runtime_profile_for_model_with_receipt(
                    request.config.profile_id.clone(),
                    &tag,
                    &manager.version_path(&tag),
                    None,
                    None,
                )
                .await;
            match receipt {
                Ok(receipt) if receipt.response.success && receipt.observation.is_some() => {
                    receipt.observation.unwrap()
                }
                result => {
                    tracing::warn!(?result, "Managed Torch profile launch failed");
                    return non_critical_failure_response(
                        state,
                        fail(
                            ModelServeErrorCode::ProviderLoadFailed,
                            "Managed Torch profile failed to start; inspect its runtime log",
                        ),
                    )
                    .await;
                }
            }
        }
    };
    let client = TorchClient::new(Some(owned.endpoint_url.as_str()));
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        if state
            .api
            .observe_owned_runtime_profile(&request.config.profile_id)?
            .as_ref()
            != Some(&owned)
        {
            return non_critical_failure_response(
                state,
                fail(
                    ModelServeErrorCode::EndpointUnavailable,
                    "Torch process ownership changed while waiting for startup",
                ),
            )
            .await;
        }
        if state
            .api
            .owned_runtime_profile_has_listener(&request.config.profile_id, &owned)?
            && client.health_check().await.unwrap_or(false)
        {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return non_critical_failure_response(
                state,
                fail(
                    ModelServeErrorCode::EndpointUnavailable,
                    "Managed Torch runtime did not become healthy within 60 seconds",
                ),
            )
            .await;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // The handshake (protocol plus capability) must pass before any model
    // load or publication through this runtime.
    if client.verify_image_runtime().await.is_err() {
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::ProviderLoadFailed,
                "Torch runtime is incompatible; install and activate a qualified Torch runtime",
            ),
        )
        .await;
    }
    let slots = client.list_slots().await?;
    if slots.iter().any(|slot| slot.model_name == request.model_id) {
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::ProviderLoadFailed,
                "This image model already occupies a Torch slot; unload it before loading again",
            ),
        )
        .await;
    }
    // The managed profile owns device visibility; CUDA device zero is its first
    // visible device, even if the host's physical device ID differs.
    let slot = match client
        .load_image_model(
            &checkpoint.to_string_lossy(),
            &request.model_id,
            &ComputeDevice::Cuda(0),
            if is_flux {
                "flux2-klein-9b-kv-fp8"
            } else {
                "nunchaku-z-image-turbo"
            },
            &ImageModelComponents {
                pipeline_path: &components,
                vae_path: vae.as_deref(),
            },
        )
        .await
    {
        Ok(slot) if slot.state == SlotState::Ready => slot,
        result => {
            tracing::warn!(?result, "Torch image load did not become ready");
            return non_critical_failure_response(
                state,
                fail(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "Torch could not load the image pipeline; inspect its runtime log",
                ),
            )
            .await;
        }
    };
    // Re-verify live compatibility and process identity before publication:
    // the sidecar may have been replaced while the model loaded, and a stale
    // admission must not publish. Replacement, incompatibility, load failure,
    // busy rejection, and unsupported operation each keep their owning
    // outcome: the model-type gate above owns unsupported-operation, the load
    // result above owns load/busy failure, and these checks own replacement
    // and post-load incompatibility.
    if state
        .api
        .observe_owned_runtime_profile(&request.config.profile_id)?
        .as_ref()
        != Some(&owned)
    {
        if let Err(cleanup) = client.unload_model(&slot.slot_id).await {
            tracing::warn!(%cleanup, "Failed to compensate Torch slot after replacement");
        }
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::EndpointUnavailable,
                "Torch process ownership changed while loading; retry the request",
            ),
        )
        .await;
    }
    if let Err(error) = client.verify_image_runtime().await {
        tracing::warn!(%error, "Torch runtime became incompatible while loading");
        if let Err(cleanup) = client.unload_model(&slot.slot_id).await {
            tracing::warn!(%cleanup, "Failed to compensate Torch slot after incompatibility");
        }
        return non_critical_failure_response(
            state,
            fail(
                ModelServeErrorCode::ProviderLoadFailed,
                "Torch runtime is incompatible; install and activate a qualified Torch runtime",
            ),
        )
        .await;
    }
    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: request.config.model_alias.clone(),
        provider: RuntimeProviderId::Torch,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: None,
        tensor_split: None,
        context_size: None,
        keep_loaded: request.config.keep_loaded,
        endpoint_url: Some(owned.endpoint_url.clone()),
        memory_bytes: slot.gpu_memory_bytes,
        loaded_at: None,
        last_error: None,
    };
    let mut snapshot = match state
        .api
        .record_served_model_for_operation_and_owned_profile(operation, status.clone(), &owned)
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Err(cleanup) = client.unload_model(&slot.slot_id).await {
                tracing::warn!(%cleanup, "Failed to compensate Torch slot publication");
            }
            return Err(error);
        }
    };
    decorate_serving_snapshot(state, &mut snapshot);
    Ok(serde_json::to_value(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: false,
        status: Some(status),
        load_error: None,
        snapshot: Some(snapshot),
    })?)
}

/// Resolve a single validated package through the existing library authority.
async fn resolve_components(state: &AppState, repository: &str) -> pumas_library::Result<String> {
    let mut candidates: Vec<_> = state
        .api
        .model_library()
        .list_models()
        .await?
        .into_iter()
        .filter(|model| model.metadata.get("repo_id").and_then(Value::as_str) == Some(repository))
        .collect();
    // Retain the BF16 source while preferring the library's converted encoder.
    // Ambiguous variants still require an explicit library correction below.
    if repository == "Qwen/Qwen3-8B"
        && candidates.iter().any(|model| {
            model
                .metadata
                .get("selected_artifact_quant")
                .and_then(Value::as_str)
                == Some("FP8")
        })
    {
        candidates.retain(|model| {
            model
                .metadata
                .get("selected_artifact_quant")
                .and_then(Value::as_str)
                == Some("FP8")
        });
    }
    let [model] = candidates.as_slice() else {
        return Err(pumas_library::PumasError::InvalidParams {
            message: format!(
                "Exactly one library-managed {repository} component package is required"
            ),
        });
    };
    let descriptor = state
        .api
        .resolve_model_execution_descriptor(&model.id)
        .await?;
    if descriptor.validation_state != AssetValidationState::Valid {
        return Err(pumas_library::PumasError::InvalidParams {
            message: format!("Components for {repository} are not validated"),
        });
    }
    // A generic execution descriptor can select a transformer shard. Diffusion
    // adapters need the indexed package directory containing all components.
    let library = state.api.model_library();
    let library_root = tokio::fs::canonicalize(library.library_root()).await?;
    let package = tokio::fs::canonicalize(library.library_root().join(&model.path)).await?;
    let entry = tokio::fs::canonicalize(&descriptor.entry_path).await?;
    if !package.starts_with(&library_root) || !entry.starts_with(&package) {
        return Err(pumas_library::PumasError::InvalidParams {
            message: "Image component package is outside its validated library root".into(),
        });
    }
    if repository == "Comfy-Org/flux2-dev" {
        // A standalone component download can still have a directory descriptor.
        let vae = if tokio::fs::metadata(&entry).await?.is_file() {
            entry
        } else {
            tokio::fs::canonicalize(package.join("split_files/vae/flux2-vae.safetensors")).await?
        };
        if !vae.starts_with(&package) || !tokio::fs::metadata(&vae).await?.is_file() {
            return Err(pumas_library::PumasError::InvalidParams {
                message: "FLUX.2 VAE is outside its validated component package".into(),
            });
        }
        return Ok(vae.to_string_lossy().into_owned());
    }
    Ok(package.to_string_lossy().into_owned())
}

pub(super) async fn unserve_torch_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let endpoint = state
        .api
        .resolve_model_runtime_profile_endpoint_for_operation(
            RuntimeProviderId::Torch,
            &request.model_id,
            Some(profile_id.clone()),
        )
        .await?;
    let client = TorchClient::new(Some(endpoint.as_str()));
    let slots = client.list_slots().await?;
    for slot in slots
        .iter()
        .filter(|slot| slot.model_name == request.model_id)
    {
        if let Err(error) = client.unload_model(&slot.slot_id).await {
            tracing::warn!(%error, "Torch unload failed");
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("Image runtime is busy or unavailable".into()),
                unloaded: false,
                snapshot: Some(current_serving_snapshot(state).await?),
            })?);
        }
    }
    let mut snapshot = state
        .api
        .record_unserved_model(
            &request.model_id,
            Some(RuntimeProviderId::Torch),
            Some(&profile_id),
            Some(&model_alias),
        )
        .await?;
    decorate_serving_snapshot(state, &mut snapshot);
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded: true,
        snapshot: Some(snapshot),
    })?)
}

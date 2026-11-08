//! Read-only readiness of a llama.cpp session for non-text input (images,
//! audio). This is backend declaration checking, not model-byte or semantic
//! attestation.
use super::super::openai_gateway;
use super::types::Modality;
use crate::server::AppState;
use pumas_library::models::{RuntimeProviderId, RuntimeProviderMode, ServedModelStatus};

pub(in crate::handlers) async fn supported_profile(
    state: &AppState,
    served: &ServedModelStatus,
) -> bool {
    supported_mode(state, served).await.is_some()
}

/// The enabled llama.cpp profile's mode. Router and dedicated sessions both
/// load the projector and both report its modalities per model, so either
/// serves non-text input.
async fn supported_mode(
    state: &AppState,
    served: &ServedModelStatus,
) -> Option<RuntimeProviderMode> {
    if served.provider != RuntimeProviderId::LlamaCpp {
        return None;
    }
    let snapshot = state.api.get_runtime_profiles_snapshot().await.ok()?;
    snapshot
        .snapshot
        .profiles
        .iter()
        .find(|profile| {
            profile.profile_id == served.profile_id
                && profile.provider == RuntimeProviderId::LlamaCpp
                && matches!(
                    profile.provider_mode,
                    RuntimeProviderMode::LlamaCppDedicated | RuntimeProviderMode::LlamaCppRouter
                )
                && profile.enabled
        })
        .map(|profile| profile.provider_mode)
}

/// The selected llama.cpp session is current and reports every non-text
/// input in `media` ready now.
pub(in crate::handlers) async fn media_ready(
    state: &AppState,
    served: &ServedModelStatus,
    media: &[Modality],
) -> bool {
    let current = || async {
        let mode = supported_mode(state, served).await?;
        openai_gateway::generation_session_stop(state, served)
            .await
            .ok()
            .map(|_| mode)
    };
    let Some(mode) = current().await else {
        return false;
    };
    let observation = super::runtime_modalities::observe(
        state,
        served,
        super::runtime_modalities::Hints {
            // The request carries non-text input, so the session is asked.
            media_task: !media.is_empty(),
            ..Default::default()
        },
    )
    .await;
    // Profile changes and session replacement are checked again after the
    // probe, which can await a slow response; a mode change makes the
    // observation stale.
    media.iter().all(|modality| observation.live(*modality)) && current().await == Some(mode)
}

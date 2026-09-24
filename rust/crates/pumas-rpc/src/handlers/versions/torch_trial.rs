//! Explicit basic Torch startup trial for one selected managed profile.

use crate::contract::TrialTorchRuntimeOutcome;
use crate::handlers::require_version_manager;
use crate::server::AppState;
use pumas_app_manager::torch_client::{TorchClient, SUPPORTED_TORCH_PROTOCOL};
use pumas_library::models::{
    RuntimeLifecycleState, RuntimeManagementMode, RuntimeProfileId, RuntimeProviderId,
    RuntimeProviderMode,
};
use std::time::Duration;

fn failed(
    tag: &str,
    profile_id: &RuntimeProfileId,
    message: impl Into<String>,
) -> TrialTorchRuntimeOutcome {
    TrialTorchRuntimeOutcome {
        success: false,
        tag: tag.into(),
        profile_id: profile_id.as_str().into(),
        startup_status: "failed".into(),
        health_status: "not_checked".into(),
        protocol: None,
        capabilities: Vec::new(),
        generation: None,
        started_by_trial: false,
        error: Some(message.into()),
        cleanup: "not_needed".into(),
    }
}

fn blocks_new_trial(state: RuntimeLifecycleState) -> bool {
    !matches!(
        state,
        RuntimeLifecycleState::Stopped | RuntimeLifecycleState::Failed
    )
}

pub async fn trial_torch_runtime(
    state: &AppState,
    tag: &str,
    profile_id: RuntimeProfileId,
) -> pumas_library::Result<TrialTorchRuntimeOutcome> {
    let manager = require_version_manager(state, "torch").await?;
    // Serializes the selected tag and removal with this startup admission.
    let torch_lifecycle_lease = manager.torch_lifecycle_lease().await?;
    if manager.get_active_version().await?.as_deref() != Some(tag) {
        return Ok(failed(
            tag,
            &profile_id,
            "Select this installed Torch runtime before trial",
        ));
    }
    if let Err(error) = manager.verify_torch_identity(tag).await {
        tracing::warn!(%error, %tag, "Torch runtime identity check failed during trial");
        return Ok(failed(
            tag,
            &profile_id,
            "Torch runtime identity check failed",
        ));
    }
    let profiles = state.api.get_runtime_profiles_snapshot().await?;
    let Some(profile) = profiles
        .snapshot
        .profiles
        .iter()
        .find(|profile| profile.profile_id == profile_id)
    else {
        return Ok(failed(
            tag,
            &profile_id,
            "Torch runtime profile was not found",
        ));
    };
    if profile.provider != RuntimeProviderId::Torch
        || profile.provider_mode != RuntimeProviderMode::TorchServe
        || profile.management_mode != RuntimeManagementMode::Managed
        || !profile.enabled
    {
        return Ok(failed(
            tag,
            &profile_id,
            "Trial requires an enabled managed Torch profile",
        ));
    }
    if state
        .api
        .observe_owned_runtime_profile(&profile_id)?
        .is_some_and(|owned| blocks_new_trial(owned.state))
    {
        return Ok(failed(
            tag,
            &profile_id,
            "Stop the existing owned profile before starting a trial",
        ));
    }
    let receipt = match state
        .api
        .launch_runtime_profile_for_model_with_receipt(
            profile_id.clone(),
            tag,
            &manager.version_path(tag),
            None,
            None,
        )
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            tracing::warn!(%error, %tag, "Managed Torch launch failed during trial");
            return Ok(failed(tag, &profile_id, "Managed Torch launch failed"));
        }
    };
    // Binary launch failures currently have no observation, but keep the failure
    // path safe if a future launch strategy reports a failed owned generation.
    if !receipt.response.success {
        tracing::warn!(error = ?receipt.response.error, %tag, "Managed Torch launch returned a failed receipt during trial");
        let mut outcome = failed(tag, &profile_id, "Managed Torch launch failed");
        if let Some(owned) = receipt.observation {
            let mut cleanup_ticket = state
                .api
                .owned_runtime_profile_cleanup_ticket(profile_id.clone(), owned.generation);
            outcome.generation = Some(owned.generation.to_string());
            outcome.started_by_trial = true;
            outcome.cleanup = match state
                .api
                .stop_runtime_profile_if_generation(profile_id, owned.generation)
                .await
            {
                Ok(true) => {
                    cleanup_ticket.disarm();
                    "stopped_owned_generation"
                }
                Ok(false) => {
                    cleanup_ticket.disarm();
                    "not_needed"
                }
                Err(error) => {
                    tracing::warn!(%error, "Could not clean failed owned Torch launch");
                    "manual_stop_required"
                }
            }
            .into();
        }
        return Ok(outcome);
    }
    let Some(owned) = receipt.observation else {
        return Ok(failed(
            tag,
            &profile_id,
            "Managed Torch launch returned no owned process identity",
        ));
    };
    drop(torch_lifecycle_lease);
    let mut cleanup_ticket = state
        .api
        .owned_runtime_profile_cleanup_ticket(profile_id.clone(), owned.generation);
    let mut outcome = TrialTorchRuntimeOutcome {
        success: false,
        tag: tag.into(),
        profile_id: profile_id.as_str().into(),
        startup_status: "passed".into(),
        health_status: "not_checked".into(),
        protocol: None,
        capabilities: Vec::new(),
        generation: Some(owned.generation.to_string()),
        started_by_trial: true,
        error: None,
        cleanup: "not_needed".into(),
    };
    let client = TorchClient::new(Some(owned.endpoint_url.as_str()));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let trial = async {
        loop {
            if state
                .api
                .observe_owned_runtime_profile(&profile_id)
                .map_err(|error| {
                    tracing::warn!(%error, "Could not observe owned Torch profile during trial startup");
                    "Could not verify Torch process ownership during startup".to_string()
                })?
                .as_ref()
                != Some(&owned)
            {
                return Err("Torch process ownership changed during startup".to_string());
            }
            if state
                .api
                .owned_runtime_profile_has_listener(&profile_id, &owned)
                .map_err(|error| {
                    tracing::warn!(%error, "Could not check Torch profile listener during trial startup");
                    "Could not verify Torch profile listener during startup".to_string()
                })?
            {
                match tokio::time::timeout_at(deadline, client.health_check()).await {
                    Ok(Ok(true)) => break,
                    Ok(_) => {}
                    Err(_) => {
                        return Err(
                            "Managed Torch profile did not become healthy within 60 seconds"
                                .to_string(),
                        );
                    }
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(
                    "Managed Torch profile did not become healthy within 60 seconds".to_string(),
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        outcome.health_status = "passed".into();
        let handshake = client
            .handshake()
            .await
            .map_err(|error| {
                tracing::warn!(%error, "Torch trial handshake failed");
                "Torch sidecar handshake failed".to_string()
            })?;
        outcome.protocol = Some(handshake.protocol);
        outcome.capabilities = handshake.capabilities;
        if handshake.status != "ok" {
            tracing::warn!(status = %handshake.status, "Torch trial handshake reported an unhealthy status");
            return Err("Torch sidecar reported an unhealthy status".to_string());
        }
        if handshake.protocol != SUPPORTED_TORCH_PROTOCOL {
            tracing::warn!(protocol = handshake.protocol, expected = SUPPORTED_TORCH_PROTOCOL, "Torch trial handshake reported an incompatible protocol");
            return Err("Torch sidecar protocol is incompatible".to_string());
        }
        if state
            .api
            .observe_owned_runtime_profile(&profile_id)
            .map_err(|error| {
                tracing::warn!(%error, "Could not observe owned Torch profile after trial health check");
                "Could not verify Torch process ownership after health check".to_string()
            })?
            .as_ref()
            != Some(&owned)
        {
            return Err("Torch process ownership changed after health check".to_string());
        }
        Ok::<(), String>(())
    }
    .await;
    match trial {
        Ok(()) => {
            outcome.success = true;
            cleanup_ticket.disarm();
        }
        Err(error) => {
            outcome.error = Some(error);
            if outcome.health_status != "passed" {
                outcome.health_status = "failed".into();
            }
            outcome.cleanup = match state
                .api
                .stop_runtime_profile_if_generation(profile_id, owned.generation)
                .await
            {
                Ok(true) => {
                    cleanup_ticket.disarm();
                    "stopped_owned_generation"
                }
                Ok(false) => {
                    cleanup_ticket.disarm();
                    "not_needed"
                }
                Err(error) => {
                    tracing::warn!(%error, "Could not clean failed owned Torch trial");
                    "manual_stop_required"
                }
            }
            .into();
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::blocks_new_trial;
    use pumas_library::models::RuntimeLifecycleState;

    #[test]
    fn terminal_observations_allow_repeat_trial_admission() {
        assert!(!blocks_new_trial(RuntimeLifecycleState::Stopped));
        assert!(!blocks_new_trial(RuntimeLifecycleState::Failed));
        for state in [
            RuntimeLifecycleState::Unknown,
            RuntimeLifecycleState::Starting,
            RuntimeLifecycleState::Running,
            RuntimeLifecycleState::Stopping,
            RuntimeLifecycleState::External,
        ] {
            assert!(blocks_new_trial(state));
        }
    }
}

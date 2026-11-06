//! Torch inference server handlers.
//!
//! These handlers reach a caller-named Torch server (`connection_url`, or the
//! default base URL when absent). [`torch_client_for_destination`] decides
//! whether that destination may receive this backend's Torch API token.

use super::{get_str_param, require_str_param};
use crate::server::AppState;
use pumas_app_manager::TorchClient;
use pumas_library::config::AppId;
use pumas_library::models::{
    RuntimeEndpointUrl, RuntimeLifecycleState, RuntimeProviderId, RuntimeProviderMode,
};
use serde_json::{json, Value};

/// Endpoint identity as [`TorchClient`] requests it (trailing `/` trimmed),
/// in normalised URL form. `None` when the text is not an endpoint URL.
fn normalised_endpoint(url: &str) -> Option<RuntimeEndpointUrl> {
    RuntimeEndpointUrl::parse(url.trim().trim_end_matches('/')).ok()
}

/// The current endpoint of the running, owned Torch sidecar at `destination`.
///
/// Torch runtime-profile launches are the only sidecars started with this
/// backend's API token (the legacy `launch_torch` path starts a tokenless
/// sidecar). Ownership therefore needs an owned-process observation of a Torch
/// profile that is running at the same normalised endpoint and whose admitted
/// child owns that endpoint's listener. A configured profile endpoint alone is
/// not ownership: another process may be listening there.
async fn owned_torch_sidecar_endpoint(
    state: &AppState,
    destination: &str,
) -> pumas_library::Result<Option<RuntimeEndpointUrl>> {
    let Some(destination) = normalised_endpoint(destination) else {
        return Ok(None);
    };
    let profiles = state
        .api
        .get_runtime_profiles_snapshot()
        .await?
        .snapshot
        .profiles;
    for profile in profiles.iter().filter(|profile| {
        profile.provider == RuntimeProviderId::Torch
            && profile.provider_mode == RuntimeProviderMode::TorchServe
    }) {
        let Some(owned) = state
            .api
            .observe_owned_runtime_profile(&profile.profile_id)?
        else {
            continue;
        };
        if owned.state == RuntimeLifecycleState::Running
            && normalised_endpoint(owned.endpoint_url.as_str()).as_ref() == Some(&destination)
            && state
                .api
                .owned_runtime_profile_has_listener(&profile.profile_id, &owned)?
        {
            return Ok(Some(owned.endpoint_url));
        }
    }
    Ok(None)
}

/// Torch client for a caller-chosen destination (`None` means the default
/// base URL). The client carries this backend's Torch API token only when the
/// destination is a running Torch sidecar this backend owns; any other server
/// gets a tokenless client and answers for itself.
pub(super) async fn torch_client_for_destination(
    state: &AppState,
    destination: Option<&str>,
) -> pumas_library::Result<TorchClient> {
    let destination = destination.unwrap_or(AppId::Torch.default_base_url());
    Ok(
        match owned_torch_sidecar_endpoint(state, destination).await? {
            Some(owned) => TorchClient::for_owned_sidecar(owned.as_str()),
            None => TorchClient::new(Some(destination)),
        },
    )
}

pub async fn torch_list_slots(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let client = torch_client_for_destination(state, connection_url).await?;
    let slots = client.list_slots().await?;
    Ok(json!({
        "success": true,
        "slots": slots
    }))
}

pub async fn torch_load_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let model_id = require_str_param(params, "model_id", "modelId")?;
    let device = get_str_param(params, "device", "device").unwrap_or("auto");
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let descriptor = match state
        .api
        .resolve_model_execution_descriptor(&model_id)
        .await
    {
        Ok(descriptor) => descriptor,
        Err(err) => {
            return Ok(json!({
                "success": false,
                "error": crate::contract::PublicError::from(&err).message
            }));
        }
    };

    let library = state.api.model_library();
    let model_record = library.get_model(&model_id).await?;
    let model_name = model_record
        .as_ref()
        .map(|r| r.cleaned_name.clone())
        .unwrap_or_else(|| {
            model_id
                .split('/')
                .next_back()
                .unwrap_or(&model_id)
                .to_string()
        });

    let compute_device = pumas_app_manager::ComputeDevice::from_server_string(device);
    let client = torch_client_for_destination(state, connection_url).await?;
    let slot = client
        .load_model(&descriptor.entry_path, &model_name, &compute_device, None)
        .await?;

    Ok(json!({
        "success": true,
        "slot": slot
    }))
}

pub async fn torch_unload_model(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let slot_id = require_str_param(params, "slot_id", "slotId")?;
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = torch_client_for_destination(state, connection_url).await?;
    client.unload_model(&slot_id).await?;

    Ok(json!({ "success": true }))
}

pub async fn torch_get_status(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = torch_client_for_destination(state, connection_url).await?;
    let status = client.get_status().await?;

    Ok(json!({
        "success": true,
        "status": status
    }))
}

pub async fn torch_list_devices(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");

    let client = torch_client_for_destination(state, connection_url).await?;
    let devices = client.list_devices().await?;

    Ok(json!({
        "success": true,
        "devices": devices
    }))
}

pub async fn torch_configure(state: &AppState, params: &Value) -> pumas_library::Result<Value> {
    let connection_url = get_str_param(params, "connection_url", "connectionUrl");
    let config: pumas_app_manager::TorchServerConfig =
        serde_json::from_value(params.get("config").cloned().unwrap_or_default()).map_err(|e| {
            pumas_library::PumasError::InvalidParams {
                message: format!("Invalid torch config: {}", e),
            }
        })?;

    let client = torch_client_for_destination(state, connection_url).await?;
    client.configure(&config).await?;

    Ok(json!({ "success": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumas_library::runtime_profiles::{torch_api_token, TORCH_API_TOKEN_HEADER};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn endpoint_identity_ignores_trailing_slash_and_rejects_non_urls() {
        assert_eq!(
            normalised_endpoint("http://127.0.0.1:8400/"),
            normalised_endpoint(" http://127.0.0.1:8400")
        );
        assert_ne!(
            normalised_endpoint("http://127.0.0.1:8400"),
            normalised_endpoint("http://127.0.0.1:8401")
        );
        assert_eq!(normalised_endpoint("not a url"), None);
    }

    /// A Torch profile configured at the destination is not ownership: no
    /// process this backend launched is running there.
    #[tokio::test]
    async fn caller_named_torch_server_never_receives_the_api_token() {
        let root = tempfile::tempdir().unwrap();
        let state = crate::handlers::test_support::build_test_app_state(root.path()).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let port = listener.local_addr().unwrap().port();
        let mut profile = pumas_library::models::RuntimeProfileConfig::default_ollama();
        profile.profile_id =
            pumas_library::models::RuntimeProfileId::parse("torch-stopped").unwrap();
        profile.provider = RuntimeProviderId::Torch;
        profile.provider_mode = RuntimeProviderMode::TorchServe;
        profile.endpoint_url = Some(RuntimeEndpointUrl::parse(&endpoint).unwrap());
        profile.port = Some(pumas_library::models::RuntimePort::parse(port).unwrap());
        state.api.upsert_runtime_profile(profile).await.unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let mut bytes = [0; 1024];
                let count = socket.read(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&bytes[..count]);
            }
            let body = r#"{"slots":[]}"#;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            String::from_utf8_lossy(&request).to_ascii_lowercase()
        });

        let response = torch_list_slots(&state, &json!({ "connection_url": endpoint }))
            .await
            .unwrap();

        assert_eq!(response, json!({ "success": true, "slots": [] }));
        let request = server.await.unwrap();
        assert!(request.starts_with("get /api/slots "));
        assert!(!request.contains(TORCH_API_TOKEN_HEADER));
        assert!(!request.contains(torch_api_token()));
    }
}

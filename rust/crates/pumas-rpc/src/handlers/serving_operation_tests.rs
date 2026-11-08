use super::*;
use pumas_library::models::{
    ModelServingConfig, RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderId,
    ServedModelLoadState,
};

fn request() -> ServeModelRequest {
    ServeModelRequest {
        model_id: "models/example".into(),
        config: ModelServingConfig {
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("test-profile").unwrap(),
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            model_alias: Some("example".into()),
        },
    }
}

#[tokio::test]
async fn generic_serving_composition_exposes_loading_while_provider_is_pending() {
    let root = tempfile::TempDir::new().unwrap();
    let api = crate::handlers::test_support::build_test_api(root.path()).await;
    let entered = tokio::sync::Notify::new();
    let operation = execute_serving_load(&api, request(), async |_, _| {
        entered.notify_one();
        std::future::pending::<pumas_library::Result<Value>>().await
    });
    tokio::pin!(operation);
    tokio::select! {
        result = &mut operation => panic!("provider unexpectedly completed: {result:?}"),
        () = entered.notified() => {}
    }
    let response = api.get_serving_status().await.unwrap();
    let wire = serde_json::to_value(&response).unwrap();
    assert_eq!(wire["success"], true);
    assert_eq!(wire["snapshot"]["schema_version"], 1);
    assert_eq!(
        wire["snapshot"]["served_models"][0]["load_state"],
        "loading"
    );
    assert!(wire["snapshot"]["cursor"]
        .as_str()
        .unwrap()
        .starts_with("serving:"));
    let snapshot = response.snapshot;
    assert_eq!(
        snapshot.served_models.len(),
        1,
        "pending provider load must be observable after dialog reopen"
    );
    assert_eq!(
        snapshot.served_models[0].load_state,
        ServedModelLoadState::Loading
    );
    assert_eq!(snapshot.endpoint.model_count, 0);
}

fn loaded_status(request: &ServeModelRequest) -> pumas_library::models::ServedModelStatus {
    pumas_library::models::ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: request.config.model_alias.clone(),
        provider: request.config.provider,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: None,
        gpu_layers: None,
        tensor_split: None,
        context_size: None,
        keep_loaded: true,
        endpoint_url: None,
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    }
}

#[tokio::test]
async fn generic_serving_composition_admits_only_one_mutation_and_publishes_loaded() {
    let root = tempfile::TempDir::new().unwrap();
    let api = crate::handlers::test_support::build_test_api(root.path()).await;
    let entered = tokio::sync::Notify::new();
    let release = tokio::sync::Notify::new();
    let mutations = std::cell::Cell::new(0);
    let operation = execute_serving_load(&api, request(), async |request, receipt| {
        mutations.set(mutations.get() + 1);
        entered.notify_one();
        release.notified().await;
        let status = loaded_status(&request);
        let snapshot = api.record_served_model_for_operation(receipt, status.clone())?;
        Ok(serde_json::to_value(ServeModelResponse {
            success: true,
            error: None,
            loaded: true,
            loaded_models_unchanged: false,
            status: Some(status),
            load_error: None,
            snapshot: Some(snapshot),
        })?)
    });
    tokio::pin!(operation);
    tokio::select! {
        result = &mut operation => panic!("premature provider completion: {result:?}"),
        () = entered.notified() => {}
    }
    let duplicate = execute_serving_load(&api, request(), async |_, _| {
        mutations.set(mutations.get() + 1);
        panic!("duplicate must never invoke provider")
    })
    .await
    .unwrap();
    assert_eq!(duplicate["loaded"], false);
    assert_eq!(
        duplicate["snapshot"]["served_models"][0]["load_state"],
        "loading"
    );
    release.notify_one();
    let response = operation.await.unwrap();
    assert_eq!(mutations.get(), 1);
    assert_eq!(
        response["snapshot"]["served_models"][0]["load_state"],
        "loaded"
    );
    assert_eq!(response["snapshot"]["endpoint"]["model_count"], 1);
}

#[tokio::test]
async fn generic_serving_composition_cancellation_and_infrastructure_error_are_unknown() {
    for cancelled in [true, false] {
        let root = tempfile::TempDir::new().unwrap();
        let api = crate::handlers::test_support::build_test_api(root.path()).await;
        let entered = tokio::sync::Notify::new();
        {
            let operation = execute_serving_load(&api, request(), async |_, _| {
                entered.notify_one();
                if cancelled {
                    std::future::pending::<()>().await;
                }
                Err(pumas_library::PumasError::Config {
                    message: "fixture infrastructure failure".into(),
                })
            });
            tokio::pin!(operation);
            if cancelled {
                tokio::select! {
                    result = &mut operation => panic!("premature completion: {result:?}"),
                    () = entered.notified() => {}
                }
            } else {
                assert!(operation.await.is_err());
            }
        }
        let response = api.get_serving_status().await.unwrap();
        let wire = serde_json::to_value(&response).unwrap();
        assert_eq!(wire["success"], true);
        assert_eq!(wire["snapshot"]["schema_version"], 1);
        assert_eq!(wire["snapshot"]["served_models"][0]["load_state"], "failed");
        assert_eq!(
            wire["snapshot"]["served_models"][0]["last_error"]["code"],
            "unknown"
        );
        let snapshot = response.snapshot;
        assert_eq!(
            snapshot.served_models[0].load_state,
            ServedModelLoadState::Failed
        );
        assert_eq!(
            snapshot.served_models[0].last_error.as_ref().unwrap().code,
            ModelServeErrorCode::Unknown
        );
        assert_eq!(snapshot.endpoint.model_count, 0);
    }
}

#[tokio::test]
async fn generic_serving_composition_returns_terminal_snapshot_after_known_failure() {
    let root = tempfile::TempDir::new().unwrap();
    let api = crate::handlers::test_support::build_test_api(root.path()).await;
    let response = execute_serving_load(&api, request(), async |request, _| {
        let mut response = ServeModelResponse::non_critical_failure(serving_error(
            ModelServeErrorCode::ProviderLoadFailed,
            "fixture failure",
            &request,
        ));
        response.snapshot = Some(api.get_serving_status().await?.snapshot);
        Ok(serde_json::to_value(response)?)
    })
    .await
    .unwrap();
    assert_eq!(
        response["snapshot"]["served_models"][0]["load_state"],
        "failed"
    );
    assert_eq!(
        response["snapshot"]["served_models"][0]["last_error"]["code"],
        "provider_load_failed"
    );
    assert!(api.begin_serving_load(&request()).is_ok());
}

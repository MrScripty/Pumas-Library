#![warn(unsafe_code)]
#![allow(clippy::await_holding_lock)]

use pumas_library::intent::{
    AcquisitionPolicy, ArtifactRequirement, EnsureModelOutcome, EnsureModelRequest,
    GetEnsureStatusOutcome, ListModelDeclarationsOutcome, ModelDeclaration, ModelRequirement,
    ModelSelector, ObservedModelState, ReleaseModelOutcome,
};
use pumas_library::models::PumasModelRef;
use pumas_library::PumasApi;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

static REGISTRY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct RegistryGuard(std::sync::MutexGuard<'static, ()>);
impl RegistryGuard {
    #[allow(unsafe_code)]
    fn new(root: &Path) -> Self {
        let guard = REGISTRY_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // SAFETY: This test process serializes registry environment access.
        unsafe {
            std::env::set_var("PUMAS_REGISTRY_DB_PATH", root.join("registry.db"));
        }
        Self(guard)
    }
}
impl Drop for RegistryGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let _ = &self.0;
        // SAFETY: The guard still holds this process's registry lock.
        unsafe {
            std::env::remove_var("PUMAS_REGISTRY_DB_PATH");
        }
    }
}

async fn open(root: &Path) -> PumasApi {
    PumasApi::builder(root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await
        .unwrap()
}

fn request(consumer: &str, model_id: &str) -> EnsureModelRequest {
    EnsureModelRequest {
        consumer_key: consumer.to_owned(),
        requirement: ModelRequirement {
            selector: ModelSelector::LocalModel {
                model_ref: PumasModelRef {
                    model_id: model_id.to_owned(),
                    ..Default::default()
                },
            },
            artifact: ArtifactRequirement::default(),
            acquisition_policy: AcquisitionPolicy::LocalOnly,
        },
    }
}

async fn ensure(api: &PumasApi, request: &EnsureModelRequest) -> ModelDeclaration {
    match api.intent().ensure_model(request).await.unwrap() {
        EnsureModelOutcome::Accepted { declaration, .. } => declaration,
        other => panic!("expected durable acceptance: {other:?}"),
    }
}

#[tokio::test]
async fn duplicate_ensure_and_restart_preserve_identity_without_claiming_availability() {
    let root = TempDir::new().unwrap();
    let _registry = RegistryGuard::new(root.path());
    let api = open(root.path()).await;
    let req = request("consumer-a", "llm/intent/missing");
    let first = ensure(&api, &req).await;
    let duplicate = ensure(&api, &req).await;
    assert_eq!(first.reference, duplicate.reference);
    let status = api
        .intent()
        .get_ensure_status(&first.reference)
        .await
        .unwrap();
    assert!(
        matches!(
            status,
            GetEnsureStatusOutcome::Found {
                state: ObservedModelState::Missing { .. },
                ..
            }
        ),
        "{status:?}"
    );
    api.shutdown_intent().await.unwrap();
    drop(api);
    let reopened = open(root.path()).await;
    let status = reopened
        .intent()
        .get_ensure_status(&first.reference)
        .await
        .unwrap();
    let GetEnsureStatusOutcome::Found { declaration, state } = status else {
        panic!("lost durable declaration")
    };
    assert_eq!(declaration.reference, first.reference);
    assert!(!matches!(state, ObservedModelState::Available { .. }));
    reopened.shutdown_intent().await.unwrap();
}

#[tokio::test]
async fn two_consumers_retain_files_until_both_release_and_administrator_deletes() {
    let root = TempDir::new().unwrap();
    let _registry = RegistryGuard::new(root.path());
    let api = open(root.path()).await;
    let model_id = "llm/intent/retained";
    let dir = root.path().join("shared-resources/models").join(model_id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("weights.gguf"), b"retained artifact").unwrap();
    let first = ensure(&api, &request("consumer-a", model_id)).await;
    let second = ensure(&api, &request("consumer-b", model_id)).await;
    assert_ne!(
        first.reference.declaration_id,
        second.reference.declaration_id
    );
    assert!(api.delete_model_with_cascade(model_id).await.is_err());
    assert!(dir.join("weights.gguf").is_file());
    assert!(matches!(
        api.intent().release_model(&first.reference).await.unwrap(),
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(api.delete_model_with_cascade(model_id).await.is_err());
    assert!(matches!(
        api.intent().release_model(&second.reference).await.unwrap(),
        ReleaseModelOutcome::Released { .. }
    ));
    assert!(
        dir.join("weights.gguf").is_file(),
        "release must never delete bytes"
    );
    api.delete_model_with_cascade(model_id).await.unwrap();
    assert!(!dir.exists());
    api.shutdown_intent().await.unwrap();
}

#[tokio::test]
async fn old_generation_and_wrong_consumer_cannot_release_a_new_declaration() {
    let root = TempDir::new().unwrap();
    let _registry = RegistryGuard::new(root.path());
    let api = open(root.path()).await;
    let req = request("consumer-a", "llm/intent/generation");
    let first = ensure(&api, &req).await;
    let mut wrong = first.reference.clone();
    wrong.consumer_key = "consumer-b".to_owned();
    assert!(matches!(
        api.intent().release_model(&wrong).await.unwrap(),
        ReleaseModelOutcome::Conflict { .. }
    ));
    api.intent().release_model(&first.reference).await.unwrap();
    assert!(matches!(
        api.intent().release_model(&first.reference).await.unwrap(),
        ReleaseModelOutcome::AlreadyAbsent { .. }
    ));
    let replacement = ensure(&api, &req).await;
    assert_ne!(first.reference.generation, replacement.reference.generation);
    assert!(matches!(
        api.intent().release_model(&first.reference).await.unwrap(),
        ReleaseModelOutcome::Conflict { .. }
    ));
    assert!(matches!(
        api.intent()
            .get_ensure_status(&replacement.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found { .. }
    ));
    api.shutdown_intent().await.unwrap();
}

#[tokio::test]
async fn unavailable_upstream_is_durable_but_invalid_request_is_not() {
    let root = TempDir::new().unwrap();
    let _registry = RegistryGuard::new(root.path());
    let api = open(root.path()).await;
    let invalid = request("invalid consumer", "llm/intent/invalid");
    assert!(matches!(
        api.intent().ensure_model(&invalid).await.unwrap(),
        EnsureModelOutcome::InvalidRequirement { .. }
    ));
    let mut req = request("consumer-a", "llm/intent/upstream");
    req.requirement.selector = ModelSelector::UpstreamRepository {
        repository_id: "example/weights".into(),
        revision: None,
    };
    req.requirement.acquisition_policy = AcquisitionPolicy::AllowUpstream;
    let declaration = ensure(&api, &req).await;
    assert!(declaration.resolved_requirement.is_none());
    let status = api
        .intent()
        .get_ensure_status(&declaration.reference)
        .await
        .unwrap();
    assert!(
        matches!(
            status,
            GetEnsureStatusOutcome::Found {
                state: ObservedModelState::Unavailable { .. },
                ..
            }
        ),
        "{status:?}"
    );
    let ListModelDeclarationsOutcome::Declarations { declarations } =
        api.intent().list_declarations().await.unwrap()
    else {
        panic!("unexpected declaration list outcome")
    };
    assert_eq!(declarations.len(), 1);
    api.shutdown_intent().await.unwrap();
}

#[tokio::test]
async fn shutdown_closes_mutations_and_remains_repeatable() {
    let root = TempDir::new().unwrap();
    let _registry = RegistryGuard::new(root.path());
    let api = open(root.path()).await;
    let declaration = ensure(&api, &request("consumer-a", "llm/intent/shutdown")).await;
    api.shutdown_intent().await.unwrap();
    api.shutdown_intent().await.unwrap();
    assert!(api
        .intent()
        .ensure_model(&request("consumer-b", "llm/intent/closed"))
        .await
        .is_err());
    assert!(api
        .intent()
        .release_model(&declaration.reference)
        .await
        .is_err());
    assert!(matches!(
        api.intent()
            .get_ensure_status(&declaration.reference)
            .await
            .unwrap(),
        GetEnsureStatusOutcome::Found { .. }
    ));
}

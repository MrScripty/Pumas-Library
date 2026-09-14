use super::resolver::IntentResolver;
use super::types::*;
use crate::api::{RuntimeTaskContext, RuntimeTasks};
use crate::index::{BoundTarget, IntentDeclarationRecord, IntentDeclarationRelease};
use crate::model_library::{HuggingFaceClient, ModelLibrary};
use crate::{PumasApi, Result};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Owned implementation of model intent for one composed library instance.
///
/// Clones share the same declaration/admission gate. Network preparation does
/// not hold that gate; the generation reread, immutable binding and admission do.
pub(crate) struct IntentService {
    library: Arc<ModelLibrary>,
    hf_client: Option<Arc<HuggingFaceClient>>,
    runtime_tasks: RuntimeTasks,
    resolver: IntentResolver,
    declaration_gate: Mutex<()>,
    reconciliation_wake: OnceLock<Arc<dyn Fn() + Send + Sync>>,
}

impl IntentService {
    pub(crate) fn new(
        library: Arc<ModelLibrary>,
        hf_client: Option<Arc<HuggingFaceClient>>,
        runtime_tasks: RuntimeTasks,
    ) -> Self {
        Self {
            resolver: IntentResolver::new(library.clone()),
            library,
            hf_client,
            runtime_tasks,
            declaration_gate: Mutex::new(()),
            reconciliation_wake: OnceLock::new(),
        }
    }

    pub(crate) fn install_reconciliation_wake(
        &self,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<()> {
        self.reconciliation_wake.set(wake).map_err(|_| {
            crate::PumasError::Other("intent reconciliation wake is already installed".to_string())
        })
    }

    async fn get_model(&self, requirement: &ModelRequirement) -> Result<GetModelOutcome> {
        let observed = self.resolver.resolve_local(requirement).await?;
        if terminal_local_observation(&observed) {
            return Ok(observed);
        }
        if let Some(client) = self.hf_client.as_ref() {
            if let Some(acquisition) =
                super::acquisition::observe_acquisition(client, requirement).await?
            {
                return Ok(acquisition);
            }
        }
        if requirement.acquisition_policy != AcquisitionPolicy::AllowUpstream {
            return Ok(observed);
        }
        let Some(client) = self.hf_client.clone() else {
            return Ok(upstream_unavailable());
        };
        if !matches!(
            observed,
            ObservedModelState::Missing { .. }
                | ObservedModelState::Unsatisfied { .. }
                | ObservedModelState::Incomplete { .. }
        ) {
            return Ok(observed);
        }
        if let Some(acquisition) = super::acquisition::acquire_upstream(
            self.library.clone(),
            client,
            &self.resolver,
            requirement,
            matches!(
                observed,
                ObservedModelState::Missing { .. } | ObservedModelState::Unsatisfied { .. }
            ),
        )
        .await?
        {
            return Ok(acquisition);
        }
        self.resolver.resolve_local(requirement).await
    }

    async fn ensure_model(
        self: &Arc<Self>,
        request: &EnsureModelRequest,
    ) -> Result<EnsureModelOutcome> {
        if let Some(outcome) = validate_ensure_request(request) {
            return Ok(outcome);
        }
        let service = self.clone();
        let request = request.clone();
        self.runtime_tasks
            .run_owned("ensure model intent", move |context| async move {
                service.ensure_owned(context, request).await
            })
            .await
    }

    async fn ensure_owned(
        &self,
        context: RuntimeTaskContext,
        request: EnsureModelRequest,
    ) -> Result<EnsureModelOutcome> {
        // This commit is the acknowledgement boundary and intentionally occurs
        // before any upstream planning, binding or download admission.
        let library = self.library.clone();
        let consumer_key = request.consumer_key.clone();
        let original = request.requirement.clone();
        let committed = context
            .run_blocking("commit intent declaration", move || {
                library
                    .index()
                    .commit_intent_declaration(&consumer_key, &original)
            })
            .await?;
        let record = match committed {
            Ok(record) => record,
            Err(error) if is_declaration_conflict(&error) => {
                return Ok(EnsureModelOutcome::Unavailable {
                    diagnostics: vec![diagnostic(
                        IntentDiagnosticCode::AcquisitionBlocked,
                        "requirement",
                        "the resolved model currently has an active deletion claim",
                    )],
                });
            }
            Err(error) => return Err(error),
        };

        let state = self
            .converge_declaration(&context, record.clone(), true)
            .await?;
        let current = self
            .library
            .index()
            .get_intent_declaration(&record.declaration_id)?;
        let Some(current) = current.filter(|current| current.generation == record.generation)
        else {
            return Ok(EnsureModelOutcome::Unavailable {
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::AcquisitionBlocked,
                    "declaration",
                    "the declaration was concurrently released before ensure completed",
                )],
            });
        };
        if declaration_retryable(&current, &state) {
            if let Some(wake) = self.reconciliation_wake.get() {
                wake();
            }
        }
        Ok(EnsureModelOutcome::Accepted {
            declaration: declaration_projection(&current),
            state,
        })
    }

    async fn release_model(
        self: &Arc<Self>,
        reference: &ModelEnsureRef,
    ) -> Result<ReleaseModelOutcome> {
        let Some(generation) = validated_reference_generation(reference) else {
            return Ok(ReleaseModelOutcome::Conflict {
                reference: reference.clone(),
            });
        };
        let service = self.clone();
        let reference = reference.clone();
        self.runtime_tasks
            .run_owned("release model intent", move |context| async move {
                let _gate = service.declaration_gate.lock().await;
                let library = service.library.clone();
                let declaration_id = reference.declaration_id.clone();
                let consumer_key = reference.consumer_key.clone();
                let result = context
                    .run_blocking("release intent declaration", move || {
                        library.index().release_intent_declaration(
                            &declaration_id,
                            &consumer_key,
                            generation,
                        )
                    })
                    .await?;
                match result {
                    Ok(IntentDeclarationRelease::Released) => {
                        Ok(ReleaseModelOutcome::Released { reference })
                    }
                    Ok(IntentDeclarationRelease::AlreadyAbsent) => {
                        Ok(ReleaseModelOutcome::AlreadyAbsent { reference })
                    }
                    Err(error) if is_declaration_conflict(&error) => {
                        Ok(ReleaseModelOutcome::Conflict { reference })
                    }
                    Err(error) => Err(error),
                }
            })
            .await
    }

    async fn get_ensure_status(
        &self,
        reference: &ModelEnsureRef,
    ) -> Result<GetEnsureStatusOutcome> {
        if validated_reference_generation(reference).is_none() {
            return Ok(GetEnsureStatusOutcome::Conflict);
        }
        let Some(record) = self
            .library
            .index()
            .get_intent_declaration(&reference.declaration_id)?
        else {
            return Ok(GetEnsureStatusOutcome::NotFound);
        };
        if record.consumer_key != reference.consumer_key
            || record.generation.to_string() != reference.generation
        {
            return Ok(GetEnsureStatusOutcome::Conflict);
        }
        let state = self.observe_record(&record).await?;
        Ok(GetEnsureStatusOutcome::Found {
            declaration: declaration_projection(&record),
            state,
        })
    }

    fn list_declarations(&self) -> Result<ListModelDeclarationsOutcome> {
        let declarations = self
            .library
            .index()
            .list_intent_declarations()?
            .iter()
            .map(declaration_projection)
            .collect();
        Ok(ListModelDeclarationsOutcome::Declarations { declarations })
    }

    /// Reconcile every persisted declaration after the canonical library scope.
    /// `true` asks the existing reconciliation coordinator for bounded retry.
    pub(crate) async fn reconcile(&self, context: RuntimeTaskContext) -> Result<bool> {
        let records = self.library.index().list_intent_declarations()?;
        let mut retry = false;
        for record in records {
            let state = self
                .converge_declaration(&context, record.clone(), true)
                .await?;
            retry |= declaration_retryable(&record, &state);
        }
        Ok(retry)
    }

    pub(crate) async fn has_declarations(&self, context: RuntimeTaskContext) -> Result<bool> {
        let library = self.library.clone();
        context
            .run_blocking("inspect intent declarations", move || {
                library
                    .index()
                    .list_intent_declarations()
                    .map(|records| !records.is_empty())
            })
            .await?
    }

    async fn converge_declaration(
        &self,
        context: &RuntimeTaskContext,
        record: IntentDeclarationRecord,
        admit: bool,
    ) -> Result<ObservedModelState> {
        if let Some(target) = &record.bound_target {
            let observed = self.observe_record(&record).await?;
            if matches!(target, BoundTarget::Local { .. }) {
                return Ok(observed);
            }
            if matches!(observed, ObservedModelState::Blocked { .. }) {
                return match self.hf_client.clone() {
                    Some(client) => {
                        self.resume_interrupted_bound(context, &record, target, client, observed)
                            .await
                    }
                    None => Ok(observed),
                };
            }
            if !matches!(
                observed,
                ObservedModelState::Missing { .. }
                    | ObservedModelState::Incomplete { .. }
                    | ObservedModelState::Unsatisfied { .. }
                    | ObservedModelState::Unavailable { .. }
            ) {
                return Ok(observed);
            }
        }
        let Some(client) = self.hf_client.clone() else {
            return Ok(upstream_unavailable());
        };

        let prepared = match &record.bound_target {
            None => match super::acquisition::prepare_intent_upstream_download(
                self.library.clone(),
                client.clone(),
                &record.original_requirement,
            )
            .await?
            {
                Ok(prepared) => prepared,
                Err(observed) => return Ok(observed),
            },
            Some(BoundTarget::Upstream {
                model_ref,
                repository_id,
                commit,
                filename,
                format,
            }) => match super::acquisition::prepare_bound_intent_download(
                self.library.clone(),
                client.clone(),
                repository_id,
                commit,
                filename,
                *format,
                model_ref,
            )
            .await?
            {
                Ok(prepared) => prepared,
                Err(observed) => return Ok(observed),
            },
            Some(BoundTarget::Local { .. }) => unreachable!(),
        };
        let (Some(filename), Some(format)) = (prepared.filename(), prepared.format()) else {
            return Ok(ObservedModelState::Blocked {
                resolved_requirement: None,
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::UnsupportedUpstreamLayout,
                    "declaration.bound_target",
                    "the immutable acquisition plan did not select one supported file artifact",
                )],
            });
        };
        let target = BoundTarget::Upstream {
            model_ref: prepared.model_ref().clone(),
            repository_id: prepared.repository_id().to_string(),
            commit: prepared.revision().to_string(),
            filename: filename.to_string(),
            format,
        };

        let _gate = self.declaration_gate.lock().await;
        let library = self.library.clone();
        let declaration_id = record.declaration_id.clone();
        let consumer_key = record.consumer_key.clone();
        let generation = record.generation;
        let current = context
            .run_blocking("reread intent declaration generation", move || {
                library.index().get_intent_declaration(&declaration_id)
            })
            .await??;
        let Some(current) = current else {
            return Ok(released_observation());
        };
        if current.consumer_key != consumer_key || current.generation != generation {
            return Ok(released_observation());
        }
        let current = if current.bound_target.is_none() {
            let library = self.library.clone();
            let declaration_id = current.declaration_id.clone();
            let consumer_key = current.consumer_key.clone();
            let target = target.clone();
            match context
                .run_blocking("bind intent declaration", move || {
                    library.index().bind_intent_declaration(
                        &declaration_id,
                        &consumer_key,
                        generation,
                        &target,
                    )
                })
                .await?
            {
                Ok(record) => record,
                Err(error) if is_declaration_conflict(&error) => return Ok(released_observation()),
                Err(error) => return Err(error),
            }
        } else if current.bound_target.as_ref() == Some(&target) {
            current
        } else {
            return Ok(ObservedModelState::Blocked {
                resolved_requirement: current
                    .bound_target
                    .as_ref()
                    .map(|target| bound_requirement(target, &current.original_requirement)),
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::AcquisitionBlocked,
                    "declaration.bound_target",
                    "persisted immutable target does not match the replayed acquisition plan",
                )],
            });
        };
        let resolved = current
            .bound_target
            .as_ref()
            .map(|target| bound_requirement(target, &current.original_requirement))
            .expect("bound declaration must have a resolved requirement");
        let observed = self.observe_resolved(&resolved).await?;
        if !matches!(
            observed,
            ObservedModelState::Missing { .. }
                | ObservedModelState::Incomplete { .. }
                | ObservedModelState::Unsatisfied { .. }
        ) || !admit
        {
            return Ok(observed);
        }
        match PumasApi::start_prepared_hf_download_owned(
            self.library.clone(),
            client,
            prepared,
            Some(target.model_ref()),
        )
        .await
        {
            Ok(_) => self.observe_resolved(&resolved).await,
            Err(error) => Ok(super::acquisition::classify_admission_error(error)),
        }
    }

    async fn resume_interrupted_bound(
        &self,
        context: &RuntimeTaskContext,
        record: &IntentDeclarationRecord,
        target: &BoundTarget,
        client: Arc<HuggingFaceClient>,
        observed: ObservedModelState,
    ) -> Result<ObservedModelState> {
        let BoundTarget::Upstream {
            model_ref,
            repository_id,
            ..
        } = target
        else {
            return Ok(observed);
        };
        let _gate = self.declaration_gate.lock().await;
        let library = self.library.clone();
        let declaration_id = record.declaration_id.clone();
        let current = context
            .run_blocking("reread interrupted intent generation", move || {
                library.index().get_intent_declaration(&declaration_id)
            })
            .await??;
        let Some(current) = current else {
            return Ok(released_observation());
        };
        if current.consumer_key != record.consumer_key
            || current.generation != record.generation
            || current.bound_target.as_ref() != Some(target)
        {
            return Ok(released_observation());
        }

        let snapshot = client.intent_download_snapshot().await;
        let matching = snapshot
            .downloads
            .into_iter()
            .filter(|download| {
                download.repo_id == *repository_id
                    && download
                        .model_ref
                        .as_ref()
                        .is_some_and(|actual| exact_bound_model_ref(model_ref, actual))
            })
            .collect::<Vec<_>>();
        let [download] = matching.as_slice() else {
            return if matching.is_empty() {
                Ok(observed)
            } else {
                Ok(ObservedModelState::Unavailable {
                    diagnostics: vec![diagnostic(
                        IntentDiagnosticCode::DownloadCustodyUnavailable,
                        "acquisition",
                        "multiple owned downloads claim the durable immutable target",
                    )],
                })
            };
        };
        if download.status != crate::models::DownloadStatus::Paused {
            return Ok(observed);
        }
        if !client
            .resume_interrupted_download(&download.download_id, model_ref)
            .await?
        {
            return Ok(observed);
        }
        self.observe_record(&current).await
    }

    async fn observe_record(&self, record: &IntentDeclarationRecord) -> Result<ObservedModelState> {
        let Some(target) = record.bound_target.as_ref() else {
            return Ok(ObservedModelState::Unavailable {
                diagnostics: vec![diagnostic(
                    IntentDiagnosticCode::UpstreamResolutionFailed,
                    "declaration.bound_target",
                    "the durable declaration has not yet resolved an immutable target",
                )],
            });
        };
        self.observe_resolved(&bound_requirement(target, &record.original_requirement))
            .await
    }

    async fn observe_resolved(&self, requirement: &ModelRequirement) -> Result<ObservedModelState> {
        let local = self.resolver.resolve_local(requirement).await?;
        if terminal_local_observation(&local) {
            return Ok(local);
        }
        if let Some(client) = self.hf_client.as_ref() {
            if let Some(acquiring) =
                super::acquisition::observe_acquisition(client, requirement).await?
            {
                return Ok(acquiring);
            }
        }
        Ok(local)
    }
}

/// Borrowed public facade over the owning instance's shared intent service.
#[derive(Clone, Copy)]
pub struct IntentApi<'a> {
    service: &'a Arc<IntentService>,
}

impl<'a> IntentApi<'a> {
    pub(crate) fn new(service: &'a Arc<IntentService>) -> Self {
        Self { service }
    }

    pub async fn query_models(&self, requirement: &ModelRequirement) -> Result<QueryModelsOutcome> {
        self.service.resolver.query_models(requirement).await
    }

    pub async fn get_model(&self, requirement: &ModelRequirement) -> Result<GetModelOutcome> {
        self.service.get_model(requirement).await
    }

    pub async fn get_model_status(
        &self,
        requirement: &ModelRequirement,
    ) -> Result<ObservedModelState> {
        self.service.observe_resolved(requirement).await
    }

    pub async fn ensure_model(&self, request: &EnsureModelRequest) -> Result<EnsureModelOutcome> {
        self.service.ensure_model(request).await
    }

    pub async fn release_model(&self, reference: &ModelEnsureRef) -> Result<ReleaseModelOutcome> {
        self.service.release_model(reference).await
    }

    pub async fn get_ensure_status(
        &self,
        reference: &ModelEnsureRef,
    ) -> Result<GetEnsureStatusOutcome> {
        self.service.get_ensure_status(reference).await
    }

    pub async fn list_declarations(&self) -> Result<ListModelDeclarationsOutcome> {
        self.service.list_declarations()
    }
}

fn declaration_projection(record: &IntentDeclarationRecord) -> ModelDeclaration {
    ModelDeclaration {
        reference: ModelEnsureRef {
            consumer_key: record.consumer_key.clone(),
            declaration_id: record.declaration_id.clone(),
            generation: record.generation.to_string(),
        },
        requirement: record.original_requirement.clone(),
        resolved_requirement: record
            .bound_target
            .as_ref()
            .map(|target| bound_requirement(target, &record.original_requirement)),
    }
}

fn bound_requirement(target: &BoundTarget, original: &ModelRequirement) -> ModelRequirement {
    let model_ref = target.model_ref().clone();
    let mut artifact = original.artifact.clone();
    artifact.selected_artifact_id = model_ref.selected_artifact_id.clone();
    if let BoundTarget::Upstream { format, .. } = target {
        artifact.format = Some(*format);
    }
    ModelRequirement {
        artifact,
        selector: ModelSelector::LocalModel { model_ref },
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

fn validate_ensure_request(request: &EnsureModelRequest) -> Option<EnsureModelOutcome> {
    if !valid_consumer_key(&request.consumer_key) {
        return Some(EnsureModelOutcome::InvalidRequirement {
            diagnostics: vec![diagnostic(
                IntentDiagnosticCode::InvalidConsumerKey,
                "consumer_key",
                "consumer_key must be 1-128 ASCII letters, digits, '.', '_', ':', or '-', beginning with a letter or digit",
            )],
        });
    }
    let invalid = validate_requirement(&request.requirement);
    if !invalid.is_empty() {
        return Some(EnsureModelOutcome::InvalidRequirement {
            diagnostics: invalid,
        });
    }
    let mut unsupported = unsupported_diagnostics(&request.requirement);
    let supported_pair = matches!(
        (
            &request.requirement.selector,
            request.requirement.acquisition_policy
        ),
        (
            ModelSelector::LocalModel { .. },
            AcquisitionPolicy::LocalOnly
        ) | (
            ModelSelector::UpstreamRepository { .. },
            AcquisitionPolicy::AllowUpstream
        )
    );
    if !supported_pair {
        unsupported.push(diagnostic(
            IntentDiagnosticCode::UpstreamAcquisitionUnavailable,
            "acquisition_policy",
            "durable local declarations support local references with local_only and upstream repositories with allow_upstream",
        ));
    }
    (!unsupported.is_empty()).then_some(EnsureModelOutcome::Unsupported {
        diagnostics: unsupported,
    })
}

fn valid_consumer_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn validated_reference_generation(reference: &ModelEnsureRef) -> Option<Uuid> {
    if !valid_consumer_key(&reference.consumer_key)
        || reference.declaration_id.len() != 64
        || !reference
            .declaration_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    let generation = Uuid::parse_str(&reference.generation).ok()?;
    (generation.to_string() == reference.generation).then_some(generation)
}

fn exact_bound_model_ref(
    expected: &crate::models::PumasModelRef,
    actual: &crate::models::PumasModelRef,
) -> bool {
    expected.model_id == actual.model_id
        && expected.revision == actual.revision
        && expected.selected_artifact_id == actual.selected_artifact_id
}

fn terminal_local_observation(state: &ObservedModelState) -> bool {
    matches!(
        state,
        ObservedModelState::Available { .. }
            | ObservedModelState::Ambiguous { .. }
            | ObservedModelState::InvalidRequirement { .. }
            | ObservedModelState::Unsupported { .. }
    )
}

fn declaration_retryable(record: &IntentDeclarationRecord, state: &ObservedModelState) -> bool {
    record.original_requirement.acquisition_policy == AcquisitionPolicy::AllowUpstream
        && matches!(
            state,
            ObservedModelState::Missing { .. }
                | ObservedModelState::Incomplete { .. }
                | ObservedModelState::Unsatisfied { .. }
                | ObservedModelState::Acquiring { .. }
                | ObservedModelState::Unavailable { .. }
        )
}

fn released_observation() -> ObservedModelState {
    ObservedModelState::Blocked {
        resolved_requirement: None,
        diagnostics: vec![diagnostic(
            IntentDiagnosticCode::AcquisitionBlocked,
            "declaration",
            "the declaration was released or replaced before acquisition admission",
        )],
    }
}

fn upstream_unavailable() -> ObservedModelState {
    ObservedModelState::Unavailable {
        diagnostics: vec![diagnostic(
            IntentDiagnosticCode::UpstreamAcquisitionUnavailable,
            "acquisition_policy",
            "HuggingFace acquisition is not configured",
        )],
    }
}

fn is_declaration_conflict(error: &crate::PumasError) -> bool {
    matches!(error, crate::PumasError::Other(message) if message.starts_with("Intent declaration conflict:"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PumasModelRef;

    #[tokio::test]
    async fn malformed_public_references_do_not_touch_the_store_or_poison_shutdown() {
        let root = tempfile::tempdir().unwrap();
        let library = Arc::new(ModelLibrary::new(root.path()).await.unwrap());
        let runtime_tasks = RuntimeTasks::default();
        let service = Arc::new(IntentService::new(library, None, runtime_tasks.clone()));
        let api = IntentApi::new(&service);
        let request = EnsureModelRequest {
            consumer_key: "reference-test".to_string(),
            requirement: ModelRequirement {
                selector: ModelSelector::LocalModel {
                    model_ref: PumasModelRef {
                        model_id: "llm/intent/reference-test".to_string(),
                        ..Default::default()
                    },
                },
                artifact: ArtifactRequirement::default(),
                acquisition_policy: AcquisitionPolicy::LocalOnly,
            },
        };
        let EnsureModelOutcome::Accepted { declaration, .. } =
            api.ensure_model(&request).await.unwrap()
        else {
            panic!("valid declaration was not accepted");
        };

        let mut malformed_consumer = declaration.reference.clone();
        malformed_consumer.consumer_key = "invalid consumer".to_string();
        assert!(matches!(
            api.release_model(&malformed_consumer).await.unwrap(),
            ReleaseModelOutcome::Conflict { .. }
        ));
        assert!(matches!(
            api.get_ensure_status(&malformed_consumer).await.unwrap(),
            GetEnsureStatusOutcome::Conflict
        ));

        let mut malformed_id = declaration.reference.clone();
        malformed_id.declaration_id = "A".repeat(64);
        assert!(matches!(
            api.release_model(&malformed_id).await.unwrap(),
            ReleaseModelOutcome::Conflict { .. }
        ));
        assert!(matches!(
            api.get_ensure_status(&malformed_id).await.unwrap(),
            GetEnsureStatusOutcome::Conflict
        ));

        let ListModelDeclarationsOutcome::Declarations { declarations } =
            api.list_declarations().await.unwrap();
        assert_eq!(declarations, vec![declaration]);
        runtime_tasks.shutdown_owned().await.unwrap();
    }
}

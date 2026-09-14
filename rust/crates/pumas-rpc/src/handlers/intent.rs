//! Transport projection for the local intent API.

use crate::server::AppState;
use pumas_library::intent::{
    EnsureModelOutcome, EnsureModelRequest, GetEnsureStatusOutcome, ListModelDeclarationsOutcome,
    ModelEnsureRef, ModelRequirement, ObservedModelState, QueryModelsOutcome, ReleaseModelOutcome,
};

pub(crate) async fn query_models(
    state: &AppState,
    requirement: &ModelRequirement,
) -> pumas_library::Result<QueryModelsOutcome> {
    state.api.intent().query_models(requirement).await
}

pub(crate) async fn get_model(
    state: &AppState,
    requirement: &ModelRequirement,
) -> pumas_library::Result<ObservedModelState> {
    state.api.intent().get_model(requirement).await
}

pub(crate) async fn get_model_status(
    state: &AppState,
    requirement: &ModelRequirement,
) -> pumas_library::Result<ObservedModelState> {
    state.api.intent().get_model_status(requirement).await
}

pub(crate) async fn ensure_model(
    state: &AppState,
    request: &EnsureModelRequest,
) -> pumas_library::Result<EnsureModelOutcome> {
    state.api.intent().ensure_model(request).await
}

pub(crate) async fn release_model(
    state: &AppState,
    reference: &ModelEnsureRef,
) -> pumas_library::Result<ReleaseModelOutcome> {
    state.api.intent().release_model(reference).await
}

pub(crate) async fn get_ensure_status(
    state: &AppState,
    reference: &ModelEnsureRef,
) -> pumas_library::Result<GetEnsureStatusOutcome> {
    state.api.intent().get_ensure_status(reference).await
}

pub(crate) async fn list_declarations(
    state: &AppState,
) -> pumas_library::Result<ListModelDeclarationsOutcome> {
    state.api.intent().list_declarations().await
}

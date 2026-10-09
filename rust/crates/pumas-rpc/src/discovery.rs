//! Shared identity and generation-bound local HTTP rendezvous.
use crate::server::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Extension, Json,
};
use pumas_library::{
    build_info::{ProtocolAdvertisement, SchemaAdvertisement},
    discovery::{
        HttpServiceDescription, LoopbackHttpEndpoint, HTTP_ADVERTISEMENT_SCHEMA_VERSION,
        LOCAL_HTTP_PROTOCOL, LOCAL_HTTP_VERSION,
    },
    PumasBuildInfo,
};
use std::sync::Arc;

pub(crate) fn build_info() -> PumasBuildInfo {
    let mut info = PumasBuildInfo::library();
    info.component = "pumas-rpc".into();
    info.package_version = env!("CARGO_PKG_VERSION").into();
    for (name, enabled) in [
        ("inference-plugins", cfg!(feature = "inference-plugins")),
        ("s3", cfg!(feature = "s3")),
        ("export-contract", cfg!(feature = "export-contract")),
        ("test-support", cfg!(feature = "test-support")),
    ] {
        if enabled {
            info.compiled_features.push(format!("pumas-rpc/{name}"));
        }
    }
    info.protocols.push(ProtocolAdvertisement {
        name: LOCAL_HTTP_PROTOCOL.into(),
        versions: vec![LOCAL_HTTP_VERSION],
    });
    info.schemas.push(SchemaAdvertisement {
        name: "pumas.http-advertisement".into(),
        version: HTTP_ADVERTISEMENT_SCHEMA_VERSION,
    });
    info
}

/// Read-only CLI projection of the existing authenticated discovery owner.
/// Missing, closing or incompatible owners remain errors, never startup authority.
pub(crate) async fn describe_local_http(
    root: &std::path::Path,
) -> anyhow::Result<HttpServiceDescription> {
    let root = root.canonicalize()?;
    let discovery = pumas_library::discovery::LocalDiscovery::open()?;
    let service = discovery
        .borrow_http_service(
            &root,
            &pumas_library::discovery::CompatibilityRequirements::default(),
        )
        .await?;
    Ok(service.description().clone())
}

/// Bind identity belongs to this router, never to a later row using the same root.
#[derive(Clone)]
pub(crate) struct HttpRouteIdentity {
    generation: String,
    endpoint: LoopbackHttpEndpoint,
}
impl From<&HttpServiceDescription> for HttpRouteIdentity {
    fn from(description: &HttpServiceDescription) -> Self {
        Self {
            generation: description.service_generation.clone(),
            endpoint: description.endpoint.clone(),
        }
    }
}

pub(crate) async fn handle_description(
    State(state): State<Arc<AppState>>,
    Extension(identity): Extension<HttpRouteIdentity>,
) -> Response {
    match state.api.advertised_http_service() {
        Ok(Some(description))
            if description.service_generation == identity.generation
                && description.endpoint == identity.endpoint =>
        {
            ([(header::CACHE_CONTROL, "no-store")], Json(description)).into_response()
        }
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_build_features_are_identity_not_runtime_readiness() {
        let info = build_info();
        assert_eq!(info.component, "pumas-rpc");
        assert_eq!(info.protocol_version(LOCAL_HTTP_PROTOCOL, &[2, 1]), Some(1));
        assert_eq!(
            info.compiled_features.contains(&"pumas-rpc/s3".into()),
            cfg!(feature = "s3")
        );
        assert_eq!(
            info.compiled_features
                .contains(&"pumas-rpc/inference-plugins".into()),
            cfg!(feature = "inference-plugins")
        );
        let json = serde_json::to_value(info).unwrap();
        assert!(json.get("capabilities").is_none());
        assert!(json.get("models").is_none());
    }
}

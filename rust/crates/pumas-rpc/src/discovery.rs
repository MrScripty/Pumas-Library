//! Shared identity and generation-bound local HTTP rendezvous.
use crate::server::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use pumas_library::{
    build_info::{ProtocolAdvertisement, SchemaAdvertisement},
    discovery::{
        HttpAdmissionFence, HttpServiceDescription, LoopbackHttpEndpoint,
        HTTP_ADMISSION_FENCE_SCHEMA_VERSION, HTTP_ADVERTISEMENT_SCHEMA_VERSION,
        HTTP_INSTANCE_GENERATION_HEADER, HTTP_SERVICE_GENERATION_HEADER, LOCAL_HTTP_PROTOCOL,
        LOCAL_HTTP_VERSION,
    },
    PumasBuildInfo,
};
use std::sync::Arc;

/// CLI acknowledgement uses the existing authenticated HTTP description. No
/// claim token/physical handle can be serialized or transferred to the caller.
pub(crate) fn print_local_access(
    ownership: &str,
    description: &HttpServiceDescription,
) -> anyhow::Result<()> {
    println!(
        "PUMAS_LOCAL_ACCESS={}",
        serde_json::to_string(&serde_json::json!({
            "bootstrap_schema_version": 1,
            "ownership": ownership,
            "description": description,
        }))?
    );
    Ok(())
}

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
    info.schemas.push(SchemaAdvertisement {
        name: "pumas.http-admission-fence".into(),
        version: HTTP_ADMISSION_FENCE_SCHEMA_VERSION,
    });
    if cfg!(target_os = "linux") {
        info.schemas.push(SchemaAdvertisement {
            name: "pumas.http-owner-retention".into(),
            version: pumas_library::discovery::HTTP_OWNER_RETENTION_SCHEMA_VERSION,
        });
    }
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
    fence: HttpAdmissionFence,
    endpoint: LoopbackHttpEndpoint,
}
impl TryFrom<&HttpServiceDescription> for HttpRouteIdentity {
    type Error = pumas_library::PumasError;
    fn try_from(description: &HttpServiceDescription) -> Result<Self, Self::Error> {
        Ok(Self {
            fence: description.admission_fence()?,
            endpoint: description.endpoint.clone(),
        })
    }
}

impl HttpRouteIdentity {
    pub(crate) fn matches(&self, description: &HttpServiceDescription) -> bool {
        self.fence.matches(description) && self.endpoint == description.endpoint
    }
}

/// Fence opted-in requests before JSON decoding or domain admission. Legacy
/// requests retain their existing behavior. Compare to this bound router and
/// the retained core/registry generation, never merely to the requested URL.
pub(crate) async fn enforce_generation_fence(
    State((state, identity)): State<(Arc<AppState>, HttpRouteIdentity)>,
    request: Request,
    next: Next,
) -> Response {
    let fence = match request_fence(request.headers()) {
        Ok(None) => return next.run(request).await,
        Ok(Some(fence)) => fence,
        Err(()) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if state.shutdown_request.is_requested() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let description = match state.api.advertised_http_service() {
        Ok(Some(description)) => description,
        _ => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    if !identity.fence.matches(&description) || identity.endpoint != description.endpoint {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !fence.matches(&description) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    next.run(request).await
}

pub(crate) fn request_fence(
    headers: &axum::http::HeaderMap,
) -> Result<Option<HttpAdmissionFence>, ()> {
    let instance = headers.get_all(HTTP_INSTANCE_GENERATION_HEADER);
    let service = headers.get_all(HTTP_SERVICE_GENERATION_HEADER);
    if instance.iter().next().is_none() && service.iter().next().is_none() {
        return Ok(None);
    }
    // Repeated, partial, empty, non-ASCII or comma-combined values are malformed.
    // Current generations are bounded single visible ASCII tokens (timestamps
    // and UUIDs). No trimming, splitting or normalization can change identity.
    fn token(
        values: axum::http::header::GetAll<'_, axum::http::HeaderValue>,
    ) -> Result<String, ()> {
        let mut values = values.iter();
        let value = values.next().ok_or(())?;
        if values.next().is_some() {
            return Err(());
        }
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 256
            || bytes.iter().any(|b| !b.is_ascii_graphic() || *b == b',')
        {
            return Err(());
        }
        value.to_str().map(str::to_owned).map_err(|_| ())
    }
    Ok(Some(HttpAdmissionFence {
        instance_generation: token(instance)?,
        service_generation: token(service)?,
    }))
}

pub(crate) async fn handle_description(
    State(state): State<Arc<AppState>>,
    Extension(identity): Extension<HttpRouteIdentity>,
) -> Response {
    match state.api.advertised_http_service() {
        Ok(Some(description))
            if identity.fence.matches(&description)
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
        assert!(info.supports_schema(
            "pumas.http-admission-fence",
            HTTP_ADMISSION_FENCE_SCHEMA_VERSION
        ));
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

    #[test]
    fn partial_repeated_combined_and_invalid_fences_are_refused() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();
        assert_eq!(request_fence(&headers), Ok(None));
        headers.insert(
            HTTP_INSTANCE_GENERATION_HEADER,
            HeaderValue::from_static("owner"),
        );
        assert!(request_fence(&headers).is_err());
        headers.insert(
            HTTP_SERVICE_GENERATION_HEADER,
            HeaderValue::from_static("service"),
        );
        let expected = HttpAdmissionFence {
            instance_generation: "owner".into(),
            service_generation: "service".into(),
        };
        assert_eq!(request_fence(&headers), Ok(Some(expected)));
        for name in [
            HTTP_INSTANCE_GENERATION_HEADER,
            HTTP_SERVICE_GENERATION_HEADER,
        ] {
            let valid = headers.clone();
            headers.append(name, HeaderValue::from_static("duplicate"));
            assert!(request_fence(&headers).is_err());
            headers = valid;
            for value in ["", "x,y", "x y", " x", "x ", &"x".repeat(257)] {
                let mut invalid = headers.clone();
                invalid.insert(name, HeaderValue::from_str(value).unwrap());
                assert!(request_fence(&invalid).is_err(), "{name}: {value:?}");
            }
            let mut invalid = headers.clone();
            invalid.insert(name, HeaderValue::from_bytes(&[0xff]).unwrap());
            assert!(request_fence(&invalid).is_err());
        }
    }
}

#[cfg(all(test, not(feature = "inference-plugins")))]
#[path = "discovery_process_tests.rs"]
mod process_tests;

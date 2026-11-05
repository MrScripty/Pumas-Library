//! Shared identity and generation-bound local HTTP rendezvous.
use crate::http_rejection::{FenceHeaderError, HttpRejection};
use crate::server::AppState;
use axum::{
    extract::{Request, State},
    http::header,
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
use std::io::Write;
use std::sync::Arc;

/// CLI acknowledgement uses the existing authenticated HTTP description. No
/// claim token/physical handle can be serialized or transferred to the caller.
pub(crate) fn print_local_access(
    ownership: &str,
    description: &HttpServiceDescription,
) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "PUMAS_LOCAL_ACCESS={}",
        serde_json::to_string(&serde_json::json!({
            "bootstrap_schema_version": 1,
            "ownership": ownership,
            "description": description,
        }))?
    )?;
    stdout.flush()?;
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
    // Compile-time parser feature identity is separate from per-model live
    // readiness and real-model qualification in /v1/capabilities.
    #[cfg(feature = "inference-plugins")]
    info.schemas.push(SchemaAdvertisement {
        name: "pumas.model-operations.image-to-text".into(),
        version: 1,
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
        Err(error) => return HttpRejection::MalformedFence(error).into_response(),
    };
    match current_description(&state, &identity) {
        Ok(description) if fence.matches(&description) => next.run(request).await,
        Ok(_) => HttpRejection::GenerationMismatch.into_response(),
        Err(rejection) => rejection.into_response(),
    }
}

/// The registry advertisement this bound listener still owns, or why not.
pub(crate) fn current_description(
    state: &AppState,
    identity: &HttpRouteIdentity,
) -> Result<HttpServiceDescription, HttpRejection> {
    if state.shutdown_request.is_requested() {
        return Err(HttpRejection::ShuttingDown);
    }
    let description = match state.api.advertised_http_service() {
        Ok(Some(description)) => description,
        _ => return Err(HttpRejection::ServiceNotAdvertised),
    };
    if !identity.matches(&description) {
        return Err(HttpRejection::ListenerSuperseded);
    }
    Ok(description)
}

pub(crate) fn request_fence(
    headers: &axum::http::HeaderMap,
) -> Result<Option<HttpAdmissionFence>, FenceHeaderError> {
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
    ) -> Result<String, FenceHeaderError> {
        let mut values = values.iter();
        let value = values.next().ok_or(FenceHeaderError::Partial)?;
        if values.next().is_some() {
            return Err(FenceHeaderError::Repeated);
        }
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 256
            || bytes.iter().any(|b| !b.is_ascii_graphic() || *b == b',')
        {
            return Err(FenceHeaderError::Malformed);
        }
        value
            .to_str()
            .map(str::to_owned)
            .map_err(|_| FenceHeaderError::Malformed)
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
    // The description stays readable while closing; only identity is checked.
    match state.api.advertised_http_service() {
        Ok(Some(description)) if identity.matches(&description) => {
            ([(header::CACHE_CONTROL, "no-store")], Json(description)).into_response()
        }
        Ok(Some(_)) => HttpRejection::ListenerSuperseded.into_response(),
        _ => HttpRejection::ServiceNotAdvertised.into_response(),
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
        assert_eq!(
            info.supports_schema("pumas.model-operations.image-to-text", 1),
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
        assert_eq!(request_fence(&headers), Err(FenceHeaderError::Partial));
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
            assert_eq!(request_fence(&headers), Err(FenceHeaderError::Repeated));
            headers = valid;
            for value in ["", "x,y", "x y", " x", "x ", &"x".repeat(257)] {
                let mut invalid = headers.clone();
                invalid.insert(name, HeaderValue::from_str(value).unwrap());
                assert_eq!(
                    request_fence(&invalid),
                    Err(FenceHeaderError::Malformed),
                    "{name}: {value:?}"
                );
            }
            let mut invalid = headers.clone();
            invalid.insert(name, HeaderValue::from_bytes(&[0xff]).unwrap());
            assert_eq!(request_fence(&invalid), Err(FenceHeaderError::Malformed));
        }
    }
}

#[cfg(all(test, not(feature = "inference-plugins")))]
#[path = "discovery_process_tests.rs"]
mod process_tests;

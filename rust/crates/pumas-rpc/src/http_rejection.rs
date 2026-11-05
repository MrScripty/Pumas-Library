//! HTTP refusals issued before a route handler runs.
//!
//! Every refusal keeps its documented status and carries
//! `{"error":{"code","message"}}`: `code` is a stable snake_case identifier
//! callers may branch on, `message` explains the refusal to a person. The body
//! is never cached.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Why a generation-fence header pair was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FenceHeaderError {
    /// Only one of the two headers was sent.
    Partial,
    /// A header occurred more than once.
    Repeated,
    /// A value was empty, longer than 256 bytes, not visible ASCII, or had a comma.
    Malformed,
}

/// A pre-handler refusal with its status and reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HttpRejection {
    MalformedFence(FenceHeaderError),
    FenceRequired,
    GenerationMismatch,
    ShuttingDown,
    ServiceNotAdvertised,
    ListenerSuperseded,
    RetentionUnsupported,
    RetentionCapacity,
    RetentionUnavailable,
    HandlerLost,
    RouteNotFound,
    MethodNotAllowed,
    ConnectionCapacity { limit: usize },
}

impl HttpRejection {
    pub(crate) const fn status(self) -> StatusCode {
        match self {
            Self::MalformedFence(_) => StatusCode::BAD_REQUEST,
            Self::FenceRequired => StatusCode::PRECONDITION_REQUIRED,
            Self::GenerationMismatch => StatusCode::PRECONDITION_FAILED,
            Self::ShuttingDown
            | Self::ServiceNotAdvertised
            | Self::ListenerSuperseded
            | Self::RetentionUnavailable
            | Self::ConnectionCapacity { .. } => StatusCode::SERVICE_UNAVAILABLE,
            Self::RetentionUnsupported => StatusCode::NOT_IMPLEMENTED,
            Self::RetentionCapacity => StatusCode::TOO_MANY_REQUESTS,
            Self::HandlerLost => StatusCode::INTERNAL_SERVER_ERROR,
            Self::RouteNotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::MalformedFence(_) => "fence_malformed",
            Self::FenceRequired => "fence_required",
            Self::GenerationMismatch => "generation_mismatch",
            Self::ShuttingDown => "shutting_down",
            Self::ServiceNotAdvertised => "service_not_advertised",
            Self::ListenerSuperseded => "listener_superseded",
            Self::RetentionUnsupported => "retention_unsupported",
            Self::RetentionCapacity => "retention_capacity",
            Self::RetentionUnavailable => "retention_unavailable",
            Self::HandlerLost => "handler_lost",
            Self::RouteNotFound => "route_not_found",
            Self::MethodNotAllowed => "method_not_allowed",
            Self::ConnectionCapacity { .. } => "connection_capacity",
        }
    }

    pub(crate) fn message(self) -> String {
        match self {
            Self::MalformedFence(FenceHeaderError::Partial) => {
                "Send both Pumas-Instance-Generation and Pumas-Service-Generation, or neither."
                    .into()
            }
            Self::MalformedFence(FenceHeaderError::Repeated) => {
                "Each generation-fence header must occur exactly once.".into()
            }
            Self::MalformedFence(FenceHeaderError::Malformed) => {
                "Each generation-fence header must be one visible ASCII token of 1 to 256 bytes without commas."
                    .into()
            }
            Self::FenceRequired => {
                "This route requires the Pumas-Instance-Generation and Pumas-Service-Generation headers."
                    .into()
            }
            Self::GenerationMismatch => {
                "The requested generations do not match this listener; observe the HTTP description again before retrying."
                    .into()
            }
            Self::ShuttingDown => "The Pumas library owner is shutting down.".into(),
            Self::ServiceNotAdvertised => {
                "The library registry does not currently advertise this HTTP service.".into()
            }
            Self::ListenerSuperseded => {
                "The library registry advertises a different HTTP service than this listener."
                    .into()
            }
            Self::RetentionUnsupported => {
                "Passive owner retention is only supported on Linux.".into()
            }
            Self::RetentionCapacity => format!(
                "This listener already holds {} owner-retention bodies; close one and try again.",
                crate::owner_retention::MAX_ACTIVE_RETENTIONS
            ),
            Self::RetentionUnavailable => {
                "The library owner could not grant a retention guard.".into()
            }
            Self::HandlerLost => {
                "The request handler ended without a response; its outcome is unknown.".into()
            }
            Self::RouteNotFound => "No route exists at this path.".into(),
            Self::MethodNotAllowed => "This route does not accept this HTTP method.".into(),
            Self::ConnectionCapacity { limit } => format!(
                "This listener already has {limit} open connections; close one and try again."
            ),
        }
    }

    /// The JSON body, for transports that write a response without Axum.
    pub(crate) fn body(self) -> String {
        serde_json::json!({"error": {"code": self.code(), "message": self.message()}}).to_string()
    }
}

impl IntoResponse for HttpRejection {
    fn into_response(self) -> Response {
        (
            self.status(),
            [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
            Json(serde_json::json!({
                "error": {"code": self.code(), "message": self.message()}
            })),
        )
            .into_response()
    }
}

/// Router fallback for paths with no route.
pub(crate) async fn route_not_found() -> HttpRejection {
    HttpRejection::RouteNotFound
}

/// Router fallback for a known path with a method it does not accept.
pub(crate) async fn method_not_allowed() -> HttpRejection {
    HttpRejection::MethodNotAllowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_rejection_keeps_its_status_and_carries_a_reason() {
        let all = [
            HttpRejection::MalformedFence(FenceHeaderError::Partial),
            HttpRejection::MalformedFence(FenceHeaderError::Repeated),
            HttpRejection::MalformedFence(FenceHeaderError::Malformed),
            HttpRejection::FenceRequired,
            HttpRejection::GenerationMismatch,
            HttpRejection::ShuttingDown,
            HttpRejection::ServiceNotAdvertised,
            HttpRejection::ListenerSuperseded,
            HttpRejection::RetentionUnsupported,
            HttpRejection::RetentionCapacity,
            HttpRejection::RetentionUnavailable,
            HttpRejection::HandlerLost,
            HttpRejection::RouteNotFound,
            HttpRejection::MethodNotAllowed,
            HttpRejection::ConnectionCapacity { limit: 7 },
        ];
        for rejection in all {
            let response = rejection.into_response();
            assert_eq!(response.status(), rejection.status());
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["error"]["code"], rejection.code());
            assert!(!rejection.message().is_empty());
            assert_eq!(body["error"]["message"], rejection.message());
            assert_eq!(body.to_string(), rejection.body());
        }
        assert_eq!(
            HttpRejection::GenerationMismatch.status(),
            StatusCode::PRECONDITION_FAILED
        );
        assert!(HttpRejection::ConnectionCapacity { limit: 7 }
            .message()
            .contains('7'));
    }
}

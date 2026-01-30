//! Generic API response wrapper for consistent response structures.
//!
//! This module provides a generic `ApiResponse<T>` type that eliminates the need
//! for duplicating `success` and `error` fields across 20+ response types.

use serde::{Deserialize, Serialize};

/// Generic API response wrapper.
///
/// Provides consistent `success` and `error` fields for all API responses,
/// with the actual data flattened into the response structure.
///
/// # Example
///
/// ```ignore
/// use pumas_library::models::ApiResponse;
///
/// #[derive(Serialize)]
/// struct DiskSpaceData {
///     total: u64,
///     free: u64,
/// }
///
/// // Success response
/// let response = ApiResponse::success(DiskSpaceData { total: 100, free: 50 });
///
/// // Error response
/// let response: ApiResponse<DiskSpaceData> = ApiResponse::error("Disk not found");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response with data.
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }

    /// Create an error response without data.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
            data: None,
        }
    }

    /// Create a successful response without data (for void operations).
    pub fn ok() -> Self
    where
        T: Default,
    {
        Self {
            success: true,
            error: None,
            data: None,
        }
    }

    /// Check if the response indicates success.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Check if the response indicates an error.
    pub fn is_error(&self) -> bool {
        !self.success
    }

    /// Get a reference to the data if present.
    pub fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }

    /// Get the error message if present.
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Convert from a Result, mapping Ok to success and Err to error.
    pub fn from_result<E: std::fmt::Display>(result: Result<T, E>) -> Self {
        match result {
            Ok(data) => Self::success(data),
            Err(e) => Self::error(e.to_string()),
        }
    }
}

impl<T> Default for ApiResponse<T> {
    fn default() -> Self {
        Self {
            success: false,
            error: Some("No response".into()),
            data: None,
        }
    }
}

impl<T> From<Result<T, crate::error::PumasError>> for ApiResponse<T> {
    fn from(result: Result<T, crate::error::PumasError>) -> Self {
        Self::from_result(result)
    }
}

/// Unit response for operations that don't return data.
///
/// Use `ApiResponse<()>` for operations that only need success/error indication.
pub type UnitResponse = ApiResponse<()>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: i32,
        name: String,
    }

    #[test]
    fn test_success_response() {
        let response = ApiResponse::success(TestData {
            value: 42,
            name: "test".into(),
        });

        assert!(response.is_success());
        assert!(!response.is_error());
        assert!(response.error.is_none());
        assert!(response.data.is_some());
        assert_eq!(response.data().unwrap().value, 42);
    }

    #[test]
    fn test_error_response() {
        let response: ApiResponse<TestData> = ApiResponse::error("Something went wrong");

        assert!(!response.is_success());
        assert!(response.is_error());
        assert_eq!(response.error_message(), Some("Something went wrong"));
        assert!(response.data.is_none());
    }

    #[test]
    fn test_success_serialization() {
        let response = ApiResponse::success(TestData {
            value: 42,
            name: "test".into(),
        });

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"value\":42"));
        assert!(json.contains("\"name\":\"test\""));
        // Should not contain "data" wrapper due to flatten
        assert!(!json.contains("\"data\":{"));
    }

    #[test]
    fn test_error_serialization() {
        let response: ApiResponse<TestData> = ApiResponse::error("Failed");

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Failed\""));
        // Should not contain data fields
        assert!(!json.contains("\"value\""));
    }

    #[test]
    fn test_from_result_ok() {
        let result: Result<i32, String> = Ok(42);
        let response = ApiResponse::from_result(result);

        assert!(response.is_success());
        assert_eq!(response.data(), Some(&42));
    }

    #[test]
    fn test_from_result_err() {
        let result: Result<i32, String> = Err("error message".into());
        let response = ApiResponse::from_result(result);

        assert!(response.is_error());
        assert_eq!(response.error_message(), Some("error message"));
    }
}

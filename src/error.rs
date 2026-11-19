use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const CODE_VALIDATION_ERROR: &str = "validation_error";
pub const CODE_NOT_FOUND: &str = "not_found";
pub const CODE_INTERNAL_ERROR: &str = "internal_error";
pub const CODE_UPSTREAM_RATE_LIMITED: &str = "upstream_rate_limited";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

impl FieldError {
    pub fn new(
        field: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub reason: String,
    pub status_code: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldError>,
}

impl ErrorResponse {
    fn new(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            reason: reason.into(),
            status_code: status.as_u16(),
            fields: Vec::new(),
        }
    }

    pub fn client_error(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        debug_assert!(status.is_client_error());
        Self::new(status, code, message, reason)
    }

    pub fn server_error(
        status: StatusCode,
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        debug_assert!(status.is_server_error());
        Self::new(status, code, message, reason)
    }

    pub fn validation(fields: Vec<FieldError>) -> Self {
        Self::client_error(
            StatusCode::BAD_REQUEST,
            CODE_VALIDATION_ERROR,
            "Request validation failed",
            "One or more fields are invalid",
        )
        .with_fields(fields)
    }

    pub fn bad_request(
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::client_error(StatusCode::BAD_REQUEST, code, message, reason)
    }

    pub fn not_found(resource: impl AsRef<str>, id: impl std::fmt::Display) -> Self {
        Self::client_error(
            StatusCode::NOT_FOUND,
            CODE_NOT_FOUND,
            format!("{} not found", resource.as_ref()),
            format!("No {} exists with id {}", resource.as_ref(), id),
        )
    }

    pub fn unprocessable(
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::client_error(StatusCode::UNPROCESSABLE_ENTITY, code, message, reason)
    }

    pub fn internal(reason: impl Into<String>) -> Self {
        Self::server_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            CODE_INTERNAL_ERROR,
            "Internal server error",
            reason,
        )
    }

    pub fn bad_gateway(
        code: impl Into<String>,
        message: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::server_error(StatusCode::BAD_GATEWAY, code, message, reason)
    }

    pub fn upstream_rate_limited(reason: impl Into<String>) -> Self {
        Self::server_error(
            StatusCode::SERVICE_UNAVAILABLE,
            CODE_UPSTREAM_RATE_LIMITED,
            "Upstream rate limit exceeded",
            reason,
        )
    }

    pub fn with_fields(mut self, fields: Vec<FieldError>) -> Self {
        self.fields = fields;
        self
    }
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {} ({})",
            self.code, self.message, self.reason
        )
    }
}

impl std::error::Error for ErrorResponse {}

impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(self)
    }
}

#[derive(Debug, Default)]
pub struct FieldErrors {
    errors: Vec<FieldError>,
}

impl FieldErrors {
    pub fn add(
        &mut self,
        field: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.errors.push(FieldError::new(field, code, message));
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn into_response(self) -> ErrorResponse {
        ErrorResponse::validation(self.errors)
    }

    pub fn extend_from_response(&mut self, error: ErrorResponse) {
        for field in error.fields {
            self.add(field.field, field.code, field.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_builder_sets_status_code_and_reason() {
        let error = ErrorResponse::bad_request(
            "invalid_request",
            "Request is invalid",
            "sort field is unknown",
        );

        assert_eq!(error.status_code, 400);
        assert_eq!(error.code, "invalid_request");
        assert_eq!(error.message, "Request is invalid");
        assert_eq!(error.reason, "sort field is unknown");
        assert!(error.fields.is_empty());
    }

    #[test]
    fn server_builder_sets_status_code_and_reason() {
        let error = ErrorResponse::bad_gateway(
            "upstream_unavailable",
            "Upstream is unavailable",
            "upstream request timed out",
        );

        assert_eq!(error.status_code, 502);
        assert_eq!(error.code, "upstream_unavailable");
    }

    #[test]
    fn validation_uses_one_field_error_shape() {
        let mut fields = FieldErrors::default();
        fields.add("title", "required", "This field is required");
        fields.add("url", "invalid", "Must be an absolute http(s) URL");

        let error = fields.into_response();
        let json = serde_json::to_value(&error).expect("error must serialize");

        assert_eq!(json["status_code"], 400);
        assert_eq!(json["code"], "validation_error");
        assert_eq!(json["fields"][0]["field"], "title");
        assert_eq!(json["fields"][0]["code"], "required");
        assert_eq!(json["fields"][1]["field"], "url");
        assert_eq!(json["fields"][1]["code"], "invalid");
    }

    #[test]
    fn response_error_exposes_http_status() {
        let error = ErrorResponse::not_found("Item", 42);

        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(error.status_code, 404);
        assert_eq!(error.code, CODE_NOT_FOUND);
    }

    #[test]
    fn empty_fields_are_not_serialized() {
        let error = ErrorResponse::internal("database is unavailable");
        let json = serde_json::to_value(&error).expect("error must serialize");

        assert!(json.get("fields").is_none());
    }
}

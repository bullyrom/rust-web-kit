use sea_orm::entity::prelude::{DateTimeUtc, DateTimeWithTimeZone};
use url::Url;

use crate::error::{ErrorResponse, FieldError};

const SUPPORTED_URL_SCHEMES: [&str; 2] = ["http", "https"];

pub fn validate_http_url(value: &str) -> Result<(), ErrorResponse> {
    let url = Url::parse(value.trim()).map_err(|error| {
        ErrorResponse::validation(vec![FieldError::new(
            "url",
            "invalid",
            format!("Invalid URL: {error}"),
        )])
    })?;

    if !SUPPORTED_URL_SCHEMES.contains(&url.scheme()) {
        return Err(ErrorResponse::validation(vec![FieldError::new(
            "url",
            "invalid",
            "Only http and https URLs are supported",
        )]));
    }

    Ok(())
}

pub fn parse_optional_rfc3339(
    value: Option<&str>,
    field: &str,
) -> Result<Option<DateTimeUtc>, ErrorResponse> {
    value
        .map(|value| {
            DateTimeWithTimeZone::parse_from_rfc3339(value)
                .map(|date| date.to_utc())
                .map_err(|error| {
                    ErrorResponse::validation(vec![FieldError::new(
                        field,
                        "invalid",
                        format!("Invalid RFC3339 date: {error}"),
                    )])
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(validate_http_url("https://example.com/items").is_ok());
        assert!(validate_http_url("http://example.com").is_ok());
    }

    #[test]
    fn rejects_invalid_and_non_http_urls() {
        let error = validate_http_url("ftp://example.com/file")
            .expect_err("unsupported scheme must fail");

        assert_eq!(error.status_code, 400);
        assert_eq!(error.fields[0].field, "url");
        assert_eq!(error.fields[0].code, "invalid");

        assert!(validate_http_url("not a url").is_err());
    }

    #[test]
    fn parses_an_optional_rfc3339_date() {
        let date = parse_optional_rfc3339(Some("2026-01-01T10:00:00Z"), "created_from")
            .expect("date must parse")
            .expect("date must be present");

        assert_eq!(date.to_rfc3339(), "2026-01-01T10:00:00+00:00");
        assert_eq!(
            parse_optional_rfc3339(None, "created_from").expect("none is valid"),
            None
        );
    }

    #[test]
    fn invalid_date_uses_the_requested_field_name() {
        let error = parse_optional_rfc3339(Some("not-a-date"), "created_from")
            .expect_err("invalid date must fail");

        assert_eq!(error.status_code, 400);
        assert_eq!(error.fields[0].field, "created_from");
        assert_eq!(error.fields[0].code, "invalid");
    }
}

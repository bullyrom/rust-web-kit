use actix_web::{web, ResponseError};

use crate::error::ErrorResponse;

pub fn json_config() -> web::JsonConfig {
    web::JsonConfig::default().error_handler(|error, _request| {
        let reason = error.to_string();
        let response =
            ErrorResponse::bad_request("invalid_request_body", "Request body is invalid", reason);
        actix_web::error::InternalError::from_response(error, response.error_response()).into()
    })
}

pub fn query_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|error, _request| {
        let reason = error.to_string();
        let response =
            ErrorResponse::bad_request("invalid_query", "Query parameters are invalid", reason);
        actix_web::error::InternalError::from_response(error, response.error_response()).into()
    })
}

pub fn path_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|error, _request| {
        let reason = error.to_string();
        let response =
            ErrorResponse::bad_request("invalid_path", "Path parameters are invalid", reason);
        actix_web::error::InternalError::from_response(error, response.error_response()).into()
    })
}

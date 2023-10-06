use http::status::StatusCode;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LooperError {
    pub message: String,
    pub param: String,
}

pub struct LooperErrorResponse {
    pub error: LooperError,
    pub code: StatusCode,
}

impl LooperErrorResponse {
    pub fn new(code: StatusCode, message: String, param: String) -> Self {
        Self {
            code,
            error: LooperError { message, param },
        }
    }
}

// pub fn not_found() -> LooperError {
//     LooperError::new("not found".to_string(), StatusCode::NOT_FOUND)
// }

// pub fn internal_server_error() -> LooperError {
//     LooperError::new("internal server error".to_string(), StatusCode::INTERNAL_SERVER_ERROR)
// }

// pub fn bad_request(reason: String, param: String) -> LooperError {
//     LooperError::new(format!("bad request: {} {}", reason, param), StatusCode::BAD_REQUEST)
// }

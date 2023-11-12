use rocket::{
    http::{ContentType, Status},
    response::{Responder, Response, Result},
    serde::{Deserialize, Serialize},
    Request,
};

use crate::services::{loop_out::LoopOutServiceError, NOT_FOUND};

use std::io::Cursor;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct LooperError {
    pub message: String,
    pub param: String,
}

#[derive(Debug, Clone)]
pub struct LooperErrorResponse {
    pub error: LooperError,
    pub code: Status,
}

impl LooperErrorResponse {
    pub fn new(code: Status, message: String, param: String) -> Self {
        Self {
            code,
            error: LooperError { message, param },
        }
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for LooperErrorResponse {
    fn respond_to(self, _: &'r Request<'_>) -> Result<'o> {
        let json = serde_json::to_string(&self.error).unwrap();
        Response::build()
            .status(self.code)
            .header(ContentType::JSON)
            .sized_body(json.len(), Cursor::new(json))
            .ok()
    }
}

pub fn not_found(item: String) -> LooperErrorResponse {
    LooperErrorResponse::new(Status::NotFound, "not found".to_string(), item)
}

pub fn internal_server_error() -> LooperErrorResponse {
    LooperErrorResponse::new(
        Status::InternalServerError,
        "internal server error".to_string(),
        "".to_string(),
    )
}

pub fn bad_request(message: String, param: String) -> LooperErrorResponse {
    LooperErrorResponse::new(Status::BadRequest, message, param)
}

pub fn invalid_parameter(param: String) -> LooperErrorResponse {
    bad_request("invalid parameter".to_string(), param)
}

pub fn handle_loop_out_error(e: LoopOutServiceError) -> LooperErrorResponse {
    match e.message.as_str() {
        NOT_FOUND => {
            log::info!("loop out not found: {:?}", e);

            not_found("loop_out".to_string())
        }

        e => {
            log::error!("internal server error: {:?}", e);

            internal_server_error()
        }
    }
}

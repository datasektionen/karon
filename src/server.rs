use actix_web::{HttpResponse, ResponseError, http};

use crate::voteit::VoteItRequest;

pub mod api;
pub mod worker;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database broken: {0}")]
    Database(#[from] sqlx::Error),
    #[error("failed to parse UUID from string")]
    UuidParseError(#[from] sqlx::types::uuid::Error),
    #[error("meeting not active")]
    MeetingNotActive,
    #[error("card {0} does not exist")]
    CardNotExisting(String),
    #[error("token has expired")]
    TokenExpired,
    #[error("request data {0} failed to parse")]
    ReqestParseError(String),
    #[error("card uid already exists in SSO")]
    CardConflict,
    #[error("failed to send VoteIT request on MPSC channel")]
    MpscSendError(#[from] tokio::sync::mpsc::error::SendError<VoteItRequest>),
    #[error("failed to send VoteIT request to VoteIT: {0}")]
    VoteItRequestFail(#[from] reqwest::Error),
}

impl ResponseError for Error {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let code = match self {
            Error::CardNotExisting(_) => http::StatusCode::UNPROCESSABLE_ENTITY,
            _ => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        HttpResponse::build(code).body(self.to_string())
    }
}

//! Functions related to the actual Karon server functions, handling serving and endpoints.

use actix_web::{Either, HttpResponse, ResponseError, http};
use reqwest::StatusCode;
use uuid::Uuid;

use crate::{server::Error::VoteItRequestFail, sso::Member, voteit::VoteItRequest};

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
    #[error("failed to send VoteIT request to VoteIT: {0}")]
    VoteItRequestFail(#[from] reqwest::Error),
    #[error("VoteIT request queue full")]
    RequestQueueFull(
        #[from] tokio::sync::mpsc::error::SendError<(Either<String, Uuid>, Member, VoteItRequest)>,
    ),
    #[error("Person is not a member and has no permissions")]
    NonMember,
    #[error("This session is not connected to a meeting")]
    NotConnectedToMeeting,
}

impl ResponseError for Error {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        let code = match self {
            Error::CardNotExisting(_) => http::StatusCode::UNPROCESSABLE_ENTITY,
            Error::RequestQueueFull(_) => http::StatusCode::TOO_MANY_REQUESTS,
            Error::NonMember => http::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            Error::MeetingNotActive => http::StatusCode::GONE,
            _ => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        HttpResponse::build(code).body(self.to_string())
    }
}

use actix_web::{HttpResponse, ResponseError, http, post, web::Data};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::{
    AppState, db, sso,
    voteit::{self, VoteItRequest},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database broken: {0}")]
    Database(#[from] sqlx::Error),
    #[error("meeting not active")]
    MeetingNotActive,
    #[error("card {0} does not exist")]
    CardNotExisting(String),
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

#[post("/card")]
pub async fn card_api(
    data: Data<AppState>,
    card_uid: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    let meeting = db::get_meeting_from_token(&data.db, token.token()).await?;
    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    let kth_id = sso::get_kth_id(card_uid).await?;
    let perms = voteit::Permissions::default() | voteit::Permissions::DISCUSSER;

    let req = VoteItRequest {
        token: meeting.voteit_token,
        email: format!("{kth_id}@kth.se"),
        perms,
    };

    data.producer.send(req);

    Ok(HttpResponse::build(http::StatusCode::OK).body(kth_id))
}

// #[post("/onboard")]
// pub async fn onboard_api(
//     data: Data<AppState>,
//     card_uid: String,
//     token: BearerAuth,
// ) -> Result<HttpResponse, Error> {
// }

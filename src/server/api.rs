use actix_web::{HttpResponse, http, post, web::Data};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::{
    db::{self, Db, Meeting},
    server::Error,
    sso::{self, check_moderator},
    voteit::{self, Permissions, VoteItRequest},
};

#[post("/card")]
pub async fn card_api(
    db: Data<Db>,
    tx: Data<tokio::sync::mpsc::Sender<VoteItRequest>>,
    card_uid: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    let kth_id = sso::get_kth_id(&card_uid).await?;

    let meeting = db::get_meeting_from_token(&db, token.token()).await?;
    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    card(tx, &kth_id, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(kth_id))
}

async fn card(
    tx: Data<tokio::sync::mpsc::Sender<VoteItRequest>>,
    kth_id: &str,
    meeting: Meeting,
) -> Result<(), Error> {
    // TODO: Very temporary code for testing
    let mut perms = voteit::Permissions::default() | voteit::Permissions::DISCUSSER;

    match check_moderator(kth_id).await {
        Ok(true) => perms |= Permissions::MODERATOR,
        _ => (),
    };

    let req = VoteItRequest {
        meeting_id: meeting.meeting_id,
        email: format!("{kth_id}@kth.se"),
        perms,
    };

    tx.send(req).await?;
    Ok(())
}

#[post("/onboard")]
pub async fn onboard_api(
    db: Data<Db>,
    req_body: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    db::verify_onboard_token(&db, token.token()).await?;

    let (kth_id, card_uid) = match req_body.split_once('#') {
        Some(c) => c,
        None => return Err(Error::ReqestParseError(req_body)),
    };

    sso::onboard(card_uid, kth_id).await?;
    Ok(HttpResponse::build(http::StatusCode::OK).body(""))
}

async fn onboard(req_body: &str) -> Result<String, Error> {
    let (kth_id, card_uid) = match req_body.split_once('#') {
        Some(c) => c,
        None => return Err(Error::ReqestParseError(req_body.to_string())),
    };

    sso::onboard(card_uid, kth_id).await?;
    Ok(kth_id.to_string())
}

#[post("/card/onboard")]
pub async fn card_onboard_api(
    db: Data<Db>,
    tx: Data<tokio::sync::mpsc::Sender<VoteItRequest>>,
    req_body: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    let meeting = db::get_meeting_from_token(&db, token.token()).await?;
    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    let kth_id = onboard(&req_body).await?;
    card(tx, &kth_id, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(kth_id))
}

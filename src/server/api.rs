//! Handles all functions for Karon's API endpoints.

use actix_web::{HttpResponse, http, post, web::Data};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::{
    db::{self, Db, Meeting},
    server::Error,
    sso::{self, Member, check_moderator},
    voteit::{self, Permissions, VoteItRequest},
};

/// Handles the `/card` API, which is to be used when a user checks in or out from a meeting.
#[post("/card")]
pub async fn card_api(
    db: Data<Db>,
    tx: Data<tokio::sync::mpsc::Sender<VoteItRequest>>,
    card_uid: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    let member_info = sso::get_member_info(&card_uid).await?;

    let meeting = db::get_meeting_from_token(&db, token.token()).await?;
    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    card(tx, &member_info, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(member_info.kth_id))
}

async fn card(
    tx: Data<tokio::sync::mpsc::Sender<VoteItRequest>>,
    member_info: &Member,
    meeting: Meeting,
) -> Result<(), Error> {
    let mut perms: Permissions = member_info.member_type.into();

    if !perms.contains(voteit::Permissions::PARTICIPANT) {
        return Err(Error::NonMember);
    }

    match check_moderator(&member_info.kth_id).await {
        Ok(true) => perms |= Permissions::MODERATOR,
        _ => (),
    };

    let req = VoteItRequest {
        meeting_id: meeting.meeting_id,
        email: member_info.email.clone(),
        perms,
        voteit_token: meeting.voteit_token,
    };

    tx.send(req).await?;
    Ok(())
}

/// Handles the `/onboard` API, which is to be used when adding a new card uid to a user in the
/// membership database.
#[post("/onboard")]
pub async fn onboard_api(
    db: Data<Db>,
    req_body: String,
    token: BearerAuth,
) -> Result<HttpResponse, Error> {
    db::verify_onboard_token(&db, token.token()).await?;

    onboard(req_body).await?;
    Ok(HttpResponse::build(http::StatusCode::OK).body(""))
}

async fn onboard(req_body: String) -> Result<String, Error> {
    let (kth_id, card_uid) = match req_body.split_once('#') {
        Some(c) => c,
        None => return Err(Error::ReqestParseError(req_body)),
    };

    sso::onboard(card_uid, kth_id).await?;
    Ok(card_uid.to_string())
}

/// Handles the `/card/onboard` API, which is to be used when a user checks in or out from a meeting
/// but does not have a card uid associated with them. First onboards the user, and then updates
/// their meeting attendance.
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

    let card_uid = onboard(req_body).await?;
    let member_info = sso::get_member_info(&card_uid).await?;
    card(tx, &member_info, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(member_info.kth_id))
}

//! Handles all functions for Karon's API endpoints.

use actix_session::Session;
use actix_web::{
    HttpResponse, http, post,
    web::{self, Data},
};
use either::Either;
use actix_web_httpauth::extractors::bearer::BearerAuth;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    client::{Event, KTH_ID, MEETING_ID}, db::{self, Db, Meeting}, server::Error, sso::{self, Member, check_moderator}, voteit::{self, Permissions, VoteItRequest},
};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CardData {
    card_uid: String,
}

/// Handles the `/card` API, which is to be used when a user checks in or out from a meeting.
#[post("/card")]
pub async fn card_api(
    db: Data<Db>,
    tx: Data<tokio::sync::mpsc::Sender<(Either<String, Uuid>, Member, VoteItRequest)>>,
    event_stream: Data<async_broadcast::Sender<(Either<String, Uuid>, Event)>>,
    form: web::Form<CardData>,
    token: actix_web::Either<BearerAuth, Session>,
) -> Result<HttpResponse, Error> {
    let (user, meeting) = match &token {
        actix_web::Either::Left(token) => (
            Either::Right(Uuid::parse_str(token.token())?),
            db::get_meeting_from_token(&db, token.token()).await?,
        ),
        actix_web::Either::Right(session) => {
            let kthid = session.get(KTH_ID).unwrap().unwrap();
            let Some(meeting_id) = session.get(MEETING_ID).map_err(|_| Error::TokenExpired)?
            else {
                return Err(Error::NotConnectedToMeeting);
            };

            (
                Either::Left(kthid),
                db::get_meeting(&db, meeting_id).await?,
            )
        }
    };

    let member_info = match sso::get_member_info(&form.card_uid).await {
        Ok(member_info) => member_info,
        Err(error) => {
            match error {
                Error::CardNotExisting(ref card_uid) if matches!(token, actix_web::Either::Right(_)) => {
                    let _ = event_stream
                        .broadcast((
                            user,
                            Event::Onboard {
                                card_uid: card_uid.clone(),
                            },
                        ))
                        .await
                        .unwrap();
                }
                _ => {}
            }
            return Err(error);
        }
    };

    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    let kth_id = member_info.kth_id.clone();

    card(tx, event_stream, user, member_info, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(kth_id))
}

pub async fn card(
    tx: Data<tokio::sync::mpsc::Sender<(Either<String, Uuid>, Member, VoteItRequest)>>,
    event_stream: Data<async_broadcast::Sender<(Either<String, Uuid>, Event)>>,
    user: Either<String, Uuid>,
    member_info: Member,
    meeting: Meeting,
) -> Result<(), Error> {
    let mut perms: Permissions = member_info.member_type.into();

    if !perms.contains(voteit::Permissions::PARTICIPANT) {
        let _ = event_stream
            .broadcast((
                user,
                Event::NonMember {
                    name: member_info.name,
                    picture: member_info.picture,
                },
            ))
            .await
            .unwrap();
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

    tx.send((user, member_info, req)).await?;

    Ok(())
}

/// Handles the `/onboard` API, which is to be used when adding a new card uid to a user in the
/// membership database.
#[post("/onboard")]
pub async fn onboard_api(
    db: Data<Db>,
    form: web::Form<OnboardData>,
    token: actix_web::Either<BearerAuth, Session>,
) -> Result<HttpResponse, Error> {
    match token {
        actix_web::Either::Left(token) => db::verify_onboard_token(&db, token.token()).await?,
        actix_web::Either::Right(_session) => {}
    }

    onboard(format!("{}#{}", form.kth_id, form.card_uid)).await?;
    Ok(HttpResponse::build(http::StatusCode::OK).body(""))
}

pub async fn onboard(req_body: String) -> Result<String, Error> {
    let (kth_id, card_uid) = match req_body.split_once('#') {
        Some(c) => c,
        None => return Err(Error::ReqestParseError(req_body)),
    };

    sso::onboard(card_uid, kth_id).await?;
    Ok(card_uid.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OnboardData {
    pub kth_id: String,
    pub card_uid: String,
}

/// Handles the `/card/onboard` API, which is to be used when a user checks in or out from a meeting
/// but does not have a card uid associated with them. First onboards the user, and then updates
/// their meeting attendance.
#[post("/card/onboard")]
pub async fn card_onboard_api(
    db: Data<Db>,
    tx: Data<tokio::sync::mpsc::Sender<(Either<String, Uuid>, Member, VoteItRequest)>>,
    event_stream: Data<async_broadcast::Sender<(Either<String, Uuid>, Event)>>,
    form: web::Form<OnboardData>,
    token: actix_web::Either<BearerAuth, Session>,
) -> Result<HttpResponse, Error> {
    let (user, meeting) = match token {
        actix_web::Either::Left(token) => (
            Either::Right(Uuid::parse_str(token.token())?),
            db::get_meeting_from_token(&db, token.token()).await?,
        ),
        actix_web::Either::Right(session) => {
            let kthid = session.get(KTH_ID).unwrap().unwrap();
            let Some(meeting_id) = session.get(MEETING_ID).map_err(|_| Error::TokenExpired)?
            else {
                return Err(Error::NotConnectedToMeeting);
            };

            (
                Either::Left(kthid),
                db::get_meeting(&db, meeting_id).await?,
            )
        }
    };

    if !meeting.active {
        return Err(Error::MeetingNotActive);
    }

    let data = format!("{}#{}", form.kth_id, form.card_uid);

    let card_uid = onboard(data).await?;
    let member_info = sso::get_member_info(&card_uid).await?;
    let kth_id = member_info.kth_id.clone();
    card(tx, event_stream, user, member_info, meeting).await?;

    Ok(HttpResponse::build(http::StatusCode::OK).body(kth_id))
}

use actix_session::Session;
use actix_web::{
    Either, HttpResponse, Responder, ResponseError, get,
    http::StatusCode,
    post,
    web::{self, Data, Form},
};
use askama::Template;
use async_stream::stream;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use thiserror::Error;

use crate::{
    db::{self, Db, Meeting},
    sso::MemberTypes,
};

pub enum Event {
    Joined {
        picture: String,
        name: String,
        member_type: MemberTypes,
    },
    Left {
        picture: String,
        name: String,
    },
    Onboard {
        card_uid: String,
    },
    NonMember,
}

#[derive(Debug, Error)]
enum ClientError {
    #[error("failed to render template: {0}")]
    TemplateError(#[from] askama::Error),
    #[error("failed to get from session: {0}")]
    SessionError(#[from] actix_session::SessionGetError),
    #[error("db failure: {0}")]
    DBError(#[from] sqlx::Error),
}

impl ResponseError for ClientError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Scanner {
    Kerberos(Uuid),
    NFC,
}

#[derive(Template)]
#[template(path = "voteit.html")]
struct VoteITView {}

#[derive(Template)]
#[template(path = "meeting.html")]
struct MeetingView {
    voteit_token: String,
}

#[derive(Template)]
#[template(path = "init.html")]
struct InitView {}

#[derive(Template)]
#[template(path = "check.html")]
struct CheckView {
    scanner: Scanner,
    meeting: Meeting,
}

#[derive(Template)]
#[template(path = "action.html")]
struct ActionPartial {
    image: String,
    action: String,
    name: String,
}

#[derive(Template)]
#[template(path = "onboard.html")]
struct OnboardPartial {
    card_uid: String,
}

#[get("/")]
pub async fn index(db: Data<Db>, session: Session) -> Result<impl Responder, ClientError> {
    let Some(meeting_id) = session.get::<Uuid>("meeting-id")? else {
        let template = VoteITView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    let Some(scanner) = session.get::<Scanner>("scanner")? else {
        let template = InitView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    let meeting = db::get_meeting(&db, meeting_id).await?;

    let template = CheckView { scanner, meeting };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct JoinMeetingForm {
    voteit_token: String,
}

#[post("/join/meeting")]
pub async fn join_meeting(
    db: Data<Db>,
    session: Session,
    form: Form<JoinMeetingForm>,
) -> Result<HttpResponse, ClientError> {
    let Ok(meeting) = db::get_meeting_from_votit_token(&db, &form.voteit_token).await else {
        let template = MeetingView {
            voteit_token: form.voteit_token.clone(),
        };

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    let _ = session.insert("meeting-id", meeting.meeting_id);

    let template = InitView {};

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct InitMeetingForm {
    name: String,
    date: NaiveDate,
    voteit_token: String,
}

#[post("/init/meeting")]
pub async fn init_meeting(
    db: Data<Db>,
    session: Session,
    form: Form<InitMeetingForm>,
) -> Result<HttpResponse, ClientError> {
    let meeting_id = db::create_meeting(
        &db,
        form.name.clone(),
        form.date.clone(),
        form.voteit_token.clone(),
    )
    .await?;

    let _ = session.insert("meeting-id", meeting_id);

    let template = InitView {};

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[post("/meeting/active")]
pub async fn activate_meeting(db: Data<Db>, session: Session) -> Result<HttpResponse, ClientError> {
    let Some(meeting_id) = session.get::<Uuid>("meeting-id")? else {
        let template = VoteITView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    db::enable_meeting(&db, meeting_id).await?;

    Ok(HttpResponse::Ok().body(r##"<div id="active"><p>Meeting is active</p><button hx-post="/meeting/inactive" hx-target="#active">Activate Meeting</button></div>"##))
}

#[post("/meeting/inactive")]
pub async fn deactivate_meeting(
    db: Data<Db>,
    session: Session,
) -> Result<HttpResponse, ClientError> {
    let Some(meeting_id) = session.get::<Uuid>("meeting-id")? else {
        let template = VoteITView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    db::disable_meeting(&db, meeting_id).await?;

    Ok(HttpResponse::Ok().body(r##"<div id="active"><p>Meeting is inactive</p><button hx-post="/meeting/active" hx-target="#active">Deactivate Meeting</button></div>"##))
}

#[post("/init/kerberos")]
pub async fn init_kerberos(db: Data<Db>, session: Session) -> Result<HttpResponse, ClientError> {
    let Some(meeting_id) = session.get::<Uuid>("meeting-id")? else {
        let template = VoteITView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    let meeting = db::get_meeting(&db, meeting_id).await?;

    let scanner = Scanner::Kerberos(meeting.kerberos_token);

    let _ = session.insert("scanner", scanner.clone());

    let template = CheckView { scanner, meeting };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[post("/init/nfc")]
pub async fn init_nfc(db: Data<Db>, session: Session) -> Result<HttpResponse, ClientError> {
    let Some(meeting_id) = session.get::<Uuid>("meeting-id")? else {
        let template = VoteITView {};

        return Ok(HttpResponse::Ok().body(template.render()?));
    };

    let meeting = db::get_meeting(&db, meeting_id).await?;

    let scanner = Scanner::NFC;

    let _ = session.insert("scanner", scanner.clone());

    let template = CheckView { scanner, meeting };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/events")]
pub async fn events(
    session: Session,
    rx: Data<async_channel::Receiver<(Either<String, Uuid>, Event)>>,
) -> Result<HttpResponse, ClientError> {
    let kthid = session.get::<String>("kthid").unwrap().unwrap();
    let scanner = session.get::<Scanner>("scanner").unwrap().unwrap();

    let event_stream = stream! {
        while let Ok((id, event)) = rx.recv().await {
            // Filter the event stream
            match id {
                Either::Left(id) => {if id != kthid {continue;}},
                Either::Right(token) => {match scanner {
                    Scanner::Kerberos(kb_token) => if token != kb_token {continue;},
                    Scanner::NFC => continue
                }}
            }

            let data = match event {
                Event::Joined{name, picture, member_type} => {
                let template = ActionPartial {
                    image: picture,
                    action: format!("Joined the meeting as {member_type} member"),
                    name: name,
                };

                template.render().unwrap().replace('\n', "")
            }
                Event::Left {name, picture} => {
                    let template = ActionPartial {
                        image: picture,
                        name: name,
                        action: format!("Left the meeting")
                    };

                    template.render().unwrap().replace('\n', "")
                }
                Event::Onboard {card_uid} => {
                    let template = OnboardPartial {
                        card_uid
                    };

                    template.render().unwrap().replace('\n', "")
                },
                Event::NonMember => todo!()
            };

            let sse_chunk = format!(
                "event: event\ndata: {data}\n\n",
            );

            println!("{:?}", sse_chunk);

            yield Ok::<_, actix_web::Error>(web::Bytes::from(sse_chunk));
        }
    };

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(event_stream))
}

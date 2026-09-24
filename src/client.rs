use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, get,
    http::StatusCode,
    patch, post, rt,
    web::{self, Data, Form, Redirect},
};
use askama::Template;
use chrono::{DateTime, Local, NaiveDate, Utc};
use either::Either;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use thiserror::Error;
use tokio_stream::StreamExt;
use tracing_log::log;

use crate::{
    db::{self, Attendance, Db, Meeting},
    server::{self, api::OnboardData},
    sso::{self, MemberTypes, Populate, onboard},
};

pub const KTH_ID: &str = "kth-id";
pub const KERBEROS_TOKEN: &str = "kerberos-token";
pub const SCANNER: &str = "scanner";
pub const MEETING_ID: &str = "meeting-id";

#[derive(Debug, Clone)]
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
    NonMember {
        picture: String,
        name: String,
    },
}

#[derive(Debug, Error)]
enum ClientError {
    #[error("failed to render template: {0}")]
    TemplateError(#[from] askama::Error),
    #[error("missing session key: {0}")]
    SessionDataError(String),
    #[error("failed to get from session: {0}")]
    SessionGetError(#[from] actix_session::SessionGetError),
    #[error("failed to insert data into session: {0}")]
    SessionInsertError(#[from] actix_session::SessionInsertError),
    #[error("db failure: {0}")]
    DBError(#[from] sqlx::Error),
    #[error("error in server fuction: {0}")]
    ServerError(#[from] server::Error),
    #[error("actix error: {0}")]
    ActixError(#[from] actix_web::Error),
}

impl ResponseError for ClientError {
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        HttpResponse::build(self.status_code()).body(self.to_string())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

struct MeetingAttendance {
    name: String,
    entered_at: DateTime<Local>,
    left_at: Option<DateTime<Local>>,
    suffrage: bool,
}

impl MeetingAttendance {
    async fn from(value: Attendance) -> Result<Self, ClientError> {
        let sso_info = sso::get_member_info(&value.kthid).await?;

        Ok(Self {
            name: sso_info.name,
            entered_at: value.entered_at.with_timezone(&Local),
            left_at: value.left_at.map(|time| time.with_timezone(&Local)),
            suffrage: value.suffrage,
        })
    }
}

impl Populate<MeetingAttendance> for Vec<Attendance> {
    type Error = ClientError;

    async fn populate(self) -> Result<Vec<MeetingAttendance>, Self::Error> {
        let mut populated_attendance = Vec::with_capacity(self.len());

        for attendee in self {
            populated_attendance.push(MeetingAttendance::from(attendee).await?);
        }

        Ok(populated_attendance)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Scanner {
    Kerberos(Uuid),
    NFC,
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminView {
    meetings: Vec<Meeting>,
}

#[derive(Template)]
#[template(path = "meeting.html")]
struct CreateMeetingView {}

#[derive(Template)]
#[template(path = "meeting_entry.html")]
struct MeetingEntryPartial {
    meeting: Meeting,
}

#[derive(Template)]
#[template(path = "onboard.html")]
struct OnboardView {
    scanner: Scanner,
}

#[derive(Template)]
#[template(path = "scan.html")]
struct ScanView {
    meeting_id: Uuid,
    scanner: Scanner,
    attendance: Vec<MeetingAttendance>,
}

#[derive(Template)]
#[template(path = "attendance.html")]
struct AttendancePartial {
    meeting_id: Uuid,
    attendance: Vec<MeetingAttendance>,
}

#[derive(Template)]
#[template(path = "nfc_onboard.html")]
struct OnboardPartial {
    card_uid: String,
}

#[derive(Template)]
#[template(path = "action.html")]
struct ActionPartial {
    picture: String,
    action: String,
    name: String,
}

#[derive(Template)]
#[template(path = "non_member.html")]
struct NonMemberPartial {
    picture: String,
    name: String,
}

#[get("/")]
pub async fn admin_view(db: Data<Db>) -> Result<impl Responder, ClientError> {
    let meetings = db::list_meetings(&db).await?;

    let template = AdminView { meetings };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/meeting")]
pub async fn meeting_view() -> Result<HttpResponse, ClientError> {
    let template = CreateMeetingView {};

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CreateMeetingForm {
    name: String,
    date: NaiveDate,
    voteit_token: String,
}

#[post("/meeting")]
pub async fn create_meeting(
    db: Data<Db>,
    form: Form<CreateMeetingForm>,
) -> Result<Redirect, ClientError> {
    db::create_meeting(&db, &form.name, &form.date, &form.voteit_token).await?;

    Ok(Redirect::to("/").see_other())
}

#[patch("/meeting/activate/{id}")]
pub async fn activate_meeting(
    db: Data<Db>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ClientError> {
    let meeting = db::enable_meeting(&db, &id).await?;

    let template = MeetingEntryPartial { meeting };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[patch("/meeting/deactivate/{id}")]
pub async fn deactivate_meeting(
    db: Data<Db>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ClientError> {
    let meeting = db::disable_meeting(&db, &id).await?;

    let template = MeetingEntryPartial { meeting };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/onboard/nfc")]
pub async fn onboard_nfc_view() -> Result<HttpResponse, ClientError> {
    let template = OnboardView {
        scanner: Scanner::NFC,
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[post("/onboard/nfc")]
pub async fn onboard_nfc(form: web::Form<OnboardData>) -> Result<Redirect, ClientError> {
    onboard(&form.card_uid, &form.kth_id).await?;

    Ok(Redirect::to("/onboard/nfc").see_other())
}

#[get("/onboard/kerberos")]
pub async fn onboard_kerberos(db: Data<Db>, session: Session) -> Result<HttpResponse, ClientError> {
    let token = if let Ok(Some(token)) = session.get::<Uuid>(KERBEROS_TOKEN) {
        token
    } else {
        let token = db::create_onboard_token(&db).await?;

        session.insert(KERBEROS_TOKEN, token)?;

        token
    };

    let token = if db::verify_onboard_token(&db, &token.to_string())
        .await
        .is_ok()
    {
        token
    } else {
        let token = db::create_onboard_token(&db).await?;

        session.insert(KERBEROS_TOKEN, token)?;

        token
    };

    let template = OnboardView {
        scanner: Scanner::Kerberos(token),
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/scan/nfc/{meeting_id}")]
pub async fn scan_nfc(
    db: Data<Db>,
    session: Session,
    meeting_id: web::Path<Uuid>,
) -> Result<HttpResponse, ClientError> {
    let attendance = db::list_attendance_for_meeting(&db, &meeting_id)
        .await?
        .populate()
        .await?;

    session.insert(MEETING_ID, *meeting_id)?;
    session.insert(SCANNER, Scanner::NFC)?;

    let template = ScanView {
        meeting_id: *meeting_id,
        scanner: Scanner::NFC,
        attendance,
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/meeting/{meeting_id}/attendance")]
pub async fn meeting_attendance(
    db: Data<Db>,
    meeting_id: web::Path<Uuid>,
) -> Result<HttpResponse, ClientError> {
    let attendance = db::list_attendance_for_meeting(&db, &meeting_id)
        .await?
        .populate()
        .await?;

    let template = AttendancePartial {
        meeting_id: *meeting_id,
        attendance,
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/scan/kerberos/{meeting_id}")]
pub async fn scan_kerberos(
    db: Data<Db>,
    session: Session,
    meeting_id: web::Path<Uuid>,
) -> Result<HttpResponse, ClientError> {
    let meeting = db::get_meeting(&db, *meeting_id).await?;

    let attendance = db::list_attendance_for_meeting(&db, &meeting_id)
        .await?
        .populate()
        .await?;

    session.insert(MEETING_ID, *meeting_id)?;
    session.insert(SCANNER, Scanner::Kerberos(meeting.kerberos_token))?;

    let template = ScanView {
        meeting_id: *meeting_id,
        scanner: Scanner::Kerberos(meeting.kerberos_token),
        attendance,
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[get("/events")]
pub async fn events(
    session: Session,
    rx: Data<async_broadcast::Receiver<(Either<String, Uuid>, Event)>>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, ClientError> {
    let kthid = session
        .get::<String>(KTH_ID)?
        .ok_or(ClientError::SessionDataError(KTH_ID.to_string()))?;
    let scanner = session
        .get::<Scanner>(SCANNER)?
        .ok_or(ClientError::SessionDataError(SCANNER.to_string()))?;

    let mut events = rx.get_ref().clone();

    let (res, mut session, mut ws_stream) = actix_ws::handle(&req, stream)?;
    rt::spawn(async move {
        loop {
            tokio::select! {
                None = ws_stream.next() => break,

                Ok((id, event)) = events.recv() => {
                    // Filter the event stream
                    match id {
                        Either::Left(id) => {
                            if id != kthid {
                                continue;
                            }
                        }
                        Either::Right(token) => match scanner {
                            Scanner::Kerberos(kb_token) => {
                                if token != kb_token {
                                    continue;
                                }
                            }
                            Scanner::NFC => continue,
                        },
                    }

                    let data = match event {
                        Event::Joined {
                            name,
                            picture,
                            member_type,
                        } => {
                            let template = ActionPartial {
                                picture,
                                name,
                                action: format!("Joined the meeting as {member_type} member"),
                            };

                            template.render().expect("template without runtime funcions should not be able to fail")
                        }
                        Event::Left { name, picture } => {
                            let template = ActionPartial {
                                picture,
                                name,
                                action: String::from("Left the meeting"),
                            };

                            template.render().expect("template without runtime funcions should not be able to fail")
                        }
                        Event::Onboard { card_uid } => {
                            let template = OnboardPartial { card_uid };

                            template.render().expect("template without runtime funcions should not be able to fail")
                        },
                        Event::NonMember { name, picture } => {
                            let template = NonMemberPartial {
                                picture,
                                name,
                            };

                            template.render().expect("template without runtime funcions should not be able to fail")
                        }
                    };

                    // If it fails to send -> ws client is closed -> ws server will close on next
                    // loop iteration
                    let _ = session.text(data).await.inspect_err(|e| log::error!("{}", e));
                }
            };
        }
    });

    // respond immediately with response connected to WS session
    Ok(res)
}

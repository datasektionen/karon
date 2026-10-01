use std::time::Duration;

use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, get,
    http::StatusCode,
    patch, post, rt,
    web::{self, Data, Form, Redirect},
};
use askama::Template;
use chrono::{DateTime, Local, NaiveDate};
use either::Either;
use metrics::{counter, gauge};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use thiserror::Error;
use tokio::time::Instant;
use tokio_stream::StreamExt;
use tracing_log::log;

use crate::{
    db::{self, Action, Attendance, Db, FailedSync, Meeting},
    server::{self, api::OnboardData},
    sso::{self, Member, MemberTypes, Populate, onboard},
    voteit::VoteItRequest,
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
    VoteITFailed {
        kthid: String,
        name: String,
        action: Action,
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

struct PopulateFailedVoteIT {
    name: String,
    kthid: String,
    action: Action,
}

impl Populate<PopulateFailedVoteIT> for Vec<FailedSync> {
    type Error = ClientError;

    async fn populate(self) -> Result<Vec<PopulateFailedVoteIT>, Self::Error> {
        let mut desynced_attendance = Vec::with_capacity(self.len());

        for FailedSync { kthid, action } in self {
            let member = sso::get_member_info(&kthid).await?;
            let value = PopulateFailedVoteIT {
                name: member.name,
                kthid: member.kth_id,
                action,
            };
            desynced_attendance.push(value);
        }

        Ok(desynced_attendance)
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
    desynced_attendance: Vec<PopulateFailedVoteIT>,
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

#[derive(Template)]
#[template(path = "voteit_failed.html")]
struct VoteITFailed {
    attendee: PopulateFailedVoteIT,
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

    let desynced_attendance = db::list_unsynced_attendace(&db, *meeting_id)
        .await?
        .populate()
        .await?;

    session.insert(MEETING_ID, *meeting_id)?;
    session.insert(SCANNER, Scanner::NFC)?;

    let template = ScanView {
        meeting_id: *meeting_id,
        scanner: Scanner::NFC,
        attendance,
        desynced_attendance,
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

    let desynced_attendance = db::list_unsynced_attendace(&db, *meeting_id)
        .await?
        .populate()
        .await?;

    session.insert(MEETING_ID, *meeting_id)?;
    session.insert(SCANNER, Scanner::Kerberos(meeting.kerberos_token))?;

    let template = ScanView {
        meeting_id: *meeting_id,
        scanner: Scanner::Kerberos(meeting.kerberos_token),
        attendance,
        desynced_attendance,
    };

    Ok(HttpResponse::Ok().body(template.render()?))
}

#[post("/voteit/retry/{kthid}/{action}")]
pub async fn retry_voteit(
    db: Data<Db>,
    path: web::Path<(String, Action)>,
    tx: Data<
        tokio::sync::mpsc::Sender<(Either<String, Uuid>, Member, VoteItRequest, Option<Action>)>,
    >,
    session: Session,
) -> Result<HttpResponse, ClientError> {
    let (kthid, action) = path.as_ref();

    let user = session.get(KTH_ID).unwrap().unwrap();
    let meeting_id = session.get(MEETING_ID).unwrap().unwrap();

    let meeting = db::get_meeting(&db, meeting_id).await?;

    let member_info = sso::get_member_info(&kthid).await?;

    let perms = member_info.member_type.into();

    let req = VoteItRequest {
        meeting_id: meeting.meeting_id,
        email: member_info.email.clone(),
        perms,
        voteit_token: meeting.voteit_token,
    };

    tx.send((Either::Left(user), member_info, req, Some(*action)))
        .await
        .unwrap();

    Ok(HttpResponse::Ok().body("<article id=\"voteit-fail\" style=\"display: none\"></article>"))
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

    let mut interval = tokio::time::interval(Duration::from_secs(6));

    let (res, mut session, mut ws_stream) = actix_ws::handle(&req, stream)?;
    rt::spawn(async move {
        struct WsGuard;
        impl Drop for WsGuard {
            fn drop(&mut self) {
                gauge!("karon.ws.count").decrement(1);
            }
        }

        // Track connection health
        let mut last_pong = Instant::now();
        gauge!("karon.ws.count").increment(1);
        let _guard = WsGuard; // Guarantees decrement on break/return/panic

        use actix_ws::Message;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Heartbeat check
                    if Instant::now().duration_since(last_pong) > Duration::from_secs(30) {
                        log::info!("ws session timed out");
                        let _ = session.close(None).await;
                        break;
                    }

                    if let Err(e) = session.ping(b"ping").await {
                        log::info!("ws ping failed: {}", e);
                        break;
                    }
                },

                msg = ws_stream.next() => {
                    match msg {
                        Some(Ok(Message::Pong(_))) => {
                            last_pong = Instant::now();
                        }
                        Some(Ok(Message::Ping(bytes))) => {
                            let _ = session.pong(&bytes).await;
                        }
                        Some(Ok(Message::Text(_))) => {
                            // Process client input if expected
                        }
                        Some(Ok(Message::Close(reason))) => {
                            log::info!("ws session closed: {:?}", reason);
                            break;
                        }
                        Some(Err(e)) => {
                            log::info!("ws stream error: {}", e);
                            break;
                        }
                        None => {
                            log::info!("ws stream ended");
                            break;
                        }
                        _ => {}
                    }
                },

                res = events.recv() => {
                    let Ok((id, event)) = res else {
                        log::info!("events channel closed");
                        break;
                    };

                    // Event filtering
                    match id {
                        Either::Left(id) if id != kthid => continue,
                        Either::Right(token) => match scanner {
                            Scanner::Kerberos(kb_token) if token != kb_token => continue,
                            Scanner::NFC => continue,
                            _ => {}
                        },
                        _ => {}
                    }

                    let data = match event {
                        Event::Joined { name, picture, member_type } => {
                            ActionPartial {
                                picture,
                                name,
                                action: format!("Joined the meeting as {member_type} member"),
                            }.render().expect("template render failed")
                        }
                        Event::Left { name, picture } => {
                            ActionPartial {
                                picture,
                                name,
                                action: String::from("Left the meeting"),
                            }.render().expect("template render failed")
                        }
                        Event::Onboard { card_uid } => {
                            OnboardPartial { card_uid }.render().expect("template render failed")
                        }
                        Event::NonMember { name, picture } => {
                            NonMemberPartial { picture, name }.render().expect("template render failed")
                        }
                        Event::VoteITFailed { kthid, name, action } => {
                            let attendee = PopulateFailedVoteIT {name, kthid, action};

                            let template = VoteITFailed {
                                attendee
                            };

                            template.render().expect("template without runtime funcions should not be able to fail")
                        }
                    };

                    if let Err(e) = session.text(data).await {
                        log::error!("ws session send err: {}", e);
                        break;
                    }
                }
            }
        }
    });

    // respond immediately with response connected to WS session
    Ok(res)
}

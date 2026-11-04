//! Handles the worker thread which actually sends the [`VoteItRequest`]s and updates the database.

use std::{env, time::Duration};

use actix_web::web::Data;
use metrics::gauge;
use tokio::sync::mpsc::Receiver;
use tracing_log::log;
use uuid::Uuid;

use either::Either;

/// Sets the max length of the multi-producer-single-consumer queue which all [`VoteItRequest`]s
/// are added to.
pub const WORKER_CHANNEL_SIZE: usize = 32;

use crate::{
    client::Event,
    db::{self, Action, Db},
    server::Error,
    sso::Member,
    voteit::{self, Permissions, VoteItRequest},
};

/// Listens to the [`VoteItRequest`]-queue and tries to update both the database and VoteIT with the
/// requests. If it does not succeed it will retry a couple of times with an exponentially
/// increasing wait time until it has waited 127 seconds in total, at which point if will fail.
pub async fn work(
    db: Data<Db>,
    mut rx: Receiver<(Either<String, Uuid>, Member, VoteItRequest, Option<Action>)>,
    event_stream: async_broadcast::Sender<(Either<String, Uuid>, Event)>,
) {
    while let Some((user, member, mut req, retry_action)) = rx.recv().await {
        gauge!("karon.worker.queue").set(rx.len() as f64);
        let timestamp = chrono::Utc::now();

        let action = if let Some(action) = retry_action {
            action
        } else {
            let action = db::update_attendance(
                &db,
                req.meeting_id,
                &member.kth_id,
                timestamp,
                req.perms.has_suffrage(),
            )
            .await
            .expect("Could not connect to database");

            let event = match action {
                Action::Entered => Event::Joined {
                    name: member.name.clone(),
                    picture: member.picture,
                    member_type: member.member_type,
                },
                Action::Left => {
                    req.perms &= Permissions::default() | Permissions::MODERATOR;
                    Event::Left {
                        name: member.name.clone(),
                        picture: member.picture,
                    }
                }
            };

            match event_stream.try_broadcast((user.clone(), event)) {
                Err(e) if e.is_full() => {
                    log::warn!("event stream buffer full, dropping event");
                }
                Err(s) => {
                    log::error!(
                        "Error broadcasting event for user {}: {:?}",
                        member.kth_id,
                        s
                    );
                }
                Ok(s) => {
                    log::info!("Broadcasted event for user {}: {:?}", member.kth_id, s);
                }
            };

            action
        };

        let mut sleep_time = 1;

        loop {
            match voteit::update_attendance(&req).await {
                Ok(_) => {
                    match action {
                        Action::Entered => log::info!("{} has entered meeting", &member.kth_id),
                        Action::Left => log::info!("{} has left meeting", &member.kth_id),
                    };

                    let _ = db::set_attendance_synced(&db, &member.kth_id)
                        .await
                        .expect("Could not connect to database");

                    break;
                }

                Err(Error::VoteItRequestFail(e)) => {
                    if sleep_time > 64 {
                        log::error!(
                            "Unable to send requests to VoteIT ({}) for 127 seconds. Dropping packet {req} at {timestamp}.",
                            &env::var("VOTEIT_URL").expect("VoteIT url not found"),
                        );

                        match event_stream.try_broadcast((
                            user,
                            Event::VoteITFailed {
                                kthid: member.kth_id.clone(),
                                name: member.name,
                                action,
                            },
                        )) {
                            Err(e) if e.is_full() => {
                                log::warn!("event stream buffer full, dropping event");
                            }
                            Err(s) => {
                                log::error!(
                                    "Error broadcasting event for user {}: {:?}",
                                    member.kth_id,
                                    s
                                );
                            }
                            Ok(s) => {
                                log::info!("Broadcasted event for user {}: {:?}", member.kth_id, s);
                            }
                        };

                        break;
                    }

                    log::info!(
                        "Failed to send packet to VoteIT. Waiting {sleep_time} seconds.\n{e}"
                    );
                    tokio::time::sleep(Duration::from_secs(sleep_time)).await;
                    sleep_time *= 2;
                    continue;
                }
                _ => break,
            };
        }
    }
}

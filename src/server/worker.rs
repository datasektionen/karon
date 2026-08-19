//! Handles the worker thread which actually sends the [`VoteItRequest`]s and updates the database.

use std::{env, time::Duration};

use actix_web::{Either, web::Data};
use tokio::sync::mpsc::Receiver;
use tracing_log::log;
use uuid::Uuid;

/// Sets the max length of the multi-producer-single-consumer queue which all [`VoteItRequest`]s
/// are added to.
pub const WORKER_CHANNEL_SIZE: usize = 64;

use crate::{
    client::Event, db::{self, Action, Db}, server::Error, sso::Member, voteit::{self, Permissions, VoteItRequest}
};

/// Listens to the [`VoteItRequest`]-queue and tries to update both the database and VoteIT with the
/// requests. If it does not succeed it will retry a couple of times with an exponentially
/// increasing wait time until it has waited 127 seconds in total, at which point if will fail.
pub async fn work(
    db: Data<Db>,
    mut rx: Receiver<(Either<String, Uuid>, Member, VoteItRequest)>,
    event_stream: async_channel::Sender<(Either<String, Uuid>, Event)>,
) {
    while let Some((user, member, mut req)) = rx.recv().await {
        let timestamp = chrono::Utc::now();
        let action = db::update_attendance(
            &db,
            req.meeting_id,
            &req.email,
            timestamp,
            req.perms.has_suffrage(),
        )
        .await
        .expect("Could not connect to database");

        match action {
            Action::Entered => {
                let _ = event_stream
                    .send((
                        user,
                        Event::Joined {
                            name: member.name,
                            picture: member.picture,
                            member_type: member.member_type,
                        },
                    ))
                    .await
                    .unwrap();
            }
            Action::Left => {
                req.perms &= Permissions::default() | Permissions::MODERATOR;
                let _ = event_stream
                    .send((
                        user,
                        Event::Left {
                            name: member.name,
                            picture: member.picture,
                        },
                    ))
                    .await
                    .unwrap();
            }
        };

        let mut sleep_time = 1;

        loop {
            match voteit::update_attendance(&req).await {
                Ok(_) => {
                    match action {
                        Action::Entered => log::info!("{} has entered meeting", &req.email),
                        Action::Left => log::info!("{} has left meeting", &req.email),
                    };
                    break;
                }

                Err(Error::VoteItRequestFail(e)) => {
                    if sleep_time > 64 {
                        log::error!(
                            "Unable to send requests to VoteIT ({}) for 127 seconds. Dropping packet {req} at {timestamp}.",
                            &env::var("VOTEIT_URL").expect("VoteIT url not found"),
                        );
                        // TODO: Should we try to reverse the database entry?
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

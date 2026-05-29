use std::{env, time::Duration};

use actix_web::web::Data;
use tokio::sync::mpsc::Receiver;
use tracing_log::log;

pub const WORKER_CHANNEL_SIZE: usize = 64;

use crate::{
    db::{self, Action, Db},
    server::Error,
    voteit::{self, Permissions, VoteItRequest},
};

pub async fn work(db: Data<Db>, mut rx: Receiver<VoteItRequest>) {
    while let Some(mut req) = rx.recv().await {
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
            Action::Entered => (),
            Action::Left => {
                req.perms &= Permissions::default() | Permissions::MODERATOR;
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
                // TODO: We really should not silently just break when encountering errors.
                _ => break,
            };
        }
    }
}

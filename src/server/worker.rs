use std::time::Duration;

use actix_web::web::Data;
use tokio::sync::mpsc::Receiver;

use crate::{
    db::{self, Db},
    server::Error,
    voteit::{self, VoteItRequest},
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

        let mut sleep_time = 1;

        // TODO: We really should not silently just break when encountering errors. Log, maybe?
        loop {
            if sleep_time > 64 {
                break;
            }
            match voteit::update_attendance(action, &mut req).await {
                Ok(_) => break,
                Err(Error::VoteItRequestFail(e)) => {
                    // TODO: Log this instead.
                    println!("{:?}", e);
                    tokio::time::sleep(Duration::from_secs(sleep_time)).await;
                    sleep_time *= 2;
                    continue;
                }
                _ => break,
            };
        }
    }
}

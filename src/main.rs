use std::env;

use actix_web::{App, HttpServer, middleware, web::Data};
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing_actix_web::TracingLogger;
use tracing_log::log;

use crate::{
    db::{Db, create_meeting, enable_meeting, get_meeting},
    server::worker::{self, work},
};

mod db;
mod server;
mod sso;
mod voteit;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();

    let pool = PgPool::connect(&env::var("DATABASE_URL").expect("database not found"))
        .await
        .expect("Failed to connect to database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply database migrations");

    let (tx, rx) = mpsc::channel(worker::WORKER_CHANNEL_SIZE);

    let db = Data::new(Db::new(pool));

    let meeting_id = create_meeting(
        &db,
        "test-SM".to_string(),
        chrono::Utc::now().date_naive(),
        "TOKEN".to_string(),
    )
    .await
    .unwrap();

    let meeting = get_meeting(&db, meeting_id).await.unwrap();
    log::info!("Kerberos token: {}", meeting.kerberos_token);

    enable_meeting(&db, meeting.meeting_id).await.unwrap();

    tokio::spawn(work(db.clone(), rx));

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            .app_data(Data::new(tx.to_owned()))
            .wrap(middleware::Compress::default())
            .wrap(TracingLogger::default())
            .service(server::api::card_api)
            .service(server::api::onboard_api)
            .service(server::api::card_onboard_api)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

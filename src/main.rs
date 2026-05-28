use std::env;

use actix_web::{App, HttpServer, middleware, web::Data};
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::EnvFilter;

use crate::{db::Db, server::worker::work};

mod db;
mod server;
mod sso;
mod voteit;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("init");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::default())
        .init();

    let pool = PgPool::connect(&env::var("DATABASE_URL").expect("database not found"))
        .await
        .expect("lol");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply database migrations");

    let (tx, rx) = mpsc::channel(64);

    println!("migration completed");

    let db = Data::new(Db::new(pool));

    tokio::spawn(work(db.clone(), rx));

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            .app_data(tx.to_owned())
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

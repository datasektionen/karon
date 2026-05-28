use std::env;

use actix_web::{App, HttpServer, middleware, web::Data};
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::EnvFilter;

use crate::{db::Db, voteit::VoteItRequest};

mod db;
mod server;
mod sso;
mod voteit;

pub struct AppState {
    pub db: Db,
    pub producer: tokio::sync::mpsc::Sender<VoteItRequest>,
}

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

    let (tx, mut rx) = mpsc::channel(32);

    println!("migration completed");

    let data = Data::new(AppState {
        db: Db::new(pool),
        producer: tx,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(data.to_owned())
            .wrap(middleware::Compress::default())
            .wrap(TracingLogger::default())
            .service(server::card_api)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

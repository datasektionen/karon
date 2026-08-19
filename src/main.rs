use std::env;

use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    App, HttpServer,
    cookie::Key,
    web::{self, Data, scope},
};
use jsonwebtoken::{DecodingKey, EncodingKey};
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing_actix_web::TracingLogger;

use crate::{
    auth::AuthMiddleware,
    db::Db,
    oidc::OIDCClient,
    server::worker::{self, work},
};

mod auth;
mod client;
mod cookies;
mod db;
mod oidc;
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

    let db = Data::new(Db::new(pool));

    let decoding_key = Data::new(DecodingKey::from_secret(
        env::var("APP_SECRET")
            .expect("APP_SECRET to be set")
            .as_bytes(),
    ));
    let encoding_key = Data::new(EncodingKey::from_secret(
        env::var("APP_SECRET")
            .expect("APP_SECRET to be set")
            .as_bytes(),
    ));
    let oidc_client = web::Data::new(OIDCClient::new().await.unwrap());

    let (worker_tx, worker_rx) = mpsc::channel(worker::WORKER_CHANNEL_SIZE);
    let (event_stream_tx, event_stream_rx) = async_channel::unbounded();

    tokio::spawn(work(db.clone(), worker_rx, event_stream_tx.clone()));

    let port = env::var("PORT")
        .expect("PORT not found.")
        .parse::<u16>()
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .wrap(
                SessionMiddleware::builder(
                    CookieSessionStore::default(),
                    Key::derive_from(
                        env::var("APP_SECRET")
                            .expect("APP_SECRET to be set")
                            .as_bytes(),
                    ),
                )
                .cookie_name(String::from("auth_token"))
                .build(),
            )
            .app_data(decoding_key.clone())
            .app_data(oidc_client.clone())
            .app_data(db.clone())
            .app_data(Data::new(worker_tx.to_owned()))
            .app_data(Data::new(event_stream_tx.clone()))
            .app_data(Data::new(event_stream_rx.clone()))
            // .wrap(middleware::Compress::default())
            .wrap(TracingLogger::default())
            .service(scope("auth").service(auth::callback))
            .service(server::api::card_api)
            .service(server::api::onboard_api)
            .service(server::api::card_onboard_api)
            .service(
                scope("")
                    .wrap(AuthMiddleware::new(
                        oidc_client.clone(),
                        encoding_key.clone(),
                    ))
                    .service(client::index)
                    .service(client::init_kerberos)
                    .service(client::init_nfc)
                    .service(client::init_meeting)
                    .service(client::join_meeting)
                    .service(client::activate_meeting)
                    .service(client::deactivate_meeting)
                    .service(client::events),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

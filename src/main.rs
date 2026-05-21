use std::env;

use actix_web::{App, HttpServer, middleware};
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;
use tracing_log::LogTracer;
use tracing_subscriber::EnvFilter;

mod db;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("init");
    LogTracer::init().unwrap();

    // tracing_subscriber::fmt()
    //     .with_env_filter(EnvFilter::default())
    //     .init();

    let pool = PgPool::connect(&env::var("DATABASE_URL").expect("databae not found"))
        .await
        .expect("lol");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply database migrations");

    println!("migration completed");

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .wrap(TracingLogger::default())
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

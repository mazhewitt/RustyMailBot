mod config;
mod models;
mod routes;
mod handlers;
mod services;
mod utils;

use actix_files::Files;
use actix_web::{web, App, HttpServer};
use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::{Key, SameSite};
use log::info;
use routes::app_state::AppState;
use config::init_logging;
use services::{email_service, embedding_service};


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_logging();
    info!("Starting server on http://127.0.0.1:8080");

    let secret_key = Key::from("0123456789012345678901234567890123456789012345678901234567890123".as_bytes());

    // Create an instance of Ollama and GlobalSessionManager.
    let ollama = embedding_service::create_ollama();
    let session_manager = email_service::create_session_manager();

    let app_state = AppState {   ollama, session_manager };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(false)
                    .cookie_same_site(SameSite::Lax)
                    .build()
            )
            // Dynamic routes must be configured first:
            .configure(routes::session_routes::init_routes)
            .configure(routes::chat_routes::init_routes)
            .configure(routes::oauth_routes::init_routes)
            // Finally, static files:
            .service(Files::new("/", "./static").index_file("index.html"))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
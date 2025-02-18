mod gmail;

use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Error};
use actix_web::middleware::Logger;
use async_stream::stream;
use bytes::Bytes;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use log::{info};
use gmail::{check_auth, oauth_login, oauth_callback, get_inbox_messages};

/// This endpoint streams a greeting unless the client sends "@list",
/// in which case it streams Gmail inbox messages.
async fn stream_greeting(req_body: web::Json<Value>) -> HttpResponse {
    info!("Received request with payload: {:?}", req_body);

    let input = req_body.get("name")
        .and_then(Value::as_str)
        .unwrap_or("world")
        .to_string();

    if input.trim() == "@list" {
        info!("Streaming inbox messages as requested with @list");
        let inbox_stream = stream! {
            match get_inbox_messages().await {
                Ok(messages) => {
                    for message in messages {
                        if let Ok(json_message) = serde_json::to_string(&message) {
                            yield Ok(Bytes::from(json_message + "\n"));
                        } else {
                            yield Err(actix_web::error::ErrorInternalServerError("Serialization error"));
                        }
                    }
                },
                Err(e) => yield Err(actix_web::error::ErrorInternalServerError(e)),
            }
        };

        return HttpResponse::Ok()
            .content_type("application/json")
            .streaming(inbox_stream);
    }

    let greeting_stream = stream! {
        info!("Streaming chunk: 'hello '");
        yield Ok::<_, Error>(Bytes::from("hello "));
        sleep(Duration::from_millis(500)).await;
        info!("Streaming chunk: '{}'", input);
        yield Ok(Bytes::from(input));
    };

    HttpResponse::Ok()
        .content_type("text/plain")
        .streaming(greeting_stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    info!("Starting server on http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            // Use the Gmail module’s endpoints:
            .route("/stream", web::post().to(stream_greeting))
            .route("/check_auth", web::get().to(check_auth))
            .route("/oauth/login", web::get().to(oauth_login))
            .route("/oauth/callback", web::get().to(oauth_callback))
            // Serve static files (including index.html) from the "./static" directory.
            .service(Files::new("/", "./static").index_file("index.html"))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
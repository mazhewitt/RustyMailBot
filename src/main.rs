use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Error, middleware::Logger};
use async_stream::stream;
use bytes::Bytes;
use std::time::Duration;
use tokio::time::sleep;
use serde_json::Value;

async fn stream_greeting(req_body: web::Json<Value>) -> HttpResponse {
    // Log the received request
    log::info!("Received request with payload: {:?}", req_body);

    // Extract the name from the JSON payload; default to "world" if missing.
    let name = req_body.get("name")
        .and_then(Value::as_str)
        .unwrap_or("world")
        .to_string();

    // Create an asynchronous stream that yields two chunks.
    let greeting_stream = stream! {
        log::info!("Streaming chunk: 'hello '");
        yield Ok::<Bytes, Error>(Bytes::from("hello "));
        sleep(Duration::from_millis(500)).await;
        log::info!("Streaming chunk: '{}'", name);
        yield Ok(Bytes::from(name));
    };

    HttpResponse::Ok()
        .content_type("text/plain")
        .streaming(greeting_stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    log::info!("Starting server on http://127.0.0.1:8080");
    HttpServer::new(|| {
        App::new()
            // Use the Logger middleware to log incoming requests.
            .wrap(Logger::default())
            // Endpoint to handle the streaming greeting.
            .route("/stream", web::post().to(stream_greeting))
            // Serve static files (including index.html) from the "./static" directory.
            .service(Files::new("/", "./static").index_file("index.html"))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
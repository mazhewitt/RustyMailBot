use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Error, HttpRequest, Responder};
use actix_web::middleware::Logger;
use async_stream::stream;
use bytes::Bytes;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use std::path::Path;
use std::fs;
use serde::{Serialize, Deserialize};

// For Gmail API
use google_gmail1 as gmail1;

// OAuth2 imports
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, TokenUrl, RedirectUrl, ClientId, ClientSecret, Scope, CsrfToken, AuthorizationCode};
use oauth2::reqwest::async_http_client;

// Change these scopes as needed
const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

// This endpoint remains as before.
async fn stream_greeting(req_body: web::Json<Value>) -> HttpResponse {
    log::info!("Received request with payload: {:?}", req_body);

    let name = req_body.get("name")
        .and_then(Value::as_str)
        .unwrap_or("world")
        .to_string();

    let greeting_stream = stream! {
        log::info!("Streaming chunk: 'hello '");
        yield Ok::<_, Error>(Bytes::from("hello "));
        sleep(Duration::from_millis(500)).await;
        log::info!("Streaming chunk: '{}'", name);
        yield Ok(Bytes::from(name));
    };

    HttpResponse::Ok()
        .content_type("text/plain")
        .streaming(greeting_stream)
}

/// Check whether a token file exists. (In a real app, you’d check for validity.)
async fn check_auth() -> impl Responder {
    let authenticated = Path::new("tokencache.json").exists();
    HttpResponse::Ok().json(serde_json::json!({ "authenticated": authenticated }))
}

/// /oauth/login: Build the Google OAuth URL and redirect the user.
async fn oauth_login() -> impl Responder {
    // Read the client secret from your file.
    let secret: oauth2::basic::BasicClient = {
        // For simplicity we use the same file that you provided.
        // In your client_secret.json the "installed" object is expected.
        let secret_str = fs::read_to_string("./cfg/client_secret.json")
            .expect("Unable to read client secret file");
        let json_secret: serde_json::Value = serde_json::from_str(&secret_str)
            .expect("Invalid JSON in client secret file");

        let installed = &json_secret["installed"];
        let client_id = ClientId::new(installed["client_id"].as_str().unwrap().to_string());
        let client_secret = ClientSecret::new(installed["client_secret"].as_str().unwrap().to_string());
        let auth_url = AuthUrl::new(installed["auth_uri"].as_str().unwrap().to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new(installed["token_uri"].as_str().unwrap().to_string())
            .expect("Invalid token endpoint URL");

        BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url))
            .set_redirect_uri(RedirectUrl::new("http://localhost:8080/oauth/callback".to_string())
                .expect("Invalid redirect URL"))
    };

    // Generate the authorization URL.
    let (auth_url, _csrf_token) = secret
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(GMAIL_SCOPE.to_string()))
        .url();

    // Redirect the browser to Google’s OAuth 2.0 server.
    HttpResponse::Found()
        .append_header(("Location", auth_url.to_string()))
        .finish()
}

/// /oauth/callback: Handle the redirect back from Google.
async fn oauth_callback(req: HttpRequest) -> impl Responder {
    // Extract the "code" query parameter.
    let query: Vec<(String, String)> = req.query_string()
        .split('&')
        .filter_map(|s| {
            let mut split = s.split('=');
            if let (Some(key), Some(value)) = (split.next(), split.next()) {
                Some((key.to_string(), value.to_string()))
            } else {
                None
            }
        })
        .collect();
    let query_map: std::collections::HashMap<_, _> = query.into_iter().collect();

    let code = match query_map.get("code") {
        Some(code) => code.to_string(),
        None => return HttpResponse::BadRequest().body("Missing code"),
    };

    // Rebuild the OAuth2 client (ideally you’d share this in app state)
    let oauth_client: BasicClient = {
        let secret_str = fs::read_to_string("./cfg/client_secret.json")
            .expect("Unable to read client secret file");
        let json_secret: serde_json::Value = serde_json::from_str(&secret_str)
            .expect("Invalid JSON in client secret file");

        let installed = &json_secret["installed"];
        let client_id = ClientId::new(installed["client_id"].as_str().unwrap().to_string());
        let client_secret = ClientSecret::new(installed["client_secret"].as_str().unwrap().to_string());
        let auth_url = AuthUrl::new(installed["auth_uri"].as_str().unwrap().to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new(installed["token_uri"].as_str().unwrap().to_string())
            .expect("Invalid token endpoint URL");

        BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url))
            .set_redirect_uri(RedirectUrl::new("http://localhost:8080/oauth/callback".to_string())
                .expect("Invalid redirect URL"))
    };

    // Exchange the code with Google for a token.
    let token_result = oauth_client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await;

    match token_result {
        Ok(token) => {
            // For simplicity, write the token JSON to a file.
            let token_json = serde_json::to_string(&token).unwrap();
            fs::write("tokencache.json", token_json)
                .expect("Unable to write token to file");
            // Redirect back to the main page.
            HttpResponse::Found().append_header(("Location", "/")).finish()
        }
        Err(err) => {
            HttpResponse::InternalServerError().body(format!("Token exchange error: {:?}", err))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging.
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    log::info!("Starting server on http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            // Our endpoints:
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
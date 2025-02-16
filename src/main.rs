
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
use reqwest::Client;
use log::{info, error};

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

#[derive(Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    id: String,
    payload: Option<MessagePayload>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessagePayload {
    headers: Vec<Header>,
    body: Option<MessageBody>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Header {
    name: String,
    value: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessageBody {
    data: Option<String>,
}

const TOKEN_CACHE_FILE: &str = "tokencache.json";
const GMAIL_API_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=is:inbox";

/// Reads the access token from the cache file
fn read_access_token() -> Result<String, Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(TOKEN_CACHE_FILE)?;
    let token_cache: TokenCache = serde_json::from_str(&file_content)?;
    Ok(token_cache.access_token)
}

/// Fetches the inbox messages using the Gmail API
async fn get_inbox_messages() -> Result<Vec<Message>, Box<dyn std::error::Error>> {
    info!("Getting Inbox messages");
    let access_token = read_access_token()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    info!("Fetching inbox messages from Gmail API...");
    let response = client
        .get(GMAIL_API_URL)
        .bearer_auth(&access_token)
        .send()
        .await?;

    if response.status().is_success() {
        info!("Successfully fetched inbox messages");
        let messages_response: serde_json::Value = response.json().await?;
        let message_ids: Vec<String> = messages_response["messages"].as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
            .collect();

        let mut messages = Vec::new();
        for message_id in message_ids.iter() {
            let message_url = format!("https://gmail.googleapis.com/gmail/v1/users/me/messages/{}", message_id);
            info!("Fetching message details for ID: {}", message_id);
            let message_response = client
                .get(&message_url)
                .bearer_auth(&access_token)
                .send()
                .await?;

            if message_response.status().is_success() {
                info!("Successfully fetched message details for ID: {}", message_id);
                let message: Message = message_response.json().await?;
                messages.push(message);
            }
        }
        Ok(messages)
    } else {
        error!("Failed to fetch inbox: {}", response.status());
        error!("Failed to fetch inbox: {}", response.status());
        Err(format!("Failed to fetch inbox: {}", response.status()).into())
    }
}

#[cfg(test)]
mod tests {
    use log::error;
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_get_inbox_messages() {
        let _ = env_logger::builder().is_test(true).try_init();
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(get_inbox_messages());

        match result {
            Ok(messages) => {
                for message in messages.iter() {
                    let subject_value = message.payload.as_ref().map(|p| p.headers.iter().find(|h| h.name == "Subject").map(|h| h.value.clone()).unwrap_or("(No Subject)".to_string())).unwrap_or("(No Subject)".to_string());
                    let sender_value = message.payload.as_ref().map(|p| p.headers.iter().find(|h| h.name == "From").map(|h| h.value.clone()).unwrap_or("(Unknown Sender)".to_string())).unwrap_or("(Unknown Sender)".to_string());
                    let date_value = message.payload.as_ref().map(|p| p.headers.iter().find(|h| h.name == "Date").map(|h| h.value.clone()).unwrap_or("(Unknown Date)".to_string())).unwrap_or("(Unknown Date)".to_string());

                    println!("Sender: {}", sender_value);
                    println!("Subject: {}", subject_value);
                    println!("Date: {}", date_value);
                    println!("Message ID: {}", message.id);
                    println!("--------------------------");
                }
                assert!(!messages.is_empty());
            }
            Err(e) => error!("Failed to get inbox: {}", e)
        }
    }
}


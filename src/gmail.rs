use actix_web::{HttpResponse, Responder, HttpRequest};
use log::{info, error};
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, TokenUrl, RedirectUrl, ClientId, ClientSecret, Scope, CsrfToken, AuthorizationCode};
use oauth2::reqwest::async_http_client;
use reqwest;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};

const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";
const TOKEN_CACHE_FILE: &str = "tokencache.json";
const GMAIL_API_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=is:inbox";

#[derive(Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub id: String,
    pub payload: Option<MessagePayload>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessagePayload {
    pub headers: Vec<Header>,
    pub body: Option<MessageBody>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageBody {
    pub data: Option<String>,
}

/// Reads the access token from the cache file.
pub fn read_access_token() -> Result<String, Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(TOKEN_CACHE_FILE)?;
    let token_cache: TokenCache = serde_json::from_str(&file_content)?;
    Ok(token_cache.access_token)
}

/// Fetches the inbox messages using the Gmail API.


/// Checks whether a token file exists.
pub async fn check_auth() -> impl Responder {
    let authenticated = Path::new(TOKEN_CACHE_FILE).exists();
    HttpResponse::Ok().json(serde_json::json!({ "authenticated": authenticated }))
}

/// /oauth/login: Builds the Google OAuth URL and redirects the user.
pub async fn oauth_login() -> impl Responder {
    // Read the client secret from file.
    let secret: BasicClient = {
        let secret_str = fs::read_to_string("./cfg/client_secret.json")
            .expect("Unable to read client secret file");
        let json_secret: Value = serde_json::from_str(&secret_str)
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

/// /oauth/callback: Handles the redirect back from Google.
pub async fn oauth_callback(req: HttpRequest) -> impl Responder {
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

    // Rebuild the OAuth2 client (in a real app, consider sharing state).
    let oauth_client: BasicClient = {
        let secret_str = fs::read_to_string("./cfg/client_secret.json")
            .expect("Unable to read client secret file");
        let json_secret: Value = serde_json::from_str(&secret_str)
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
            fs::write(TOKEN_CACHE_FILE, token_json)
                .expect("Unable to write token to file");
            // Redirect back to the main page.
            HttpResponse::Found().append_header(("Location", "/")).finish()
        }
        Err(err) => {
            HttpResponse::InternalServerError().body(format!("Token exchange error: {:?}", err))
        }
    }
}







// Define the custom Email struct with just the fields you need.
#[derive(Debug, serde::Serialize)]
pub struct Email {
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
}

// Helper function to find a header value (case insensitive) from a slice of headers.
fn get_header(headers: &[Value], name: &str) -> Option<String> {
    headers.iter().find(|h| {
        h.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
        .and_then(|h| h.get("value").and_then(|v| v.as_str()).map(String::from))
}

// Helper function to extract the plain text body from a message payload.
// It first checks if the payload itself is plain text; if not, it iterates over its parts.
fn extract_plain_text_body(payload: &Value) -> Option<String> {
    if let Some(mime_type) = payload.get("mimeType").and_then(|m| m.as_str()) {
        if mime_type == "text/plain" {
            return payload.get("body")
                .and_then(|b| b.get("data"))
                .and_then(|d| d.as_str())
                .map(String::from);
        }
    }
    if let Some(parts) = payload.get("parts").and_then(|p| p.as_array()) {
        for part in parts {
            if let Some(part_mime) = part.get("mimeType").and_then(|m| m.as_str()) {
                if part_mime == "text/plain" {
                    return part.get("body")
                        .and_then(|b| b.get("data"))
                        .and_then(|d| d.as_str())
                        .map(String::from);
                }
            }
        }
    }
    None
}

pub async fn get_inbox_messages() -> Result<Vec<Email>, Box<dyn std::error::Error>> {
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
        let messages_response: Value = response.json().await?;
        let message_ids: Vec<String> = messages_response["messages"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect();

        let mut emails = Vec::new();
        for message_id in message_ids.iter() {
            let message_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
                message_id
            );
            info!("Fetching message details for ID: {}", message_id);
            let message_response = client
                .get(&message_url)
                .bearer_auth(&access_token)
                .send()
                .await?;

            if message_response.status().is_success() {
                info!("Successfully fetched message details for ID: {}", message_id);
                let message: Value = message_response.json().await?;
                // Convert headers to a slice of Value to avoid temporary lifetime issues.
                let headers: &[Value] = message["payload"]["headers"]
                    .as_array()
                    .map(|arr| &arr[..])
                    .unwrap_or(&[]);

                let from = get_header(headers, "From");
                let to = get_header(headers, "To");
                let date = get_header(headers, "Date");
                let subject = get_header(headers, "Subject");
                let body_data = extract_plain_text_body(&message["payload"]);

                // Decode the base64url-encoded body using the new API.
                let decoded_body = if let Some(data) = body_data {
                    match URL_SAFE.decode(data) {
                        Ok(bytes) => String::from_utf8(bytes).ok(),
                        Err(e) => {
                            error!("Failed to decode base64 body for message {}: {}", message_id, e);
                            None
                        }
                    }
                } else {
                    None
                };

                emails.push(Email {
                    from,
                    to,
                    date,
                    subject,
                    body: decoded_body,
                });
            }
        }
        Ok(emails)
    } else {
        error!("Failed to fetch inbox: {}", response.status());
        Err(format!("Failed to fetch inbox: {}", response.status()).into())
    }
}
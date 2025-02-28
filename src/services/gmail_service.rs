use log::{info, error, debug};
use reqwest;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::fs;
use std::time::Duration;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use oauth2::TokenResponse;

const TOKEN_CACHE_FILE: &str = "tokencache.json";
const GMAIL_API_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages?q=is:inbox";

#[derive(Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
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

/// A simplified Email struct.
#[derive(Debug, Serialize)]
pub struct Email {
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub message_id: Option<String>,
}

/// Reads the access token from the cache file.
pub fn read_access_token() -> Result<String, Box<dyn std::error::Error>> {
    let file_content = fs::read_to_string(TOKEN_CACHE_FILE)?;
    let token_cache: TokenCache = serde_json::from_str(&file_content)?;
    Ok(token_cache.access_token)
}

/// Fetches inbox messages from the Gmail API.
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
        info!("Successfully fetched inbox ID list");
        let messages_response: Value = response.json().await?;
        let message_ids: Vec<String> = messages_response["messages"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect();

        let mut emails = Vec::new();
        info!("Loading email details");
        for message_id in message_ids.iter() {
            let message_url = format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}",
                message_id
            );
            debug!("Fetching message details for ID: {}", message_id);
            let message_response = client
                .get(&message_url)
                .bearer_auth(&access_token)
                .send()
                .await?;

            if message_response.status().is_success() {
                debug!("Successfully fetched message details for ID: {}", message_id);
                let message: Value = message_response.json().await?;
                let headers: &[Value] = message["payload"]["headers"]
                    .as_array()
                    .map(|arr| &arr[..])
                    .unwrap_or(&[]);

                let from = get_header(headers, "From");
                let to = get_header(headers, "To");
                let date = get_header(headers, "Date");
                let subject = get_header(headers, "Subject");
                let body_data = extract_plain_text_body(&message["payload"]);

                // Decode the base64url-encoded body.
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
                    message_id: Some(message_id.to_string()),
                });
            }
        }
        Ok(emails)
    } else {
        error!("Failed to fetch inbox: {}", response.status());
        Err(format!("Failed to fetch inbox: {}", response.status()).into())
    }
}

/// Refreshes the OAuth token using the provided OAuth client.
///
/// Note: This function now requires you to supply an OAuth2 BasicClient
/// (constructed in your HTTP handler) because token refreshing is a business
/// logic operation that should not build HTTP responses itself.
pub async fn refresh_token(
    oauth_client: &oauth2::basic::BasicClient,
) -> Result<String, Box<dyn std::error::Error>> {
    // Read the current token cache.
    let file_content = fs::read_to_string(TOKEN_CACHE_FILE)?;
    let token_cache: TokenCache = serde_json::from_str(&file_content)?;

    // Ensure we have a refresh token.
    let current_refresh_token = match token_cache.refresh_token {
        Some(rt) => rt,
        None => return Err("No refresh token available. Please re-authenticate.".into())
    };

    // Perform the refresh token exchange.
    let new_token = oauth_client
        .exchange_refresh_token(&oauth2::RefreshToken::new(current_refresh_token))
        .request_async(oauth2::reqwest::async_http_client)
        .await?;

    // Overwrite the token cache with the new token information.
    let token_json = serde_json::to_string(&new_token)?;
    fs::write(TOKEN_CACHE_FILE, token_json)?;
    info!("Token successfully refreshed.");

    // Return the new access token.
    Ok(new_token.access_token().secret().to_string())
}

/// Helper: find a header value (case insensitive) from a slice of headers.
fn get_header(headers: &[Value], name: &str) -> Option<String> {
    headers.iter().find(|h| {
        h.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
        .and_then(|h| h.get("value").and_then(|v| v.as_str()).map(String::from))
}

/// Helper: extract the plain text body from a message payload.
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
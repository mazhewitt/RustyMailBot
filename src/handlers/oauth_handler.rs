use actix_web::{HttpResponse, Responder, HttpRequest};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, TokenUrl, RedirectUrl, ClientId, ClientSecret, Scope,
    CsrfToken, AuthorizationCode,
};
use oauth2::reqwest::async_http_client;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use log::{info, error};
use crate::services::gmail_service;

const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

/// Constructs an OAuth2 BasicClient from your client secret file.
fn build_oauth_client() -> BasicClient {
    // Read client secret from file.
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
        .set_redirect_uri(
            RedirectUrl::new("http://localhost:8080/oauth/callback".to_string())
                .expect("Invalid redirect URL")
        )
}

/// Initiates the OAuth flow by generating the authorization URL and redirecting.
pub async fn oauth_login() -> impl Responder {
    let oauth_client = build_oauth_client();
    // Generate the authorization URL.
    let (auth_url, _csrf_token) = oauth_client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(GMAIL_SCOPE.to_string()))
        .url();

    // Redirect the browser to Google’s OAuth 2.0 server.
    HttpResponse::Found()
        .append_header(("Location", auth_url.to_string()))
        .finish()
}

/// Handles the OAuth callback from Google.
///
/// Extracts the authorization code from the query string, exchanges it for a token,
/// writes the token to the cache, and then redirects back to the main page.
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

    let oauth_client = build_oauth_client();

    // Exchange the code with Google for a token.
    let token_result = oauth_client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await;

    match token_result {
        Ok(token) => {
            // Write the token JSON to the cache file.
            let token_json = serde_json::to_string(&token).unwrap();
            fs::write("tokencache.json", token_json)
                .expect("Unable to write token to file");
            info!("Token successfully obtained and cached.");
            // Redirect back to the main page.
            HttpResponse::Found().append_header(("Location", "/")).finish()
        }
        Err(err) => {
            error!("Token exchange error: {:?}", err);
            HttpResponse::InternalServerError().body(format!("Token exchange error: {:?}", err))
        }
    }

}
pub async fn check_auth() -> impl Responder {
    // If token file doesn't exist, we're unauthenticated.
    if !Path::new("tokencache.json").exists() {
        return HttpResponse::Ok().json(json!({ "authenticated": false }));
    }

    // Try fetching inbox messages to test the token.
    match gmail_service::get_inbox_messages().await {
        Ok(_) => HttpResponse::Ok().json(json!({ "authenticated": true })),
        Err(e) if e.to_string().contains("401") => {
            info!("Token appears expired.");
            HttpResponse::Ok().json(json!({ "authenticated": false, "error": "Token expired" }))
        },
        Err(e) => HttpResponse::Ok().json(json!({ "authenticated": false, "error": e.to_string() }))
    }
}

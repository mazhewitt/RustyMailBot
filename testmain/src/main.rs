use oauth2::{AuthUrl, ClientId, ClientSecret, TokenUrl, RedirectUrl, AuthorizationCode, CsrfToken, Scope, StandardTokenResponse, TokenResponse};
use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct OAuthConfig {
    installed: InstalledConfig,
}

#[derive(Deserialize)]
struct InstalledConfig {
    client_id: String,
    client_secret: String,
    auth_uri: String,
    token_uri: String,
    redirect_uris: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct TokenStorage {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

const CONFIG_FILE: &str = "../data/client_secret.json";
const TOKEN_FILE: &str = "token.json";

#[tokio::main]
async fn main() {
    getOauthToken().await;
}

async fn getOauthToken() {
    let config_data = fs::read_to_string(CONFIG_FILE).expect("Failed to read client secret file");
    let config: OAuthConfig = serde_json::from_str(&config_data).expect("Invalid JSON format");

    let client = BasicClient::new(
        ClientId::new(config.installed.client_id),
        Some(ClientSecret::new(config.installed.client_secret)),
        AuthUrl::new(config.installed.auth_uri).expect("Invalid auth URL"),
        Some(TokenUrl::new(config.installed.token_uri).expect("Invalid token URL")),
    )
        .set_redirect_uri(RedirectUrl::new("urn:ietf:wg:oauth:2.0:oob".to_string()).expect("Invalid redirect URI"));

    let (auth_url, _csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.profile".to_string()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.email".to_string()))
        .url();

    println!("Open this URL in your browser and authorize the app: \n{}", auth_url);

    print!("Enter the authorization code: ");
    io::stdout().flush().unwrap();

    let mut auth_code = String::new();
    io::stdin().read_line(&mut auth_code).expect("Failed to read input");
    let auth_code = auth_code.trim().to_string();

    let token_result = client.exchange_code(AuthorizationCode::new(auth_code))
        .request_async(|request| async {
            oauth2::reqwest::async_http_client(request).await
        })
        .await
        .expect("Failed to exchange code for token");

    let token = TokenStorage {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
        expires_in: token_result.expires_in().map(|d| d.as_secs()),
    };

    let token_json = serde_json::to_string_pretty(&token).expect("Failed to serialize token");
    fs::write(TOKEN_FILE, token_json).expect("Failed to save token");

    println!("Token saved to '{}'.", TOKEN_FILE);
}

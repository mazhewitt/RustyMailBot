use std::error::Error;
use std::sync::Arc;

use google_gmail1::Gmail;
use google_gmail1::oauth2::{read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use hyper::Client;
use hyper_rustls::{ HttpsConnectorBuilder};
use log::{error, info};
use tokio::{io, task};
use tokio::io::{stdin, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use oauth2::{AuthUrl, ClientId, ClientSecret, TokenUrl, RedirectUrl, AuthorizationCode, CsrfToken, Scope, StandardTokenResponse, TokenResponse};
use oauth2::basic::BasicClient;
use oauth2::reqwest::http_client;
use std::fs;
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

const CONFIG_FILE: &str = "./data/client_secret.json";
const TOKEN_FILE: &str = "token.json";


/// A struct that encapsulates the authenticated Gmail client.
struct GmailClient {
    /// The Gmail client is wrapped in an Arc and Mutex for safe concurrent access.
    client: Arc<Mutex<Gmail>>,
}

impl GmailClient {
    /// Logs in to Gmail using OAuth2 interactive flow.
    ///
    /// This function reads the client secret from `client_secret.json` and caches tokens to `tokencache.json`.
    async fn perform_oauth_login() {
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
        io::stdout().flush().await.expect("Failed to flush stdout");

        let mut auth_code = String::new();
        std::io::stdin()
            .read_line(&mut auth_code)
            .expect("Failed to read input");
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


    /// Initialises the Gmail client.
    ///
    /// Checks if the token file exists; if not, performs the OAuth login.
    /// Once a token is available, it builds the authenticator and creates the Gmail client.
    pub async fn initialize() -> Result<Self, Box<dyn Error>> {
        if !Path::new(TOKEN_FILE).exists() {
            println!("No token file found. Initiating OAuth login.");
            Self::perform_oauth_login().await;
        } else {
            println!("Token file found. Using existing credentials.");
        }

        let secret = read_application_secret(CONFIG_FILE).await?;
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();
        let hyper_client = Client::builder().build::<_, hyper::Body>(https);

        let auth = InstalledFlowAuthenticator::builder(
            secret,
            InstalledFlowReturnMethod::HTTPRedirect,
        )
            .persist_tokens_to_disk(TOKEN_FILE)
            .build()
            .await?;

        let gmail = Gmail::new(hyper_client, auth);

        Ok(GmailClient {
            client: Arc::new(Mutex::new(gmail)),
        })
    }

    /// Retrieves and displays the subject and snippet of the latest emails in the inbox.
    ///
    /// It lists messages in the user's inbox and then fetches each message.
    pub async fn get_inbox(&self) -> Result<(), Box<dyn Error>> {
        info!("Fetching inbox messages...");

        // Lock the client for safe async access.
        let mut client_guard = self.client.lock().await;
        info!("Client lock acquired.");

        // List messages in the user's inbox.
        let list_call = client_guard.users().messages_list("me").q("in:inbox");
        info!("API call prepared: listing inbox messages...");

        // Execute API call.
        let (_, list_messages_response) = list_call.doit().await?;
        if let Some(messages) = list_messages_response.messages {
            for message in messages.iter() {
                if let Some(message_id) = &message.id {
                    info!("Fetching message ID: {}", message_id);
                    let _msg_response = client_guard
                        .users()
                        .messages_get("me", message_id)
                        .format("full")
                        .doit()
                        .await?;
                    info!("Message {} fetched successfully.", message_id);
                }
            }
        } else {
            info!("No messages found in inbox.");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use log::info;

    /// This test checks that the initialisation process creates a GmailClient.
    /// If no token file is present, it will run the OAuth flow.
    #[tokio::test]
    async fn test_initialize_creates_client_with_token() -> Result<(), Box<dyn Error>> {
        // For testing, you may wish to simulate a "fresh start" by removing any existing token file.
        if Path::new(TOKEN_FILE).exists() {
            fs::remove_file(TOKEN_FILE)?;
            println!("Existing token file removed for testing.");
        }

        // This will prompt for OAuth if no token is found.
        let gmail_client = GmailClient::initialize().await?;
        info!("Gmail client initialised successfully.");
        Ok(())
    }

    /// This test assumes a valid token file is present and tests the get_inbox method.
    /// Marked as ignored since it performs live API calls.
    #[tokio::test]
    #[ignore]
    async fn test_get_inbox() -> Result<(), Box<dyn Error>> {
        let gmail_client = GmailClient::initialize().await?;
        gmail_client.get_inbox().await?;
        Ok(())
    }


}

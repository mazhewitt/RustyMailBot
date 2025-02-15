use std::error::Error;
use std::sync::Arc;
use std::fs;
use std::path::Path;

use google_gmail1::Gmail;
use google_gmail1::oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use hyper::Client;
use hyper_rustls::HttpsConnectorBuilder;
use log::{error, info};
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use oauth2::{
    AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl,
};
use oauth2::basic::BasicClient;

const CONFIG_FILE: &str = "./data/client_secret.json";
const TOKEN_FILE: &str = "token.json";

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
    // This field is not used in our code but is part of the file format.
    #[allow(dead_code)]
    redirect_uris: Vec<String>,
}

/// A struct that encapsulates the authenticated Gmail client.
struct GmailClient {
    client: Arc<Mutex<Gmail>>,
}

impl GmailClient {
    /// Initializes the Gmail client.
    ///
    /// This builds a custom OAuth client with the Gmail read-only scope,
    /// then creates an InstalledFlowAuthenticator in interactive mode.
    pub async fn initialize() -> Result<Self, Box<dyn Error>> {
        info!("Initializing Gmail client...");

        info!("Reading application secrets from '{}'", CONFIG_FILE);
        let config_data =
            fs::read_to_string(CONFIG_FILE).expect("Failed to read client secret file");
        let config: OAuthConfig =
            serde_json::from_str(&config_data).expect("Invalid JSON format");

        info!("Creating OAuth client with custom scope...");
        let basic_client = BasicClient::new(
            ClientId::new(config.installed.client_id),
            Some(ClientSecret::new(config.installed.client_secret)),
            AuthUrl::new(config.installed.auth_uri).expect("Invalid auth URL"),
            Some(TokenUrl::new(config.installed.token_uri).expect("Invalid token URL")),
        )
            .set_redirect_uri(
                RedirectUrl::new("urn:ietf:wg:oauth:2.0:oob".to_string()).expect("Invalid redirect URI"),
            )
            .add_scope("https://www.googleapis.com/auth/gmail.readonly".to_string());

        info!("Setting up HTTPS client...");
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();
        let hyper_client = Client::builder().build::<_, hyper::Body>(https);

        info!("Creating OAuth authenticator (Interactive Mode)...");
        let auth = InstalledFlowAuthenticator::builder_with_client(
            basic_client,
            InstalledFlowReturnMethod::Interactive, // For CLI apps without a web redirect.
        )
            .persist_tokens_to_disk(TOKEN_FILE)
            .build()
            .await
            .map_err(|e| {
                error!("Failed to create authenticator: {}", e);
                e
            })?;

        info!("Gmail client initialized successfully.");
        let gmail = Gmail::new(hyper_client, auth);

        Ok(GmailClient {
            client: Arc::new(Mutex::new(gmail)),
        })
    }

    /// Retrieves and displays the subject and snippet of the latest emails in the inbox.
    pub async fn get_inbox(&self) -> Result<(), Box<dyn Error>> {
        info!("Fetching inbox messages...");

        let client_guard = self.client.lock().await;
        info!("Client lock acquired.");

        let list_call = client_guard.users().messages_list("me").q("in:inbox");
        info!("API call prepared: listing inbox messages...");

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
    use std::sync::Once;
    use log::info;
    use std::error::Error;

    static INIT: Once = Once::new();

    fn init_logger() {
        INIT.call_once(|| {
            // Configure env_logger for tests.
            env_logger::builder().is_test(true).init();
        });
    }

    /// This test checks that the initialization process creates a GmailClient.
    ///
    /// Since the interactive OAuth flow is triggered when no token exists,
    /// this test is marked #[ignore] to prevent it from running automatically.
    #[tokio::test]
    #[ignore]
    async fn test_initialize_creates_client_with_token() -> Result<(), Box<dyn Error>> {
        init_logger();
        if Path::new(TOKEN_FILE).exists() {
            fs::remove_file(TOKEN_FILE)?;
            info!("Existing token file removed for testing.");
        }
        let _gmail_client = GmailClient::initialize().await?;
        info!("Gmail client initialised successfully.");
        Ok(())
    }

    /// This test assumes a valid token file is present and tests the get_inbox method.
    #[tokio::test]
    #[ignore]
    async fn test_get_inbox() -> Result<(), Box<dyn Error>> {
        init_logger();
        let gmail_client = GmailClient::initialize().await?;
        gmail_client.get_inbox().await?;
        Ok(())
    }
}
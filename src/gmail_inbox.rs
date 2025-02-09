use std::error::Error;
use std::sync::Arc;

use google_gmail1::Gmail;
use google_gmail1::oauth2::{read_application_secret, InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use hyper::Client;
use hyper_rustls::{ HttpsConnectorBuilder};
use log::info;
use tokio::io::{stdin, AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

/// A struct that encapsulates the authenticated Gmail client.
struct GmailClient {
    /// The Gmail client is wrapped in an Arc and Mutex for safe concurrent access.
    client: Arc<Mutex<Gmail>>,
}

impl GmailClient {
    /// Logs in to Gmail using OAuth2 interactive flow.
    ///
    /// This function reads the client secret from `client_secret.json` and caches tokens to `tokencache.json`.
    async fn login() -> Result<Self, Box<dyn Error>> {
        // Start the login process.
        log::info!("Starting Gmail login process...");

        // Log reading client secret from file.
        log::info!("Reading client secret from file...");
        let secret = read_application_secret("./data/client_secret.json").await?;
        log::info!("Client secret loaded successfully.");

        // Prompt the user to proceed with interactive authentication.
        println!("Press Enter to begin interactive OAuth login...");
        let mut reader = BufReader::new(stdin());
        let mut input = String::new();
        reader.read_line(&mut input).await?;
        log::info!("User confirmed to start interactive authentication.");

        // Build the authenticator using an interactive flow.
        // Tokens are cached to "tokencache.json" so you won’t need to login every time.
        log::info!("Building interactive authenticator...");
        let auth = InstalledFlowAuthenticator::builder(
            secret,
            InstalledFlowReturnMethod::Interactive,
        )
            .persist_tokens_to_disk("tokencache.json")
            .build()
            .await?;
        log::info!("Authenticator built successfully.");

        // Create an HTTPS connector using the builder.
        log::info!("Building HTTPS connector...");
        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_or_http()
            .enable_http1()
            .build();
        log::info!("HTTPS connector built successfully.");

        // Build a hyper client using the new HTTPS connector.
        log::info!("Building hyper client...");
        let hyper_client = Client::builder().build::<_, hyper::Body>(https);
        log::info!("Hyper client built successfully.");

        // Instantiate the Gmail API client.
        log::info!("Instantiating Gmail API client...");
        let gmail = Gmail::new(hyper_client, auth);
        log::info!("Gmail API client instantiated successfully.");

        Ok(GmailClient {
            client: Arc::new(Mutex::new(gmail)),
        })
    }

    /// Retrieves and displays the subject and snippet of the latest emails in the inbox.
    ///
    /// It first lists the messages with the query "in:inbox" and then fetches each message in full.
    async fn get_inbox(&self) -> Result<(), Box<dyn Error>> {
        // Lock the client for safe asynchronous access.
        let mut client_guard = self.client.lock().await;

        // Create a call to list messages in the user's inbox.
        // "me" is a special value that indicates the authenticated user.
        let list_call = client_guard
            .users()
            .messages_list("me")
            .q("in:inbox"); // Gmail query to select inbox messages

        // Execute the API call.
        let list_response = list_call.doit().await?;

        // Check if any messages were returned.
        if let Some(messages) = list_response.1.messages {
            // Iterate through each message.
            for message in messages.iter() {
                if let Some(message_id) = &message.id {
                    // Retrieve the full message details (including headers and snippet).
                    let msg_response = client_guard
                        .users()
                        .messages_get("me", message_id)
                        .format("full")
                        .doit()
                        .await?;

                    let msg = msg_response.1;
                    let snippet = msg.snippet.unwrap_or_else(|| String::from("(No snippet available)"));

                    let subject = msg
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.headers.as_ref())
                        .and_then(|headers| {
                            headers.iter().find(|h| {
                                h.name
                                    .as_ref()
                                    .map_or(false, |name| name.to_lowercase() == "subject")
                            })
                        })
                        .map(|h| h.value.clone())
                        .unwrap_or_else(|| Option::from(String::from("(No Subject)")));

                    // Display the subject and snippet.
                    println!("Subject: {}\nSnippet: {}\n", subject.unwrap(), snippet);
                }
            }
        } else {
            println!("No messages found in the inbox.");
        }
        Ok(())
    }
}

#[tokio::test]
#[ignore] // Marked as ignored because this test requires real OAuth and network access.
async fn test_gmail_inbox_integration() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    info!("Application starting...");
    // The test will perform an interactive OAuth login if necessary.
    // Make sure you have the `client_secret.json` in place and that your token cache (tokencache.json) is writable.
    let gmail_client = GmailClient::login().await?;

    // Call get_inbox, which will print the email subjects and snippets.
    // For an integration test, we simply verify that the function returns Ok.
    gmail_client.get_inbox().await?;

    // If no error occurs, we assume the integration with Gmail works as expected.
    Ok(())
}


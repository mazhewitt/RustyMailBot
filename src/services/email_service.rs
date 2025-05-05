use log::{info, warn};
use crate::services::gmail_service;
use crate::models::email::Email;
use crate::models::email_db::EmailDB;

pub async fn load_emails() -> Result<Vec<Email>, Box<dyn std::error::Error>> {
    info!("Load email Handler Called...");
    
    // Initialize the email database
    info!("Initializing email database...");
    let email_db = EmailDB::default().await?;
    
    // Clear existing emails from the database
    info!("Clearing existing emails from database...");
    if let Err(e) = email_db.clear().await {
        warn!("Failed to clear email database: {}", e);
        // Continue even if clearing fails
    }
    
    // Fetch new emails from Gmail
    info!("Fetching emails from Gmail...");
    let emails = gmail_service::get_inbox_messages().await?;
    
    // Store the new emails in the database
    if !emails.is_empty() {
        info!("Storing {} new emails in database...", emails.len());
        if let Err(e) = email_db.store_emails(&emails).await {
            warn!("Failed to store emails in database: {}", e);
            // Continue even if storing fails
        }
    } else {
        info!("No emails retrieved from Gmail");
    }
    
    Ok(emails)
}

// Creates a new session manager instance.
pub fn create_session_manager() -> crate::models::global_session_manager::GlobalSessionManager {
    crate::models::global_session_manager::GlobalSessionManager::new()
}
use log::info;
use crate::services::gmail_service;
use ollama_rs::Ollama;
use crate::models::email::Email;

pub async fn load_emails(ollama: &mut Ollama) -> Result<Vec<Email>, Box<dyn std::error::Error>> {
    info!("Load email Handler Called...");
    let emails = gmail_service::get_inbox_messages().await?;
    Ok(emails)
}

// Creates a new session manager instance.
pub fn create_session_manager() -> crate::models::global_session_manager::GlobalSessionManager {
    crate::models::global_session_manager::GlobalSessionManager::new()
}
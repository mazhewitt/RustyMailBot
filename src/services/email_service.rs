use log::info;
use crate::models::vector_database::Document;
use crate::services::gmail_service;
use crate::services::embedding_service;
use ollama_rs::Ollama;

pub async fn load_emails(ollama: &mut Ollama) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let emails = gmail_service::get_inbox_messages().await?;
    let mut documents = Vec::new();

    for email in emails {
        let id = email.message_id.unwrap_or_default();
        let text = format!(
            "Message:{}\nFrom: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
            id,
            email.from.unwrap_or_default(),
            email.to.unwrap_or_default(),
            email.date.unwrap_or_default(),
            email.subject.unwrap_or_default(),
            email.body.unwrap_or_default()
        );
        info!("Generating embedding for email: {}", id);
        let embedding = embedding_service::fetch_embedding(ollama, &text).await?;
        documents.push(embedding);
    }
    Ok(documents)
}

// Creates a new session manager instance.
pub fn create_session_manager() -> crate::models::global_session_manager::GlobalSessionManager {
    crate::models::global_session_manager::GlobalSessionManager::new()
}
use crate::models::email_db::EmailDB;
use ollama_rs::generation::chat::ChatMessage;

#[derive(Clone)]
pub struct UserSession {
    pub history: Vec<ChatMessage>,
    pub mailbox: EmailDB,
}


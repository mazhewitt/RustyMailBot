use serde::{Serialize, Deserialize};
use crate::models::email_db::EmailDB;
use ollama_rs::generation::chat::ChatMessage;
use crate::config::SYSTEM_PROMPT;

#[derive(Clone)]
pub struct UserSession {
    pub history: Vec<ChatMessage>,
    pub mailbox: EmailDB,
}


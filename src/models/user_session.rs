use serde::{Serialize, Deserialize};
use crate::models::vector_database::VectorDatabase;
use ollama_rs::generation::chat::ChatMessage;
use crate::config::SYSTEM_PROMPT;

#[derive(Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub history: Vec<ChatMessage>,
    pub mailbox: VectorDatabase,
}

impl Default for UserSession {
    fn default() -> Self {
        Self {
            history: vec![ChatMessage::system(SYSTEM_PROMPT.to_string())],
            mailbox: VectorDatabase::new(),
        }
    }
}
use crate::models::user_session::UserSession;
use ollama_rs::Ollama;
use crate::config::SYSTEM_PROMPT;
use crate::services::embedding_service;
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};
use log::{info};

pub async fn process_chat(
    user_input: &str,
    user_session: &mut UserSession,
    mut ollama: Ollama
) -> Result<String, Box<dyn std::error::Error>> {
    // Refine the query using Ollama.
    let refined_query = embedding_service::refine_query(user_input, &mut ollama).await?;
    info!("Refined query: {}", refined_query);

    // Retrieve context from the mailbox.
    let context_str = user_session.mailbox.get_context(&refined_query, 2, &mut ollama).await?;
    info!("Retrieved context: {}", context_str);

    let conversation = vec![
        ChatMessage::system(SYSTEM_PROMPT.to_string()),
        ChatMessage::system(format!("Context from emails:\n{}", context_str)),
        ChatMessage::user(user_input.to_string()),
    ];

    let request = ChatMessageRequest::new(crate::config::MODEL_NAME.to_string(), conversation);
    let response = ollama.send_chat_messages_with_history(&mut user_session.history, request).await?;
    Ok(response.message.content)
}
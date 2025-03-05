use ollama_rs::Ollama;
use crate::config::MODEL_NAME;
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage};
use crate::config;

pub async fn refine_query(original_query: &str, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
    let refinement_prompt = format!(
        "Given the following instruction, extract the key details to search for relevant emails:\n\nInstruction: {}\n\nRefined Query:",
        original_query
    );
    let mut conversation = vec![
        ChatMessage::system("You are an expert at extracting key information from instructions.".to_string()),
        ChatMessage::user(refinement_prompt),
    ];
    let request = ChatMessageRequest::new(MODEL_NAME.to_string(), vec![]);
    let response = ollama.send_chat_messages_with_history(&mut conversation, request).await?;
    Ok(response.message.content.trim().to_string())
}

pub fn create_ollama() -> Ollama {
    Ollama::new(config::ollama_host(), config::ollama_port())
}


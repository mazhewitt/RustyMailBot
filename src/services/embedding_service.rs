use ollama_rs::Ollama;
use crate::models::vector_database::Document;
use crate::config::{EMBEDDING_MODEL, MODEL_NAME};
use ollama_rs::generation::embeddings::request::{GenerateEmbeddingsRequest, EmbeddingsInput};
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};

pub async fn fetch_embedding(ollama: &Ollama, text: &str) -> Result<Document, Box<dyn std::error::Error>> {
    let request = GenerateEmbeddingsRequest::new(
        EMBEDDING_MODEL.to_string(),
        EmbeddingsInput::Single(text.to_string()),
    );
    let res = ollama.generate_embeddings(request).await?;
    let embedding = res.embeddings.into_iter().next().ok_or("No embeddings returned")?;
    Ok(Document {
        text: text.to_string(),
        embedding,
    })
}

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
    Ollama::new("http://localhost", 11434)
}
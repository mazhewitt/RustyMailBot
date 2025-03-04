use crate::models::user_session::UserSession;
use crate::config::SYSTEM_PROMPT;
use crate::services::embedding_service;
use log::info;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};
use serde::{Deserialize, Serialize};
use crate::models::email::format_emails;

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Reply,
    Compose,
    Explain,
    General, // For queries that don't match the specific intents
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntentClassification {
    pub intent: String,
    pub confidence: f32,
    pub reasoning: String,
}

impl IntentClassification {
    pub fn get_intent(&self) -> Intent {
        match self.intent.as_str() {
            "reply" => Intent::Reply,
            "compose" => Intent::Compose,
            "explain" => Intent::Explain,
            _ => Intent::General,
        }
    }
}

/// Classifies the user's intent based on their input
pub async fn classify_intent(
    user_input: &str,
    ollama: &mut Ollama
) -> Result<IntentClassification, Box<dyn std::error::Error>> {
    // Define the prompt for intent classification
    let classification_prompt = format!(
        "You are an AI assistant that classifies user intent related to emails. Your task is to determine whether the user wants to:

(A) Reply to an email
(B) Compose a new email
(C) Explain an email

Based on the user input, respond in valid JSON format with the following structure:

{{
  \"intent\": \"reply\" | \"compose\" | \"explain\",
  \"confidence\": 0.0 - 1.0,
  \"reasoning\": \"Short explanation of why this classification was chosen.\"
}}

Ensure that:
- \"intent\" is one of \"reply\", \"compose\", or \"explain\".
- \"confidence\" is a number between 0 and 1, representing how sure you are about the classification.
- \"reasoning\" provides a concise justification for the classification.

Now, classify the following user input:

**User Input:** \"{}\"", user_input);

    // Create a conversation for the intent classification
    let conversation = vec![
        ChatMessage::system("You are a helpful assistant.".to_string()),
        ChatMessage::user(classification_prompt),
    ];

    // Send the request to the LLM
    let request = ChatMessageRequest::new(crate::config::MODEL_NAME.to_string(), conversation);
    let mut history = vec![];
    let response = ollama.send_chat_messages_with_history(&mut history, request).await?;

    // Parse the JSON response
    let json_str = response.message.content.trim();

    // Check if the response is wrapped in code blocks and extract the JSON
    let json_content = if json_str.contains("```json") && json_str.contains("```") {
        // Extract JSON between the markers
        let start = json_str.find("```json").unwrap_or(0) + 7;
        let end = json_str[start..].find("```").map_or(json_str.len(), |pos| start + pos);
        json_str[start..end].trim()
    } else {
        // Try to find the JSON object with curly braces
        let start = json_str.find('{').unwrap_or(0);
        let end = json_str[start..].rfind('}').map_or(json_str.len(), |pos| start + pos + 1);
        &json_str[start..end]
    };

    let classification: IntentClassification = serde_json::from_str(json_content)?;

    Ok(classification)
}

/// Process the chat based on the user's intent
pub async fn process_chat(
    user_input: &str,
    user_session: &mut UserSession,
    mut ollama: Ollama
) -> Result<String, Box<dyn std::error::Error>> {
    // Classify the user's intent
    let intent_classification = classify_intent(user_input, &mut ollama).await?;
    info!("Intent classification: {:?}", intent_classification);
    let intent = intent_classification.get_intent();

    // Refine the query using Ollama
    let refined_query = embedding_service::refine_query(user_input, &mut ollama).await?;
    info!("Refined query: {}", refined_query);

    // Retrieve context from the mailbox
    let context_emails = user_session.mailbox.search_emails(&refined_query).await?;

    // Handle the intent
    handle_intent(&intent, user_input, user_session, &mut ollama, &*format_emails(&context_emails)).await
}

/// Handle the different types of intents
async fn handle_intent(
    intent: &Intent,
    user_input: &str,
    user_session: &mut UserSession,
    ollama: &mut Ollama,
    context_str: &str
) -> Result<String, Box<dyn std::error::Error>> {
    let intent_prompt = match intent {
        Intent::Reply => "The user wants to reply to an email. Generate an appropriate response that they can send as a reply.",
        Intent::Compose => "The user wants to compose a new email. Help them draft a complete email with subject line and content.",
        Intent::Explain => "The user wants to understand an email better. Provide explanations, insights, and analysis of the email content.",
        Intent::General => "Answer the user's general question about their emails or provide assistance as needed.",
    };

    let conversation = vec![
        ChatMessage::system(SYSTEM_PROMPT.to_string()),
        ChatMessage::system(format!("Context from emails:\n{}", context_str)),
        ChatMessage::system(intent_prompt.to_string()),
        ChatMessage::user(user_input.to_string()),
    ];

    let request = ChatMessageRequest::new(crate::config::MODEL_NAME.to_string(), conversation);
    let response = ollama.send_chat_messages_with_history(&mut user_session.history, request).await?;
    Ok(response.message.content)
}
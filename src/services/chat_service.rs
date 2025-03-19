use crate::models::user_session::UserSession;
use crate::config::SYSTEM_PROMPT;
use crate::models::email_query;
use log::info;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};
use serde::{Deserialize, Serialize};
use crate::config;
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
pub async fn classify_intent(user_input: &str) -> Result<IntentClassification, Box<dyn std::error::Error>> {
    let mut ollama = config::create_ollama();
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
    user_session: &mut UserSession
) -> Result<String, Box<dyn std::error::Error>> {
    // Classify the user's intent
    let intent_classification = classify_intent(user_input).await?;
    info!("Intent classification: {:?}", intent_classification);
    let intent = intent_classification.get_intent();

    // Refine the query using the more complex refine_query function
    let refined_query = email_query::refine_query(user_input, None).await?;
    info!("Refined query: {:?}", refined_query);

    // Retrieve context from the mailbox
    let context_emails = user_session.mailbox.search_emails_by_criteria(refined_query).await?;

    // Handle the intent
    handle_intent(&intent, user_input, user_session, &*format_emails(&context_emails)).await
}

/// Handle the different types of intents
async fn handle_intent(
    intent: &Intent,
    user_input: &str,
    user_session: &mut UserSession,
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
    let mut ollama = config::create_ollama();
    let response = ollama.send_chat_messages_with_history(&mut user_session.history, request).await?;
    Ok(response.message.content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::email::Email;
    use crate::models::user_session::UserSession;
    use crate::models::email_db::EmailDB;
    use crate::services::chat_service::{classify_intent, process_chat};
    // Remove unused imports
    // use std::sync::Arc;
    // use Intent;

    // Create an empty user session for tests
    async fn create_empty_session() -> Result<UserSession, Box<dyn std::error::Error>> {
        let mail_db = EmailDB::default().await?;
        Ok(UserSession {
            history: Vec::new(),
            mailbox: mail_db,
        })
    }

    // Utility function to create a test session with sample emails
    async fn create_test_session() -> Result<UserSession, Box<dyn std::error::Error>> {
        let mut mail_db = EmailDB::default().await?;

        // Add some test emails
        let sample_emails = vec![
            Email {
                from: Some("alice@example.com".to_string()),
                to: Some("user@example.com".to_string()),
                subject: Some("Meeting tomorrow".to_string()),
                body: Some("Hi, can we meet tomorrow to discuss the project? Thanks, Alice".to_string()),
                date: Some("2023-06-01T10:00:00Z".to_string()),
                message_id: Some("msg_1".to_string()),
            },
            Email {
                from: Some("bob@example.com".to_string()),
                to: Some("user@example.com".to_string()),
                subject: Some("Urgent: Report submission".to_string()),
                body: Some("Hi, I need the quarterly report by end of day. It's urgent! Thanks, Bob".to_string()),
                date: Some("2023-06-02T15:30:00Z".to_string()),
                message_id: Some("msg_2".to_string()),
            },
        ];

        mail_db.store_emails(&sample_emails).await?;

        Ok(UserSession {
            history: Vec::new(),
            mailbox: mail_db,
        })
    }

    #[tokio::test]
    async fn test_classify_intent_reply() {
        let result = classify_intent("Can you help me reply to Alice about the meeting?").await;
        assert!(result.is_ok(), "Intent classification failed");
        let classification = result.unwrap();
        assert_eq!(classification.intent, "reply");
        assert!(classification.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_classify_intent_compose() {
        let result = classify_intent("I need to write an email to the team about the delay").await;
        assert!(result.is_ok(), "Intent classification failed");
        let classification = result.unwrap();
        assert_eq!(classification.intent, "compose");
        assert!(classification.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_classify_intent_explain() {
        let result = classify_intent("What does Bob mean by urgent in his email?").await;
        assert!(result.is_ok(), "Intent classification failed");
        let classification = result.unwrap();
        assert_eq!(classification.intent, "explain");
        assert!(classification.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_process_chat_with_email_context() {
        let session = create_test_session().await;
        assert!(session.is_ok(), "Failed to create test session");
        let mut session = session.unwrap();

        let result = process_chat("Help me understand Bob's email about the report", &mut session).await;
        assert!(result.is_ok(), "Failed to process chat");
        let response = result.unwrap();
        assert!(!response.is_empty());
        // The response should mention something about the report or Bob
        assert!(response.to_lowercase().contains("bob") ||
            response.to_lowercase().contains("report") ||
            response.to_lowercase().contains("urgent"));
    }

    #[tokio::test]
    async fn test_process_chat_reply_intent() {
        let session = create_test_session().await;
        assert!(session.is_ok(), "Failed to create test session");
        let mut session = session.unwrap();

        let result = process_chat("Draft a reply to Alice about the meeting", &mut session).await;
        assert!(result.is_ok(), "Failed to process chat for reply intent");
        let response = result.unwrap();
        assert!(!response.is_empty());
        // Response should look like an email reply
        assert!(response.to_lowercase().contains("alice") ||
            response.to_lowercase().contains("meeting") ||
            response.to_lowercase().contains("tomorrow"));
    }

    #[tokio::test]
    async fn test_process_chat_without_relevant_context() {
        let session = create_test_session().await;
        assert!(session.is_ok(), "Failed to create test session");
        let mut session = session.unwrap();

        let result = process_chat("Tell me about emails from Charlie", &mut session).await;
        assert!(result.is_ok(), "Failed to process chat for irrelevant query");
        let response = result.unwrap();
        assert!(!response.is_empty());
        // The response should indicate no relevant emails were found
        assert!(response.to_lowercase().contains("no") ||
            response.to_lowercase().contains("not found") ||
            response.to_lowercase().contains("don't have") ||
            response.to_lowercase().contains("couldn't find"));
    }


}
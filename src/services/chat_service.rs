use crate::models::user_session::UserSession;
use crate::config::SYSTEM_PROMPT;
use log::info;
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};
use serde::{Deserialize, Serialize};
use crate::config;
use crate::models::email::format_emails;
use crate::services::llm_service;

#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Reply,
    Compose,
    Explain,
    List,    // New intent for listing emails
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
            "list" => Intent::List,
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
(D) List emails in the inbox

Based on the user input, respond in valid JSON format with the following structure:

{{
  \"intent\": \"reply\" | \"compose\" | \"explain\" | \"list\",
  \"confidence\": 0.0 - 1.0,
  \"reasoning\": \"Short explanation of why this classification was chosen.\"
}}

Ensure that:
- \"intent\" is one of \"reply\", \"compose\", \"explain\", or \"list\".
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
    // Classify the user's intent first
    let intent_classification = classify_intent(user_input).await?;
    info!("Intent classification: {:?}", intent_classification);
    let intent = intent_classification.get_intent();

    // Special case for List intent
    if let Intent::List = intent {
        info!("Processing List intent");

        // Check for a generic 'from <sender>' filter
        let input_lower = user_input.to_lowercase();
        if let Some(pos) = input_lower.find("from ") {
            // Extract the sender token immediately after "from "
            let after = &input_lower[pos + 5..];
            let sender = after.split_whitespace().next().unwrap_or("");
            info!("Filtering for emails from {}", sender);
            let emails = user_session.mailbox.search_emails(sender).await?;
            if emails.is_empty() {
                return Ok("No emails found matching your criteria.".to_string());
            }

            // Format and return summary for filtered results
            let mut summary = String::new();
            summary.push_str("Here's a summary of emails in your inbox:\n\n");
            for (i, email) in emails.iter().enumerate() {
                summary.push_str(&format!("{}. From: {} | Subject: {} | Date: {}\n",
                    i + 1,
                    email.from.as_deref().unwrap_or("Unknown"),
                    email.subject.as_deref().unwrap_or("No Subject"),
                    email.date.as_deref().unwrap_or("Unknown")
                ));
            }
            return Ok(summary);
        }

        // No specific sender filter: list all emails
        info!("Getting all emails");
        let mut emails = user_session.mailbox.search_emails("").await?;
        if emails.is_empty() {
            return Ok("No emails found matching your criteria.".to_string());
        }

        // Format and return summary for all emails
        let mut summary = String::new();
        summary.push_str("Here's a summary of emails in your inbox:\n\n");
        for (i, email) in emails.iter().enumerate() {
            summary.push_str(&format!("{}. From: {} | Subject: {} | Date: {}\n",
                i + 1,
                email.from.as_deref().unwrap_or("Unknown"),
                email.subject.as_deref().unwrap_or("No Subject"),
                email.date.as_deref().unwrap_or("Unknown")
            ));
        }
        return Ok(summary);
    }

    // Handle email retrieval differently based on intent
    let context_emails = match intent {
            Intent::Reply => {
                // For replies, we need to find a specific email
                let refined_query = llm_service::refine_query(user_input, Intent::Reply).await?;
                info!("Refined query for reply: {:?}", refined_query);
                let emails = user_session.mailbox.search_emails_by_criteria(refined_query).await?;

                // If we couldn't find a specific email to reply to, ask for clarification
                if emails.is_empty() {
                    return Ok("I couldn't find the specific email you want to reply to. Could you provide more details about the email, like who sent it or what it was about?".to_string());
                }
                emails
            },
            Intent::Compose => {
                // For compose, we might want related emails as context but don't require them
                let refined_query = llm_service::refine_query(user_input, Intent::Compose).await?;
                info!("Refined query for compose: {:?}", refined_query);
                user_session.mailbox.search_emails_by_criteria(refined_query).await?
                // Empty results are fine for compose
            },
            Intent::Explain => {
                // For explain, we need to find the specific email(s) to explain
                let refined_query = llm_service::refine_query(user_input, Intent::Explain).await?;
                info!("Refined query for explain: {:?}", refined_query);
                let emails = user_session.mailbox.search_emails_by_criteria(refined_query).await?;

                // If we couldn't find a specific email to explain, ask for clarification
                if emails.is_empty() {
                    return Ok("I couldn't find the specific email you want me to explain. Could you provide more details about the email, like who sent it or what it was about?".to_string());
                }
                emails
            },
            Intent::List => {
                // This code won't actually be reached since we handle the List intent earlier
                // But we need this to make the match exhaustive
                vec![]
            },
            Intent::General => {
                // For general queries, do a broad search
                let refined_query = llm_service::refine_query(user_input, Intent::General).await?;
                info!("Refined query for general query: {:?}", refined_query);
                let emails = user_session.mailbox.search_emails_by_criteria(refined_query).await?;
                // If no relevant emails, indicate none found
                if emails.is_empty() {
                    return Ok("No emails found matching your criteria.".to_string());
                }
                emails
            }
    };

    // Format emails for context
    let context_str = format_emails(&context_emails);

    // Handle the intent with the appropriate context
    handle_intent(&intent, user_input, user_session, &context_str).await
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
        Intent::List => "The user wants to list emails in their inbox. Provide a summary of their emails.",
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
    use crate::models::email::Email;
    use crate::models::user_session::UserSession;
    use crate::models::email_db::EmailDBError;
    use crate::services::chat_service::classify_intent;
    use mockall::predicate::*;
    use mockall::mock;

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
    async fn test_classify_intent_list() {
        let result = classify_intent("Show me all emails in my inbox").await;
        assert!(result.is_ok(), "Intent classification failed");
        let classification = result.unwrap();
        assert_eq!(classification.intent, "list");
        assert!(classification.confidence > 0.5);
        
        // Test another common list request phrasing
        let result2 = classify_intent("List my recent emails").await;
        assert!(result2.is_ok(), "Intent classification failed for second query");
        let classification2 = result2.unwrap();
        assert_eq!(classification2.intent, "list");
        assert!(classification2.confidence > 0.5);
    }
}
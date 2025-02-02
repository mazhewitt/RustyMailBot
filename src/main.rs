use reqwest::Client;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use std::io::{self, Write};
use std::sync::Mutex;


const API_URL: &str = "http://localhost:11434/api/chat";
const MODEL_NAME: &str = "llama3.2";
const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";

#[derive(Serialize, Clone)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    model: String,
    created_at: String,
    message: MessageResponse,
    done_reason: Option<String>,
    done: bool,
}

#[derive(Deserialize)]
struct MessageResponse {
    role: String,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    // Create and initialize conversation history with a system prompt.
    let mut conversation: Vec<Message> = vec![
        Message {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        },
    ];

    println!("Welcome to the Rust Chatbot. Type your message below (or 'exit' to quit):");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        if user_input.is_empty() {
            continue;
        }
        if user_input.eq_ignore_ascii_case("exit") || user_input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }

        // Append the user's message to the conversation history.
        conversation.push(Message {
            role: "user".to_string(),
            content: user_input.to_string(),
        });

        // Create the request payload using the full conversation.
        let request_body = ChatRequest {
            model: MODEL_NAME.to_string(),
            messages: conversation.clone(), // Clone the history to send in the request.
        };

        let response = client
            .post(API_URL)
            .json(&request_body)
            .send()
            .await?;

        // Process the streaming response.
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut assistant_message = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = std::str::from_utf8(&chunk)?;
            buffer.push_str(chunk_str);

            // Process complete lines in the buffer.
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer.drain(..pos + 1);

                if !line.trim().is_empty() {
                    let chat_response: ChatResponse = serde_json::from_str(&line)?;
                    print!("{}", chat_response.message.content);
                    io::stdout().flush()?;
                    assistant_message.push_str(&chat_response.message.content);
                }
            }
        }
        println!(); // Newline after the assistant's reply.

        // Append the assistant's full reply to the conversation history.
        conversation.push(Message {
            role: "assistant".to_string(),
            content: assistant_message,
        });
    }

    Ok(())
}

fn load_emails(folder: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut emails = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let content = std::fs::read_to_string(path)?;
            emails.push(content);
        }
    }
    Ok(emails)
}

#[derive(Clone)]
pub struct Document {
    pub text: String,
    pub embedding: Vec<f32>,
}

pub struct VectorDatabase {
    pub documents: Vec<Document>,
}

impl VectorDatabase {
    pub fn new() -> Self {
        Self { documents: Vec::new() }
    }

    /// Insert a document into the database.
    pub fn insert(&mut self, text: String) {
        let embedding = real_embedding(&text);
        self.documents.push(Document { text, embedding });
    }

    /// Search for the top_n most similar documents to the query.
    pub fn search(&self, query: &str, top_n: usize) -> Vec<&Document> {
        let query_embedding = real_embedding(query);
        let mut results: Vec<(&Document, f32)> = self.documents.iter()
            .map(|doc| (doc, cosine_similarity(&doc.embedding, &query_embedding)))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.into_iter().take(top_n).map(|(doc, _)| doc).collect()
    }

    pub fn get_context(&self, query: &str, top_n: usize) -> String {
            let results = self.search(query, top_n);
            let context: Vec<String> = results.iter().map(|doc| doc.text.clone()).collect();
            context.join("\n---\n")
    }

}

use once_cell::sync::Lazy;
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder,
    SentenceEmbeddingsModelType,
    SentenceEmbeddingsModel,
};

// Create and cache the model once.
static EMBEDDINGS_MODEL: Lazy<Mutex<SentenceEmbeddingsModel>> = Lazy::new(|| {
    Mutex::new(
        SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL12V2)
            .create_model()
            .expect("Failed to load sentence embeddings model")
    )
});

fn real_embedding(text: &str) -> Vec<f32> {
    let model = EMBEDDINGS_MODEL.lock().expect("Failed to lock the embeddings model");
    let embeddings = model
        .encode(&[text.to_owned()])
        .expect("Failed to encode text");
    embeddings.into_iter().next().unwrap()
}

/// A simple cosine similarity implementation.
fn cosine_similarity(a: &Vec<f32>, b: &Vec<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

async fn refine_query(original_query: &str) -> Result<String, Box<dyn std::error::Error>> {
    let refinement_prompt = format!(
        "Given the following instruction, extract the key details to search for relevant emails:\n\nInstruction: {}\n\nRefined Query:",
        original_query
    );

    // Build a conversation specifically for query refinement.
    let conversation = vec![
        Message {
            role: "system".to_string(),
            content: "You are an expert at extracting key information from instructions.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: refinement_prompt,
        },
    ];

    let request_body = ChatRequest {
        model: MODEL_NAME.to_string(),
        messages: conversation,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(API_URL)
        .json(&request_body)
        .send()
        .await?;

    // For simplicity, assume the model responds in one shot (or adjust if streaming).
    let response_text = response.text().await?;

    // Extract the refined query from the response.
    // You might need more sophisticated parsing if the response is complex.
    Ok(response_text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_database_load_and_search() {
        // Use the dummy inbox folder (suggested location)
        let folder = "data/dummy_inbox";
        let emails = load_emails(folder).expect("Failed to load emails");
        // Ensure we have 5 emails.
        assert_eq!(emails.len(), 5, "Expected 5 emails in the dummy inbox");

        let mut db = VectorDatabase::new();
        for email in emails {
            db.insert(email);
        }
        assert_eq!(db.documents.len(), 5, "Expected 5 documents in the database");

        // Test searching with a dummy query.
        let results = db.search("important", 3);
        // Since it's a dummy embedding, we don't know the ranking, but we expect at most 3 results.
        assert!(results.len() <= 3, "Expected at most 3 results");
    }
    #[test]
    fn test_get_context() {
        let mut db = VectorDatabase::new();
        db.insert("Email about meeting schedule and agenda.".to_string());
        db.insert("Email regarding project deadlines and updates.".to_string());
        db.insert("Email discussing the upcoming team meeting.".to_string());
        db.insert("Email about vacation plans and holiday schedule.".to_string());
        db.insert("Email regarding budget report and financial review.".to_string());

        let context = db.get_context("meeting", 3);
        // We expect that the context contains at least one occurrence of "meeting"
        assert!(!context.is_empty(), "Context should not be empty");
        assert!(context.to_lowercase().contains("meeting"), "Context should mention 'meeting'");
    }
}

#[tokio::test]
async fn test_reply_to_dummy_email() -> Result<(), Box<dyn std::error::Error>> {
    // Force the embeddings model to initialize on a blocking thread.
    tokio::task::spawn_blocking(|| {
        // Accessing the lazy static forces initialization.
        let _ = &*EMBEDDINGS_MODEL;
    })
        .await?;
    // Load emails from the dummy inbox folder.
    let emails = load_emails("data/dummy_inbox")?;
    assert!(!emails.is_empty(), "Dummy inbox should not be empty");

    // Create a vector database and insert each email.
    let mut vector_db = VectorDatabase::new();
    for email in emails {
        vector_db.insert(email);
    }

    // Define the dummy email (Email 1 – Simple Greeting).
    let email_1 = "Email 1 – Simple Greeting
From: alice@example.com
To: bob@example.com
Subject: Test Email 1: Hello from Alice

Hi Bob,

Just a quick note to test out our new email chatbot system. I hope you’re having a good day.

Cheers,
Alice";

    // Retrieve context from the vector DB for this email.
    let context = vector_db.get_context(email_1, 3);

    // Build the conversation prompt.
    let conversation = vec![
        Message {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        },
        Message {
            role: "system".to_string(),
            content: format!("Context from emails:\n{}", context),
        },
        Message {
            role: "user".to_string(),
            content: format!("Please write a reply to the following email:\n\n{}", email_1),
        },
    ];

    let request_body = ChatRequest {
        model: MODEL_NAME.to_string(),
        messages: conversation,
    };

    // Send the chat request to the API.
    let client = reqwest::Client::new();
    let response = client
        .post(API_URL)
        .json(&request_body)
        .send()
        .await?;

    let response_text = response.text().await?;
    // Assert that the chatbot's reply is not empty.
    assert!(
        !response_text.trim().is_empty(),
        "The chatbot reply should not be empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_refined_query_retrieval() -> Result<(), Box<dyn std::error::Error>> {

    tokio::task::spawn_blocking(|| {
        // Accessing the lazy static forces initialization.
        let _ = &*EMBEDDINGS_MODEL;
    }).await?;
    // Example user input:
    let user_input = "Compose a reply to Alice saying that the chatbot system is very good";

    // First, refine the query.
    let refined_query = refine_query(user_input).await?;
    println!("Refined Query: {}", refined_query);


    // Assume you have an instance of your VectorDatabase with emails already inserted.
    let mut vector_db = VectorDatabase::new();
    // For the sake of testing, insert some dummy documents.
    vector_db.insert("Email from Alice: Hi Bob, just checking in.".to_string());
    vector_db.insert("Email from Carol: Meeting tomorrow.".to_string());
    vector_db.insert("Email from Alice: System feedback, everything is working well.".to_string());

    // First, filter by sender.
    let context_str = vector_db.get_context(&refined_query,2);

    println!("Retrieved Context:\n{}", context_str);
    // You can assert that the refined query is non-empty and the context contains relevant info.
    assert!(!refined_query.is_empty());
    assert!(context_str.to_lowercase().contains("alice"));

    Ok(())
}
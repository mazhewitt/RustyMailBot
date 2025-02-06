use std::io::{self, Write};
use tokio;

use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::embeddings::request::{GenerateEmbeddingsRequest, EmbeddingsInput};

const MODEL_NAME: &str = "llama3.2";
const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a mutable Ollama client.
    let mut ollama = Ollama::default();

    // load inbox
    let emails = load_emails("./data/dummy_inbox").expect("Cannot load emails");
    let mut vector_db = VectorDatabase::new();
    for email in emails {
        vector_db.insert(email, &mut ollama).await?;
    }
    // Start the conversation history with the system prompt.
    let mut history = vec![ChatMessage::system(SYSTEM_PROMPT.to_string())];

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

        let refined_query = refine_query(user_input, &mut ollama).await?;
        println!("Refined Query: {}", refined_query);


        let context_str = vector_db.get_context(&refined_query, 2, &mut ollama).await?;
        println!("Retrieved Context:\n{}", context_str);

        let mut conversation = vec![
            ChatMessage::system(SYSTEM_PROMPT.to_string()),
            ChatMessage::system(format!("Context from emails:\n{}", context_str)),
            ChatMessage::user(user_input.to_string()),
        ];

        // Create a chat request with the user's message.
        let request = ChatMessageRequest::new(
            MODEL_NAME.to_string(),
            conversation,
        );

        // Send the chat messages with the current history.
        let response = ollama.send_chat_messages_with_history(&mut history, request).await?;
        println!("{}", response.message.content);
    }

    Ok(())
}

/// Loads all emails (as plain text) from the specified folder.
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

/// A document stored in the vector database.
#[derive(Clone)]
pub struct Document {
    pub text: String,
    pub embedding: Vec<f32>,
}

/// A simple vector database that indexes documents by their embeddings.
/// (Now using the Ollama embeddings API rather than rust‑bert.)
pub struct VectorDatabase {
    pub documents: Vec<Document>,
}

impl VectorDatabase {
    pub fn new() -> Self {
        Self { documents: Vec::new() }
    }

    /// Insert a document into the database using the Ollama embeddings API.
    pub async fn insert(&mut self, text: String, ollama: &mut Ollama) -> Result<(), Box<dyn std::error::Error>> {
        let embedding = real_embedding(&text, ollama).await?;
        self.documents.push(Document { text, embedding });
        Ok(())
    }

    /// Search for the top_n most similar documents to the query.
    pub async fn search(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<Vec<&Document>, Box<dyn std::error::Error>> {
        let query_embedding = real_embedding(query, ollama).await?;
        let mut results: Vec<(&Document, f32)> = self.documents.iter()
            .map(|doc| (doc, cosine_similarity(&doc.embedding, &query_embedding)))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(results.into_iter().take(top_n).map(|(doc, _)| doc).collect())
    }

    /// Return the concatenated text of the top_n most similar documents.
    pub async fn get_context(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
        let results = self.search(query, top_n, ollama).await?;
        let context: Vec<String> = results.iter().map(|doc| doc.text.clone()).collect();
        Ok(context.join("\n---\n"))
    }
}

/// Generate an embedding for the provided text using the Ollama embeddings API.
async fn real_embedding(text: &str, ollama: &mut Ollama) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let request = GenerateEmbeddingsRequest::new(
        MODEL_NAME.to_string(),
        EmbeddingsInput::Single(text.to_string()),
    );
    let res = ollama.generate_embeddings(request).await?;
    // Extract the first embedding from the returned vector.
    let embedding = res.embeddings.into_iter().next().ok_or("No embeddings returned")?;
    Ok(embedding)
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

/// Refine a query using the Ollama chat API.
/// Given an original instruction, this function asks the model to extract key details.
async fn refine_query(original_query: &str, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
    let refinement_prompt = format!(
        "Given the following instruction, extract the key details to search for relevant emails:\n\nInstruction: {}\n\nRefined Query:",
        original_query
    );

    let mut conversation = vec![
        ChatMessage::system("You are an expert at extracting key information from instructions.".to_string()),
        ChatMessage::user(refinement_prompt),
    ];

    // Create a chat request; note that no extra user messages are needed since the conversation history has them.
    let request = ChatMessageRequest::new(MODEL_NAME.to_string(), vec![]);
    let response = ollama.send_chat_messages_with_history(&mut conversation, request).await?;
    Ok(response.message.content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    // Test that loading emails and searching via the embeddings API works as expected.
    #[tokio::test]
    async fn test_vector_database_load_and_search() -> Result<(), Box<dyn std::error::Error>> {
        let mut ollama = Ollama::default();
        let folder = "data/dummy_inbox";
        let emails = load_emails(folder)?;
        let mut db = VectorDatabase::new();
        for email in emails {
            db.insert(email, &mut ollama).await?;
        }
        assert_eq!(db.documents.len(), 5, "Expected 5 documents in the database");

        // Test searching with a dummy query.
        let results = db.search("important", 3, &mut ollama).await?;
        // Since embeddings are produced by the model, we expect at most 3 results.
        assert!(results.len() <= 3, "Expected at most 3 results");
        Ok(())
    }

    // Test that the context (concatenated emails) returned for a query contains relevant keywords.
    #[tokio::test]
    async fn test_get_context() -> Result<(), Box<dyn std::error::Error>> {
        let mut ollama = Ollama::default();
        let mut db = VectorDatabase::new();
        db.insert("Email about meeting schedule and agenda.".to_string(), &mut ollama).await?;
        db.insert("Email regarding project deadlines and updates.".to_string(), &mut ollama).await?;
        db.insert("Email discussing the upcoming team meeting.".to_string(), &mut ollama).await?;
        db.insert("Email about vacation plans and holiday schedule.".to_string(), &mut ollama).await?;
        db.insert("Email regarding budget report and financial review.".to_string(), &mut ollama).await?;

        let context = db.get_context("meeting", 3, &mut ollama).await?;
        assert!(!context.is_empty(), "Context should not be empty");
        println!("Context: {}", context);
        assert!(context.to_lowercase().contains("meeting"), "Context should mention 'meeting'");
        Ok(())
    }

    // Test that a reply generated for a dummy email is not empty.
    #[tokio::test]
    async fn test_reply_to_dummy_email() -> Result<(), Box<dyn std::error::Error>> {
        let mut ollama = Ollama::default();

        // Load emails from the dummy inbox folder.
        let emails = load_emails("data/dummy_inbox")?;
        assert!(!emails.is_empty(), "Dummy inbox should not be empty");

        let mut vector_db = VectorDatabase::new();
        for email in emails {
            vector_db.insert(email, &mut ollama).await?;
        }

        // Define a dummy email.
        let email_1 = "Email 1 – Simple Greeting
From: alice@example.com
To: bob@example.com
Subject: Test Email 1: Hello from Alice

Hi Bob,

Just a quick note to test out our new email chatbot system. I hope you’re having a good day.

Cheers,
Alice";

        // Retrieve context from the vector database.
        let context = vector_db.get_context(email_1, 3, &mut ollama).await?;

        // Build the conversation using system and user messages.
        let mut conversation = vec![
            ChatMessage::system(SYSTEM_PROMPT.to_string()),
            ChatMessage::system(format!("Context from emails:\n{}", context)),
            ChatMessage::user(format!("Please write a reply to the following email:\n\n{}", email_1)),
        ];

        let response = ollama.send_chat_messages_with_history(&mut conversation,
                                                              ChatMessageRequest::new(MODEL_NAME.to_string(), vec![])
        ).await?;
        let reply = response.message.content;
        assert!(!reply.trim().is_empty(), "The chatbot reply should not be empty");
        Ok(())
    }

    // Test that the query refinement function returns a refined query and that using it to retrieve context finds a mention of "alice".
    #[tokio::test]
    async fn test_refined_query_retrieval() -> Result<(), Box<dyn std::error::Error>> {
        let mut ollama = Ollama::default();

        let mut vector_db = VectorDatabase::new();
        vector_db.insert("Email from Alice: Hi Bob, just checking in.".to_string(), &mut ollama).await?;
        vector_db.insert("Email from Carol: Meeting tomorrow.".to_string(), &mut ollama).await?;
        vector_db.insert("Email from Alice: System feedback, everything is working well.".to_string(), &mut ollama).await?;


        let user_input = "Compose a reply to Alice saying that the chatbot system is very good";

        let refined_query = refine_query(user_input, &mut ollama).await?;
        println!("Refined Query: {}", refined_query);


        let context_str = vector_db.get_context(&refined_query, 2, &mut ollama).await?;
        println!("Retrieved Context:\n{}", context_str);

        assert!(!refined_query.is_empty());
        assert!(context_str.to_lowercase().contains("alice"));
        Ok(())
    }
}
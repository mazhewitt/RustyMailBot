mod gmail;

use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Error};
use actix_web::middleware::Logger;
use async_stream::stream;
use bytes::Bytes;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use log::{info};
use gmail::{check_auth, oauth_login, oauth_callback, get_inbox_messages};

use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::embeddings::request::{GenerateEmbeddingsRequest, EmbeddingsInput};


const MODEL_NAME: &str = "llama3.2";
const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";

/// This endpoint streams a greeting unless the client sends "@list",
/// in which case it streams Gmail inbox messages.
async fn stream_greeting(req_body: web::Json<Value>) -> HttpResponse {

    let mut ollama = Ollama::default();

    // load inbox

    let mut vector_db = VectorDatabase::new();

    let mut history = vec![ChatMessage::system(SYSTEM_PROMPT.to_string())];


    info!("Received request with payload: {:?}", req_body);

    let user_input = req_body.get("name")
        .and_then(Value::as_str)
        .unwrap_or("world")
        .to_string();

    if user_input.trim() == "@list" {
        info!("Streaming inbox messages as requested with @list");
        let inbox_stream = stream! {
            match get_inbox_messages().await {
                Ok(messages) => {
                    for message in messages {
                        if let Ok(json_message) = serde_json::to_string(&message) {
                            yield Ok(Bytes::from(json_message + "\n"));
                        } else {
                            yield Err(actix_web::error::ErrorInternalServerError("Serialization error"));
                        }
                    }
                },
                Err(e) => yield Err(actix_web::error::ErrorInternalServerError(e)),
            }
        };

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

        return HttpResponse::Ok()
            .content_type("application/json")
            .streaming(inbox_stream);
    }

    let greeting_stream = stream! {
        info!("Streaming chunk: 'hello '");
        yield Ok::<_, Error>(Bytes::from("hello "));
        sleep(Duration::from_millis(500)).await;
        info!("Streaming chunk: '{}'", user_input);
        yield Ok(Bytes::from(user_input));
    };

    HttpResponse::Ok()
        .content_type("text/plain")
        .streaming(greeting_stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    info!("Starting server on http://127.0.0.1:8080");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            // Use the Gmail module’s endpoints:
            .route("/stream", web::post().to(stream_greeting))
            .route("/check_auth", web::get().to(check_auth))
            .route("/oauth/login", web::get().to(oauth_login))
            .route("/oauth/callback", web::get().to(oauth_callback))
            // Serve static files (including index.html) from the "./static" directory.
            .service(Files::new("/", "./static").index_file("index.html"))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

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
// --- Import necessary crates and modules ---
mod gmail;
mod memory_session_store;

use std::str::from_boxed_utf8_unchecked;
use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer, Error};
use actix_web::middleware::Logger;
use async_stream::stream;
use bytes::Bytes;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use log::info;
use gmail::{check_auth, oauth_login, oauth_callback, get_inbox_messages};

use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::embeddings::request::{GenerateEmbeddingsRequest, EmbeddingsInput};

use actix_session::{SessionMiddleware, Session};
use actix_web::cookie::Key;
use serde::{Deserialize, Serialize};
use crate::memory_session_store::MemorySessionStore;

struct AppState {
    ollama: Ollama,
}

#[derive(Deserialize, Serialize)]
struct UserSession {
    history: Vec<ChatMessage>,
    mailbox: VectorDatabase,
}

impl Default for UserSession {
    fn default() -> Self {
        UserSession {
            history: vec![ChatMessage::system(SYSTEM_PROMPT.to_string())],
            mailbox: VectorDatabase::new(),
        }
    }
}

const MODEL_NAME: &str = "llama3.2";

const EMBEDDING_MODEL: &str = "all-minilm";
const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";

/// New endpoint to initialize the user session and load the inbox into the vector database.


async fn init_session_endpoint(data: web::Data<AppState>, session: Session) -> HttpResponse {
    if let Ok(Some(_)) = session.get::<UserSession>("user_session") {
        return HttpResponse::Ok().json(serde_json::json!({ "initialized": true }));
    }

    let mut new_session = UserSession::default();
    let mut ollama_instance = data.ollama.clone();
    info!("Loading emails into vector database");

    // Set a timeout of, say, 60 seconds
    let load_result = tokio::time::timeout(tokio::time::Duration::from_secs(300), load_emails(&mut ollama_instance)).await;
    match load_result {
        Ok(result) => match result {
            Ok(documents) => new_session.mailbox.documents = documents,
            Err(e) => {
                log::error!("Error loading emails: {:?}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to load emails",
                    "details": e.to_string()
                }));
            }
        },
        Err(e) => {
            log::error!("Timed out loading emails: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Loading emails timed out"
            }));
        }
    }

    if let Err(e) = session.insert("user_session", &new_session) {
        log::error!("Failed to save session: {:?}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to save session",
            "details": e.to_string()
        }));
    }
    info!("Initialized user session");
    HttpResponse::Ok().json(serde_json::json!({ "initialized": true }))
}

/// Updated load_emails function that populates the vector database with real embeddings.
pub async fn load_emails(ollama: &mut Ollama) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let emails = get_inbox_messages().await?;
    let mut documents = Vec::new();

    for email in emails {
        let id = email.message_id.unwrap_or_default().clone();
        let text = format!(
            "Message:{}\nFrom: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
            id.clone(),
            email.from.unwrap_or_default(),
            email.to.unwrap_or_default(),
            email.date.unwrap_or_default(),
            email.subject.unwrap_or_default(),
            email.body.unwrap_or_default()
        );
        info!("Getting embeddings for email: {}", id);
        let embedding = fetch_embedding(ollama, text.as_str()).await?;
        documents.push(embedding);
    }
    Ok(documents)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    info!("Starting server on http://127.0.0.1:8080");

    let ollama = Ollama::new("http://localhost", 11434);
    let secret_key: [u8; 64] = *b"0123456789012345678901234567890123456789012345678901234567890123";
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState { ollama: ollama.clone() }))
            .wrap(Logger::default())
            .wrap(SessionMiddleware::new(
                MemorySessionStore::new(),
                Key::try_from(&secret_key[..]).unwrap(),
            ))
            .route("/init_session", web::get().to(init_session_endpoint))
            .route("/stream", web::post().to(stream_greeting))
            .route("/check_auth", web::get().to(check_auth))
            .route("/oauth/login", web::get().to(oauth_login))
            .route("/oauth/callback", web::get().to(oauth_callback))
            .service(Files::new("/", "./static").index_file("index.html"))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub text: String,
    pub embedding: Vec<f32>,
}

/// A simple vector database that indexes documents by their embeddings.
#[derive(Clone, Serialize, Deserialize)]
pub struct VectorDatabase {
    pub documents: Vec<Document>,
}

impl VectorDatabase {
    pub fn new() -> Self {
        Self { documents: Vec::new() }
    }

    /// Insert a document into the database using the Ollama embeddings API.
    pub async fn insert(&mut self, text: String, ollama: &mut Ollama) -> Result<(), Box<dyn std::error::Error>> {
        let embedding = fetch_embedding( ollama, &text).await?;
        self.documents.push(embedding);
        Ok(())
    }

    /// Search for the top_n most similar documents to the query.
    pub async fn search(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<Vec<&Document>, Box<dyn std::error::Error>> {
        let query_embedding = fetch_embedding( ollama, query).await?;
        let mut results: Vec<(&Document, f32)> = self.documents.iter()
            .map(|doc| (doc, cosine_similarity(&doc.embedding, &query_embedding.embedding)))
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
async fn fetch_embedding(ollama: &Ollama, text: &str) -> Result<Document, Box<dyn std::error::Error>> {
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

/// A simple cosine similarity implementation.
fn cosine_similarity(a: &Vec<f32>, b: &Vec<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

/// Refine a query using the Ollama chat API.
async fn refine_query(original_query: &str, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
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

/// The existing streaming endpoint (now assuming session is pre‐initialized).
async fn stream_greeting(data: web::Data<AppState>, session: Session, req_body: web::Json<Value>) -> HttpResponse {
    let maybe_session = session.get::<UserSession>("user_session");
    let mut user_session = if let Ok(Some(session_data)) = maybe_session {
        session_data
    } else {
        // In case the session is missing, return an error.
        return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Session not initialized"}));
    };

    info!("Received request with payload: {:?}", req_body);
    let user_input = req_body["message"].as_str().unwrap_or_default().to_string();
    session.insert("user_session", &user_session).unwrap();

    if user_input.trim() == "@list" {
        info!("Streaming inbox messages with @list");
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

        let refined_query = refine_query(&user_input, &mut data.ollama.clone()).await.unwrap();
        println!("Refined Query: {}", refined_query);

        let context_str = user_session.mailbox.get_context(&refined_query, 2, &mut data.ollama.clone()).await.unwrap();
        println!("Retrieved Context:\n{}", context_str);

        let conversation = vec![
            ChatMessage::system(SYSTEM_PROMPT.to_string()),
            ChatMessage::system(format!("Context from emails:\n{}", context_str)),
            ChatMessage::user(user_input.to_string()),
        ];

        let request = ChatMessageRequest::new(MODEL_NAME.to_string(), conversation);
        let mut local_ollama = data.ollama.clone();
        let response = local_ollama.send_chat_messages_with_history(&mut user_session.history, request).await.unwrap();
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
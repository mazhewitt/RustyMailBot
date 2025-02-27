mod gmail;
mod global_session_manager;

use std::time::Duration;
use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web::middleware::Logger;
use serde_json::Value;
use tokio::time::timeout;
use log::{info, error, warn};

use actix_session::{SessionMiddleware, Session};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use gmail::{check_auth, oauth_login, oauth_callback, get_inbox_messages};
use ollama_rs::Ollama;
use ollama_rs::generation::chat::{ChatMessage, request::ChatMessageRequest};
use ollama_rs::generation::embeddings::request::{GenerateEmbeddingsRequest, EmbeddingsInput};
use crate::global_session_manager::GlobalSessionManager;

struct AppState {
    ollama: Ollama,
    session_manager: GlobalSessionManager,
}

#[derive(Clone, Deserialize, Serialize)]
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

/// Endpoint to initialize the user session and load the inbox into the vector database.
async fn init_session_endpoint(data: web::Data<AppState>, session: Session) -> HttpResponse {
    let session_id = Uuid::new_v4().to_string();
    if let Err(e) = session.insert("session_id", session_id.clone()) {
        error!("Failed to insert session_id into cookie: {:?}", e);
    } else {
        info!("Stored session_id {} in cookie", session_id);
    }

    // Check if the session already exists (unlikely with a new UUID)
    if data.session_manager.get(&session_id).is_some() {
        return HttpResponse::Ok().json(serde_json::json!({ "initialized": true, "session_id": session_id }));
    }

    let mut new_session = UserSession::default();
    let mut ollama_instance = data.ollama.clone();

    info!("Loading emails into vector database for session {}", session_id);
    let load_result = timeout(Duration::from_secs(300), load_emails(&mut ollama_instance)).await;

    match load_result {
        Ok(result) => match result {
            Ok(documents) => {
                let email_count = documents.len();
                new_session.mailbox.documents = documents;
                info!("Successfully loaded {} emails for session {}", email_count, session_id);
            },
            Err(e) => {
                error!("Error loading emails for session {}: {:?}", session_id, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to load emails",
                    "details": e.to_string()
                }));
            }
        },
        Err(_) => {
            error!("Timed out loading emails for session {}", session_id);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Loading emails timed out"
            }));
        }
    }

    data.session_manager.insert(session_id.clone(), new_session);
    info!("Initialized user session: {}", session_id);

    HttpResponse::Ok().json(serde_json::json!({ "initialized": true, "session_id": session_id }))
}

/// Load emails from Gmail and generate embeddings.
pub async fn load_emails(ollama: &mut Ollama) -> Result<Vec<Document>, Box<dyn std::error::Error>> {
    let emails = get_inbox_messages().await?;
    let mut documents = Vec::new();

    for email in emails {
        let id = email.message_id.unwrap_or_default();
        let text = format!(
            "Message:{}\nFrom: {}\nTo: {}\nDate: {}\nSubject: {}\n\n{}",
            id,
            email.from.unwrap_or_default(),
            email.to.unwrap_or_default(),
            email.date.unwrap_or_default(),
            email.subject.unwrap_or_default(),
            email.body.unwrap_or_default()
        );
        info!("Generating embedding for email: {}", id);
        let embedding = fetch_embedding(ollama, &text).await?;
        documents.push(embedding);
    }
    Ok(documents)
}

/// Document structure for embeddings.
#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub text: String,
    pub embedding: Vec<f32>,
}

/// A simple vector database for documents.
#[derive(Clone, Serialize, Deserialize)]
pub struct VectorDatabase {
    pub documents: Vec<Document>,
}

impl VectorDatabase {
    pub fn new() -> Self {
        Self { documents: Vec::new() }
    }

    pub async fn insert(&mut self, text: String, ollama: &mut Ollama) -> Result<(), Box<dyn std::error::Error>> {
        let embedding = fetch_embedding(ollama, &text).await?;
        self.documents.push(embedding);
        Ok(())
    }

    pub async fn search(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<Vec<&Document>, Box<dyn std::error::Error>> {
        let query_embedding = fetch_embedding(ollama, query).await?;
        let mut results: Vec<(&Document, f32)> = self.documents.iter()
            .map(|doc| (doc, cosine_similarity(&doc.embedding, &query_embedding.embedding)))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(results.into_iter().take(top_n).map(|(doc, _)| doc).collect())
    }

    pub async fn get_context(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
        let results = self.search(query, top_n, ollama).await?;
        let context: Vec<String> = results.iter().map(|doc| doc.text.clone()).collect();
        Ok(context.join("\n---\n"))
    }
}

/// Generate an embedding using Ollama.
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

/// Cosine similarity between two embeddings.
fn cosine_similarity(a: &Vec<f32>, b: &Vec<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

/// Refine the user query using Ollama chat.
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

/// Streaming endpoint to handle user messages.
async fn stream_greeting(data: web::Data<AppState>, session: Session, req_body: web::Json<Value>) -> HttpResponse {
    // Retrieve the session ID from the cookie (fallback to request body if necessary)
    let session_id = if let Ok(Some(id)) = session.get::<String>("session_id") {
        id
    } else {
        warn!("No valid session_id found in cookie; falling back to request body");
        req_body["session_id"].as_str().unwrap_or_default().to_string()
    };

    info!("Stream Greeting on session: {}", session_id);

    if let Some(mut user_session) = data.session_manager.get(&session_id) {
        let user_input = req_body["message"].as_str().unwrap_or_default().to_string();
        info!("Processing message for session {}: {}", session_id, user_input);

        let refined_query = match refine_query(&user_input, &mut data.ollama.clone()).await {
            Ok(q) => {
                info!("Refined query for session {}: {}", session_id, q);
                q
            },
            Err(e) => {
                error!("Error refining query for session {}: {:?}", session_id, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Query refinement failed"}));
            }
        };

        let context_str = match user_session.mailbox.get_context(&refined_query, 2, &mut data.ollama.clone()).await {
            Ok(ctx) => {
                info!("Retrieved context for session {}: {}", session_id, ctx);
                ctx
            },
            Err(e) => {
                error!("Error retrieving context for session {}: {:?}", session_id, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Context retrieval failed"}));
            }
        };

        let conversation = vec![
            ChatMessage::system(SYSTEM_PROMPT.to_string()),
            ChatMessage::system(format!("Context from emails:\n{}", context_str)),
            ChatMessage::user(user_input.clone()),
        ];

        let request = ChatMessageRequest::new(MODEL_NAME.to_string(), conversation);
        let mut local_ollama = data.ollama.clone();
        let response = match local_ollama.send_chat_messages_with_history(&mut user_session.history, request).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Error processing chat for session {}: {:?}", session_id, e);
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "Chat processing failed"}));
            }
        };

        info!("Response for session {}: {}", session_id, response.message.content);

        // Update the session after processing
        data.session_manager.insert(session_id.clone(), user_session);
        HttpResponse::Ok().json(serde_json::json!({"response": response.message.content}))
    } else {
        error!("Session \"{}\" not found!", session_id);
        HttpResponse::InternalServerError().json(serde_json::json!({"error": "Session not initialized"}))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging.
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    info!("Starting server on http://127.0.0.1:8080");

    // Use a fixed secret key so that session cookies remain valid.
    let secret_key = Key::from("0123456789012345678901234567890123456789012345678901234567890123".as_bytes());

    let ollama = Ollama::new("http://localhost", 11434);
    let session_manager = GlobalSessionManager::new();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                ollama: ollama.clone(),
                session_manager: session_manager.clone(),
            }))
            .wrap(Logger::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_secure(false)      // allow cookies over HTTP (development)
                    .cookie_same_site(SameSite::Lax)
                    .build()
            )
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
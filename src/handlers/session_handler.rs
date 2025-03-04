use actix_session::Session;
use actix_web::web;
use uuid::Uuid;
use log::{info, error};
use serde_json::json;
use crate::routes::app_state::AppState;
use crate::models::user_session::UserSession;
use crate::models::email_db::EmailDB;
use crate::services::email_service;

pub async fn initialize_session(
    data: web::Data<AppState>,
    session: Session,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Generate a new session ID and store it in the cookie.
    let session_id = Uuid::new_v4().to_string();
    if let Err(e) = session.insert("session_id", session_id.clone()) {
        error!("Failed to insert session_id into cookie: {:?}", e);
    } else {
        info!("Stored session_id {} in cookie", session_id);
    }

    // Check if the session already exists (unlikely with a new UUID)
    if data.session_manager.get(&session_id).is_some() {
        return Ok(json!({ "initialized": true, "session_id": session_id }));
    }

    let mut new_session = UserSession {
        history: Vec::new(),
        mailbox: EmailDB::default().await?,
    };

    let mut ollama_instance = data.ollama.clone();

    info!("Loading emails into vector database for session {}", session_id);
    // Attempt to load emails
    let emails = match email_service::load_emails(&mut ollama_instance).await {
        Ok(docs) => docs,
        Err(e) => {
            error!("Error loading emails for session {}: {:?}", session_id, e);
            // Return a valid JSON error response
            return Err(format!("Failed to load emails: {}", e).into());
        }
    };

    let email_count = emails.len();
    new_session.mailbox.store_emails(&emails).await?;
    info!("Successfully loaded {} emails for session {}", email_count, session_id);

    data.session_manager.insert(session_id.clone(), new_session);
    info!("Initialized user session: {}", session_id);

    Ok(json!({ "initialized": true, "session_id": session_id }))
}
use actix_web::{web, HttpResponse};
use actix_session::Session;
use serde_json::Value;
use log::{info, warn, error};
use crate::routes::app_state::AppState;
use crate::services::chat_service;

pub async fn handle_chat_request(
    data: web::Data<AppState>,
    session: Session,
    req_body: web::Json<Value>
) -> HttpResponse {
    // Retrieve session_id from cookie (or fallback)
    let session_id = if let Ok(Some(id)) = session.get::<String>("session_id") {
        id
    } else {
        warn!("No valid session_id found in cookie; falling back to request body");
        req_body["session_id"].as_str().unwrap_or_default().to_string()
    };

    if let Some(mut user_session) = data.session_manager.get(&session_id) {
        let user_input = req_body["message"].as_str().unwrap_or_default().to_string();
        info!("Processing message for session {}: {}", session_id, user_input);

        match chat_service::process_chat(&user_input, &mut user_session, data.ollama.clone()).await {
            Ok(response_content) => {
                // Update the session after processing
                data.session_manager.insert(session_id.clone(), user_session);
                HttpResponse::Ok().json(serde_json::json!({"response": response_content}))
            },
            Err(e) => {
                error!("Error processing chat for session {}: {:?}", session_id, e);
                HttpResponse::InternalServerError().json(serde_json::json!({"error": "Chat processing failed"}))
            }
        }
    } else {
        error!("Session \"{}\" not found!", session_id);
        HttpResponse::InternalServerError().json(serde_json::json!({"error": "Session not initialized"}))
    }
}
use actix_web::{get, web, HttpResponse, Responder};
use crate::routes::app_state::{AppState, DummyAppState};
use actix_session::Session;
use serde_json::json;
use log::{info, error};
use uuid::Uuid;
use crate::models::user_session::UserSession;
use crate::services::email_service;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(init_session);
}


#[get("/init_session")]
async fn init_session(data: web::Data<AppState>, session: Session) -> impl Responder {
    match crate::handlers::session_handler::initialize_session(data, session).await {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => {
            error!("Error initializing session: {:?}", e);
            HttpResponse::InternalServerError().json(json!({"error": e.to_string()}))
        }
    }
}
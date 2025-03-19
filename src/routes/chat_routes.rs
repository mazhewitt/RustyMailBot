use actix_web::{post, web, Responder};
use actix_session::Session;
use serde_json::Value;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(stream_greeting);
}

#[post("/stream")]
async fn stream_greeting(
    data: web::Data<crate::routes::app_state::AppState>,
    session: Session,
    req_body: web::Json<Value>
) -> impl Responder {
    crate::handlers::chat_handler::handle_chat_request(data, session, req_body).await
}
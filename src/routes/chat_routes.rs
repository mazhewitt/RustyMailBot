use actix_web::{post, web, HttpResponse, Responder};
use actix_session::Session;
use serde_json::Value;
use log::warn;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(stream_greeting);
}

#[post("/stream")]
async fn stream_greeting(
    data: web::Data<crate::routes::app_state::AppState>,
    session: Session,
    req_body: web::Json<Value>
) -> impl Responder {
    crate::handlers::chat_handler::stream_greeting(data, session, req_body).await
}
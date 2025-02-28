use actix_web::{get, web, HttpRequest, Responder};

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(check_auth)
        .service(oauth_login)
        .service(oauth_callback);
}

#[get("/check_auth")]
async fn check_auth() -> impl Responder {
    crate::handlers::oauth_handler::check_auth().await
}

#[get("/oauth/login")]
async fn oauth_login() -> impl Responder {
    crate::handlers::oauth_handler::oauth_login().await
}

#[get("/oauth/callback")]
async fn oauth_callback(req: HttpRequest) -> impl Responder {
    crate::handlers::oauth_handler::oauth_callback(req).await
}
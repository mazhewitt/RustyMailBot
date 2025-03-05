use ollama_rs::Ollama;
use crate::models::global_session_manager::GlobalSessionManager;

#[derive(Clone)]
pub struct AppState {
    pub ollama: Ollama,
    pub session_manager: GlobalSessionManager,
}


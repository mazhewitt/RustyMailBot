use ollama_rs::Ollama;
use crate::models::global_session_manager::GlobalSessionManager;

#[derive(Clone)]
pub struct AppState {
    pub ollama: Ollama,
    pub session_manager: GlobalSessionManager,
}

impl AppState {
    pub fn new(ollama: Ollama, session_manager: GlobalSessionManager) -> Self {
        Self { ollama, session_manager }
    }
}

#[derive(Clone)]
pub struct DummyAppState {
    pub dummy: usize,
}
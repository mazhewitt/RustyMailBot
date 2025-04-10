use crate::models::global_session_manager::GlobalSessionManager;

#[derive(Clone)]
pub struct AppState {
    pub session_manager: GlobalSessionManager,
}


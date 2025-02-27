use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use crate::UserSession;

#[derive(Clone)]
pub struct GlobalSessionManager {
    sessions: Arc<Mutex<HashMap<String, UserSession>>>,
}

impl GlobalSessionManager {
    pub fn new() -> Self {
        GlobalSessionManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Inserts or updates a session
    pub fn insert(&self, session_id: String, session: UserSession) {
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id, session);
    }

    /// Retrieves a session if it exists
    pub fn get(&self, session_id: &str) -> Option<UserSession> {
        let sessions = self.sessions.lock().unwrap();
       sessions.get(session_id).cloned()
    }
}
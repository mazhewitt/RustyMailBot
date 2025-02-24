use std::collections::HashMap;
use std::future::{ready, Future};
use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use actix_web::cookie::time::Duration;
use tokio::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;
use futures::FutureExt; // for .boxed()
use anyhow::anyhow;

// A simple in-memory session store using asynchronous locks.
pub struct MemorySessionStore {
    sessions: Mutex<HashMap<String, (HashMap<String, String>, Instant)>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        MemorySessionStore {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl SessionStore for MemorySessionStore {
    fn load(
        &self,
        session_key: &SessionKey,
    ) -> impl Future<Output = Result<Option<HashMap<String, String>>, LoadError>> {
        let key_str = session_key.as_ref().to_owned();
        async move {
            let sessions = self.sessions.lock().await;
            let now = Instant::now();
            let result = if let Some((state, expiry)) = sessions.get(&key_str) {
                if now < *expiry {
                    Some(state.clone())
                } else {
                    None
                }
            } else {
                None
            };
            Ok(result)
        }
            .boxed()
    }

    fn save(
        &self,
        session_state: HashMap<String, String>,
        ttl: &Duration,
    ) -> impl Future<Output = Result<SessionKey, SaveError>> {
        let ttl = *ttl;
        async move {
            let mut sessions = self.sessions.lock().await;
            let key: String = Uuid::new_v4().to_string();
            let expiry = Instant::now() + ttl;
            sessions.insert(key.clone(), (session_state, expiry));
            let session_key = SessionKey::try_from(key).unwrap();
            Ok(session_key)
        }
            .boxed()
    }

    fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        ttl: &Duration,
    ) -> impl Future<Output = Result<SessionKey, UpdateError>> {
        let ttl = *ttl;
        let key_str = session_key.as_ref().to_owned();
        async move {
            let mut sessions = self.sessions.lock().await;
            let expiry = Instant::now() + ttl;
            sessions.insert(key_str, (session_state, expiry));
            Ok(session_key)
        }
            .boxed()
    }

    fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &Duration,
    ) -> impl Future<Output = Result<(), anyhow::Error>> {
        let ttl = *ttl;
        let key_str = session_key.as_ref().to_owned();
        async move {
            let mut sessions = self.sessions.lock().await;
            if let Some((state, _)) = sessions.get(&key_str).cloned() {
                let new_expiry = Instant::now() + ttl;
                sessions.insert(key_str, (state, new_expiry));
                Ok(())
            } else {
                Err(anyhow!("Session not found"))
            }
        }
            .boxed()
    }

    fn delete(
        &self,
        session_key: &SessionKey,
    ) -> impl Future<Output = Result<(), anyhow::Error>> {
        let key_str = session_key.as_ref().to_owned();
        async move {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(&key_str);
            Ok(())
        }
            .boxed()
    }
}
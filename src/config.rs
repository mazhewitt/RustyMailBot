pub fn init_logging() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
}

pub const MODEL_NAME: &str = "llama3.2";
pub const EMBEDDING_MODEL: &str = "all-minilm";
pub const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";
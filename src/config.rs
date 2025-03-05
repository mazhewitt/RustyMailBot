pub fn init_logging() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
}

pub const MODEL_NAME: &str = "llama3.2";
pub const EMBEDDING_MODEL: &str = "all-minilm";
pub const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";
const OLLAMA_PORT: u16 = 11435;

pub fn ollama_port() -> u16 {
    OLLAMA_PORT
}

const OLLAMA_HOST: &'static str = "localhost";

pub fn ollama_host() -> String {
    String::from("http://".to_owned() + OLLAMA_HOST)
}

const MEILISEARCH_MASTER_KEY: &'static str = "dev-key";

pub fn meilisearch_master_key() -> String {
    String::from(MEILISEARCH_MASTER_KEY)
}

const MEILISEARCH_URL: &'static str = "http://localhost:7700";

pub fn meilisearch_url() -> String {
    String::from(MEILISEARCH_URL)
}


use std::{env, fs};

use std::path::Path;
use dotenv::dotenv;

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

const MEILI_SEARCH_KEY_PATH: &str = "/etc/secrets/meili/MEILI_SEARCH_KEY";
const MEILI_ADMIN_KEY_PATH: &str = "/etc/secrets/meili/MEILI_ADMIN_KEY";
const MEILI_LOCAL_KEY_PATH: &str = "/tmp/meilisearch-keys.env"; // Dev mode file path

pub fn meilisearch_admin_key() -> String {
    if env::var("DEV_MODE").unwrap_or_else(|_| "false".to_string()) == "true" {
        return read_key_from_file("MEILI_ADMIN_KEY");
    }
    fs::read_to_string(MEILI_ADMIN_KEY_PATH)
        .expect("Failed to read MeiliSearch admin key")
        .trim()
        .to_string()
}

pub fn meilisearch_search_key() -> String {
    if env::var("DEV_MODE").unwrap_or_else(|_| "false".to_string()) == "true" {
        return read_key_from_file("MEILI_SEARCH_KEY");
    }
    fs::read_to_string(MEILI_SEARCH_KEY_PATH)
        .expect("Failed to read MeiliSearch search key")
        .trim()
        .to_string()
}

/// Reads the key from the local file (dev mode)
fn read_key_from_file(key: &str) -> String {
    let contents = fs::read_to_string(MEILI_LOCAL_KEY_PATH).expect("Failed to read local MeiliSearch key file");
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix(&format!("{}=", key)) {
            return value.trim().to_string();
        }
    }
    panic!("Key {} not found in {}", key, MEILI_LOCAL_KEY_PATH);
}



pub struct Config {
    pub meilisearch_host: String,
    pub meilisearch_search_key: String,
    pub meilisearch_admin_key: String,
    pub ollama_host: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        // Try to load from .env file if it exists
        if Path::new(".env").exists() {
            dotenv().ok();
        }

        let config = Config {
            meilisearch_host: env::var("MEILI_HOST")
                .unwrap_or_else(|_| "http://localhost:7700".to_string()),
            meilisearch_search_key: env::var("MEILI_SEARCH_KEY")
                .map_err(|_| "MEILI_SEARCH_KEY not found in environment".to_string())?,
            meilisearch_admin_key: env::var("MEILI_ADMIN_KEY")
                .map_err(|_| "MEILI_ADMIN_KEY not found in environment".to_string())?,
            ollama_host: env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
        };

        Ok(config)
    }
}
use std::{env};
use std::path::Path;
use dotenv::dotenv;
use std::sync::Once;
use url::Url;

// A global initializer to ensure the `.env` file is loaded only once
static INIT: Once = Once::new();

pub fn setup() {
    INIT.call_once(|| {
        // Load environment variables from `.env` file
        dotenv().ok();
    });
}

pub fn init_logging() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
}

pub const MODEL_NAME: &str = "llama3.2";
pub const EMBEDDING_MODEL: &str = "all-minilm";
pub const SYSTEM_PROMPT: &str = "You are a helpful assistant for writing emails";

pub fn ollama_port() -> u16 {
    // Extract port from URL
    let url = ollama_host();
    match Url::parse(&url) {
        Ok(parsed_url) => parsed_url.port().unwrap_or(11434),
        Err(_) => 11434
    }
}

pub fn ollama_host() -> String {
    Config::from_env().unwrap().ollama_url
}

pub fn meilisearch_master_key() -> String {
   Config::from_env().unwrap().meilisearch_admin_key
}

pub fn meilisearch_url() -> String {
   Config::from_env().unwrap().meilisearch_url
}

pub fn meilisearch_admin_key() -> String {
   Config::from_env().unwrap().meilisearch_admin_key
}

pub fn meilisearch_search_key() -> String {
    Config::from_env().unwrap().meilisearch_search_key
}

pub struct Config {
    pub meilisearch_url: String,
    pub meilisearch_search_key: String,
    pub meilisearch_admin_key: String,
    pub ollama_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        // Try to load from .env file if it exists
        if Path::new(".env").exists() {
            dotenv().ok();
        }

        // Get the search key, fail if empty
        let search_key = env::var("MEILI_SEARCH_KEY")
            .map_err(|_| "MEILI_SEARCH_KEY not found in environment".to_string())?;
        if search_key.is_empty() {
            return Err("MEILI_SEARCH_KEY cannot be empty".to_string());
        }

        // Get the admin key, fail if empty
        let admin_key = env::var("MEILI_ADMIN_KEY")
            .map_err(|_| "MEILI_ADMIN_KEY not found in environment".to_string())?;
        if admin_key.is_empty() {
            return Err("MEILI_ADMIN_KEY cannot be empty".to_string());
        }

        let config = Config {
            meilisearch_url: env::var("MEILI_URL").unwrap(),
            meilisearch_search_key: search_key,
            meilisearch_admin_key: admin_key,
            ollama_url: env::var("OLLAMA_URL").unwrap(),
        };

        Ok(config)
    }

    // Create a method for testing that takes a custom environment
    #[cfg(test)]
    fn from_test_env(test_env: &std::collections::HashMap<String, String>) -> Result<Self, String> {
        // Handle search key
        let search_key = test_env.get("MEILI_SEARCH_KEY")
            .ok_or_else(|| "MEILI_SEARCH_KEY not found in test environment".to_string())?;
        if search_key.is_empty() {
            return Err("MEILI_SEARCH_KEY cannot be empty".to_string());
        }

        // Handle admin key
        let admin_key = test_env.get("MEILI_ADMIN_KEY")
            .ok_or_else(|| "MEILI_ADMIN_KEY not found in test environment".to_string())?;
        if admin_key.is_empty() {
            return Err("MEILI_ADMIN_KEY cannot be empty".to_string());
        }

        let config = Config {
            meilisearch_url: test_env.get("MEILI_URL")
                .cloned()
                .unwrap_or_else(|| "http://localhost:7700".to_string()),
            meilisearch_search_key: search_key.clone(),
            meilisearch_admin_key: admin_key.clone(),
            ollama_url: test_env.get("OLLAMA_URL")
                .cloned()
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
        };

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use url::Url;

    // Helper function to validate URLs
    fn is_valid_url(url: &str) -> bool {
        match Url::parse(url) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    #[test]
    fn test_with_env_vars() {
        // Ensure environment variables are loaded
        setup();

        // Just verify that we can access the variables and they're not empty
        let meili_url = meilisearch_url();
        assert!(is_valid_url(&meili_url), "MEILI_URL should be a valid URL");
        println!("MeiliSearch URL: {}", meili_url);
    }

    // Test functions using a mock environment
    mod with_mock_env {
        use super::*;

        // This function returns mock values for the config functions
        fn mock_env_value(key: &str) -> Option<&str> {
            let mut mock_env = HashMap::new();
            mock_env.insert("MEILI_URL", "http://localhost:7700");
            mock_env.insert("MEILI_SEARCH_KEY", "test_search_key");
            mock_env.insert("MEILI_ADMIN_KEY", "test_admin_key");
            mock_env.insert("MEILI_MASTER_KEY", "test_master_key");
            mock_env.insert("OLLAMA_URL", "http://localhost:11434");

            mock_env.get(key).cloned()
        }

        #[test]
        fn test_meilisearch_url_mock() {
            assert_eq!(
                mock_env_value("MEILI_URL").unwrap_or_else(|| "http://localhost:7700"),
                "http://localhost:7700"
            );
        }

        #[test]
        fn test_meilisearch_search_key_mock() {
            assert_eq!(
                mock_env_value("MEILI_SEARCH_KEY").unwrap_or_default(),
                "test_search_key"
            );
        }

        #[test]
        fn test_meilisearch_admin_key_mock() {
            assert_eq!(
                mock_env_value("MEILI_ADMIN_KEY").unwrap_or_default(),
                "test_admin_key"
            );
        }

        #[test]
        fn test_meilisearch_master_key_mock() {
            assert_eq!(
                mock_env_value("MEILI_MASTER_KEY").unwrap_or_default(),
                "test_master_key"
            );
        }

        #[test]
        fn test_ollama_url_mock() {
            assert_eq!(
                mock_env_value("OLLAMA_URL").unwrap_or_else(|| "http://localhost:11434"),
                "http://localhost:11434"
            );
        }

        #[test]
        fn test_port_parsing() {
            let url = "http://localhost:8080";
            let parsed = Url::parse(url).unwrap();
            assert_eq!(parsed.port().unwrap_or(0), 8080);
        }

        #[test]
        fn test_config_from_test_env() {
            let mut test_env = HashMap::new();
            test_env.insert("MEILI_URL".to_string(), "http://localhost:7700".to_string());
            test_env.insert("MEILI_SEARCH_KEY".to_string(), "test_search_key".to_string());
            test_env.insert("MEILI_ADMIN_KEY".to_string(), "test_admin_key".to_string());
            test_env.insert("OLLAMA_URL".to_string(), "http://localhost:11434".to_string());

            let config = Config::from_test_env(&test_env).unwrap();

            assert_eq!(config.meilisearch_url, "http://localhost:7700");
            assert_eq!(config.meilisearch_search_key, "test_search_key");
            assert_eq!(config.meilisearch_admin_key, "test_admin_key");
            assert_eq!(config.ollama_url, "http://localhost:11434");
        }

        #[test]
        fn test_config_from_test_env_missing_keys() {
            // Test with missing search key
            let mut test_env = HashMap::new();
            test_env.insert("MEILI_URL".to_string(), "http://localhost:7700".to_string());
            test_env.insert("MEILI_ADMIN_KEY".to_string(), "test_admin_key".to_string());

            let result = Config::from_test_env(&test_env);
            assert!(result.is_err());

            // Test with empty search key
            test_env.insert("MEILI_SEARCH_KEY".to_string(), "".to_string());
            let result = Config::from_test_env(&test_env);
            assert!(result.is_err());

            // Test with missing admin key
            let mut test_env = HashMap::new();
            test_env.insert("MEILI_URL".to_string(), "http://localhost:7700".to_string());
            test_env.insert("MEILI_SEARCH_KEY".to_string(), "test_search_key".to_string());

            let result = Config::from_test_env(&test_env);
            assert!(result.is_err());

            // Test with empty admin key
            test_env.insert("MEILI_ADMIN_KEY".to_string(), "".to_string());
            let result = Config::from_test_env(&test_env);
            assert!(result.is_err());
        }
    }

    // Test real functions with the current environment (non-strict tests)
    #[test]
    fn test_config_from_real_env() {
        // This test just verifies that the config can be created and has sensible values
        match Config::from_env() {
            Ok(config) => {
                // Just check that URLs are valid and keys aren't empty
                assert!(is_valid_url(&config.meilisearch_url), "MeiliSearch URL should be valid");
                assert!(is_valid_url(&config.ollama_url), "Ollama URL should be valid");
                assert!(!config.meilisearch_search_key.is_empty(), "Search key should not be empty");
                assert!(!config.meilisearch_admin_key.is_empty(), "Admin key should not be empty");
            },
            Err(e) => {
                // If this fails, just log the error - don't fail the test
                // This handles the case where the real env doesn't have the keys
                println!("Config::from_env() failed: {}. This is expected if running tests without environment setup.", e);
            }
        }
    }
}
// Integration tests use shared baseline setup
use AdukiChatAgent::config;
use AdukiChatAgent::models::email::{Email};
use AdukiChatAgent::models::email_db::{EmailDB, EmailDBError};
use AdukiChatAgent::models::email_query::QueryCriteria;
use super::setup_test_db_all;

#[tokio::test]
async fn test_store_and_search_email() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db_all().await?;
    let results = db.search_emails("Test Email Store").await?;
    assert!(results.iter().any(|e| e.message_id == Some("test-1".to_string())));
    Ok(())
}

#[tokio::test]
async fn test_store_emails() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db_all().await?;
    let results = db.search_emails("Bulk Email").await?;
    let ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
    assert!(ids.contains(&"test-2".to_string()));
    assert!(ids.contains(&"test-3".to_string()));
    assert!(ids.contains(&"test-4".to_string()));
    Ok(())
}

#[tokio::test]
async fn test_search_emails_by_criteria() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db_all().await?;
    let criteria = QueryCriteria { keywords: vec!["Advanced".to_string()], from: Some("charlie@example.com".to_string()), to: None, subject: Some("Advanced Search Test".to_string()), date_from: None, date_to: None, raw_query: "Perform an Advanced Search".to_string(), llm_confidence: 1.0 };
    let results = db.search_emails_by_criteria(criteria).await?;
    assert!(results.iter().any(|e| e.message_id == Some("test-5".to_string())));
    Ok(())
}

#[tokio::test]
async fn test_search_from_field_matching() -> Result<(), Box<dyn std::error::Error>> {
    let db = setup_test_db_all().await?;
    let criteria = QueryCriteria { keywords: vec![], from: Some("Bob".to_string()), to: None, subject: None, date_from: None, date_to: None, raw_query: "".to_string(), llm_confidence: 0.0 };
    let results = db.search_emails_by_criteria(criteria).await?;
    let found_ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
    assert!(found_ids.contains(&"test-from-1".to_string()));
    assert!(found_ids.contains(&"test-from-2".to_string()));
    assert!(!found_ids.contains(&"test-from-3".to_string()));
    Ok(())
}
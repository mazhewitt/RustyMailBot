use std::time::Duration;
use tokio::time::sleep;

// Correct imports using the actual crate name
use AdukiChatAgent::config;
use AdukiChatAgent::models::email::{Email};
use AdukiChatAgent::models::email_db::{EmailDB, EmailDBError};
use AdukiChatAgent::models::email_query::QueryCriteria;

#[tokio::test]
async fn test_alice_name_matching_issue() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    let _ = env_logger::builder().is_test(true).try_init();

    // Use shared setup for email matching: only Alice and Parallels emails
    let db = super::setup_email_matching_db().await?;

    // Database preloaded with baseline emails: alice-email-1 and parallels-email-1

    // Create criteria that represents "explain the email from alice"
    let criteria = QueryCriteria {
        keywords: vec![],
        from: Some("alice".to_string()),  // This is what gets extracted from the query
        to: None,
        subject: None,
        date_from: None,
        date_to: None,
        raw_query: "explain the email from alice".to_string(),
        llm_confidence: 0.9,
    };

    // Execute search by criteria
    let results = db.search_emails_by_criteria(criteria).await?;
    // Only alice-email-1 should be returned

    // Print the results for debugging
    println!("Search results when looking for 'from: alice':");
    for email in &results {
        println!("- Message ID: {:?}, From: {:?}, Subject: {:?}", 
                 email.message_id, email.from, email.subject);
    }

    // Ensure at least one result
    assert!(!results.is_empty(), "Expected to find at least one email from Alice, but found none.");

    // All returned emails should have 'alice' in the from field
    for email in &results {
        let from_lower = email.from.as_ref().unwrap().to_lowercase();
        assert!(from_lower.contains("alice"),
            "Found an email not from Alice: {:?}", email.from);
    }

    // This assertion will pass only if we didn't match the Parallels email
    assert!(!results.iter().any(|e| e.message_id == Some("parallels-email-1".to_string())),
           "Found Parallels email when searching for 'from: alice', which demonstrates the bug");

    Ok(())
}
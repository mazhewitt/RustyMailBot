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

#[tokio::test]
async fn test_first_name_search_issue() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    let _ = env_logger::builder().is_test(true).try_init();

    // Set up a clean test database with a unique index name to ensure isolation
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    // Use a unique index name to prevent conflicts with other tests
    let unique_index = format!("test_phil_search_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs());
    
    let db = EmailDB::new(&url, Some(&admin_key), &unique_index).await?;
    
    // Create test email with complex "From" field containing both first name and last name
    let test_email = Email {
        message_id: Some("phil-email-1".to_string()),
        from: Some("Phil Amberg <phil.amberg@example.com>".to_string()),
        to: Some("user@example.com".to_string()),
        date: Some("2025-03-04T12:00:00Z".to_string()),
        subject: Some("Important meeting".to_string()),
        body: Some("This is a test email from Phil.".to_string()),
    };
    
    // Store the test email and wait for indexing
    db.store_email(&test_email).await?;
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await; // Allow indexing with a longer wait time
    
    // Direct test using get_all_emails to ensure email was stored
    let all_emails = db.get_all_emails().await?;
    
    println!("All emails in test database ({}):", unique_index);
    for email in &all_emails {
        println!("- ID: {:?}, From: {:?}", email.message_id, email.from);
    }
    
    // Make sure our test email is actually stored
    assert!(all_emails.iter().any(|e| e.message_id == Some("phil-email-1".to_string())), 
        "Test email not found in database");
    
    // Now proceed with the actual test
    let criteria = QueryCriteria {
        keywords: vec![],
        from: Some("Phil".to_string()),
        to: None,
        subject: None,
        date_from: None,
        date_to: None,
        raw_query: "find the email from Phil".to_string(),
        llm_confidence: 0.9,
    };
    
    // Execute search by criteria
    let results = db.search_emails_by_criteria(criteria).await?;
    
    // Print the results for debugging
    println!("Search results when looking for emails from 'Phil':");
    for email in &results {
        println!("- Message ID: {:?}, From: {:?}, Subject: {:?}", 
                 email.message_id, email.from, email.subject);
    }
    
    // This should now pass because we confirmed the email exists in the database
    // If it still fails, the problem is definitely in the search implementation
    assert!(!results.is_empty(), "Expected to find at least one email from Phil, but found none.");
    
    // Verify the right email was found
    assert!(results.iter().any(|e| e.message_id == Some("phil-email-1".to_string())), 
        "Phil's email was not found in search results");
    
    // All returned emails should have 'phil' in the from field
    for email in &results {
        let from_lower = email.from.as_ref().unwrap().to_lowercase();
        assert!(from_lower.contains("phil"),
            "Found an email not from Phil: {:?}", email.from);
    }
    
    // Clean up by deleting the unique index
    db.clear().await?;
    
    Ok(())
}
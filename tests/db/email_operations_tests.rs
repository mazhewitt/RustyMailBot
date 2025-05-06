// Integration tests use shared baseline setup
use AdukiChatAgent::config;
use AdukiChatAgent::models::email::{Email};
use AdukiChatAgent::models::email_db::{EmailDB, EmailDBError};
use AdukiChatAgent::models::email_query::QueryCriteria;
use super::setup_test_db_all;

#[tokio::test]
async fn test_store_and_search_email() -> Result<(), Box<dyn std::error::Error>> {
    // Create a direct instance of EmailDB for integration testing
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    let db = EmailDB::new(&url, Some(&admin_key), "emails_test").await?;
    db.clear().await?;  // Start with a clean database
    
    // Create a test email 
    let test_email = Email {
        message_id: Some("test-1".to_string()),
        from: Some("test@example.com".to_string()),
        to: Some("recipient@example.com".to_string()),
        date: Some("2025-03-04T12:00:00Z".to_string()),
        subject: Some("Test Email Store".to_string()),
        body: Some("This is a test email.".to_string()),
    };
    
    // Store the email directly
    db.store_email(&test_email).await?;
    
    // Wait for indexing to complete
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    
    // Verify email was stored using get_all_emails
    let all_emails = db.get_all_emails().await?;
    println!("All emails in test database:");
    for email in &all_emails {
        println!("- ID: {:?}, Subject: {:?}", email.message_id, email.subject);
    }
    
    // Check if our test email was stored
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-1".to_string())), 
        "Test email not found in all_emails result");
    
    // Search for the email
    let results = db.search_emails("Test Email Store").await?;
    
    // This assertion should now pass since we verified the email exists
    assert!(results.iter().any(|e| e.message_id == Some("test-1".to_string())));
    
    // Clean up
    db.clear().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_store_emails() -> Result<(), Box<dyn std::error::Error>> {
    // Create a direct instance of EmailDB for integration testing
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    // Use a unique index name to prevent conflicts with other tests
    let unique_index = format!("test_store_emails_{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs());
    
    let db = EmailDB::new(&url, Some(&admin_key), &unique_index).await?;
    
    // Create test emails to store
    let emails = vec![
        Email {
            message_id: Some("test-2".to_string()),
            from: Some("sender@example.com".to_string()),
            to: Some("recipient@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Bulk Email 1".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-3".to_string()),
            from: Some("sender@example.com".to_string()),
            to: Some("recipient@example.com".to_string()),
            date: Some("2025-03-04T12:05:00Z".to_string()),
            subject: Some("Bulk Email 2".to_string()),
            body: Some("This is another test email.".to_string()),
        },
        Email {
            message_id: Some("test-4".to_string()),
            from: Some("sender@example.com".to_string()),
            to: Some("recipient@example.com".to_string()),
            date: Some("2025-03-04T12:10:00Z".to_string()),
            subject: Some("Bulk Email 3".to_string()),
            body: Some("This is yet another test email.".to_string()),
        },
    ];
    
    // Store the emails directly
    db.store_emails(&emails).await?;
    
    // Wait for indexing to complete
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    
    // Verify emails were stored using get_all_emails
    let all_emails = db.get_all_emails().await?;
    
    println!("All emails in test database ({}):", unique_index);
    for email in &all_emails {
        println!("- ID: {:?}, Subject: {:?}", email.message_id, email.subject);
    }
    
    // Check if our test emails were stored
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-2".to_string())), 
        "Test email 2 not found in all_emails result");
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-3".to_string())), 
        "Test email 3 not found in all_emails result");
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-4".to_string())), 
        "Test email 4 not found in all_emails result");
    
    // Search for the emails
    let results = db.search_emails("Bulk Email").await?;
    
    println!("Search results for 'Bulk Email':");
    for email in &results {
        println!("- ID: {:?}, Subject: {:?}", email.message_id, email.subject);
    }
    
    // Extract IDs for easier assertion
    let ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
    
    // Check all three emails were found in the search results
    assert!(ids.contains(&"test-2".to_string()), "Bulk Email 1 not found in search results");
    assert!(ids.contains(&"test-3".to_string()), "Bulk Email 2 not found in search results");
    assert!(ids.contains(&"test-4".to_string()), "Bulk Email 3 not found in search results");
    
    // Clean up
    db.clear().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_search_emails_by_criteria() -> Result<(), Box<dyn std::error::Error>> {
    // Create a direct instance of EmailDB for integration testing
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    let db = EmailDB::new(&url, Some(&admin_key), "emails_test").await?;
    db.clear().await?;  // Start with a clean database
    
    // Create a test email with the specific criteria we're testing
    let test_email = Email {
        message_id: Some("test-5".to_string()),
        from: Some("charlie@example.com".to_string()),
        to: Some("recipient@example.com".to_string()),
        date: Some("2025-03-04T12:00:00Z".to_string()),
        subject: Some("Advanced Search Test".to_string()),
        body: Some("This is a test email for advanced search.".to_string()),
    };
    
    // Store the email directly
    db.store_email(&test_email).await?;
    
    // Wait for indexing to complete
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    
    // Verify email was stored using get_all_emails
    let all_emails = db.get_all_emails().await?;
    println!("All emails in test database:");
    for email in &all_emails {
        println!("- ID: {:?}, From: {:?}, Subject: {:?}", 
                email.message_id, email.from, email.subject);
    }
    
    // Check if our test email was stored
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-5".to_string())), 
        "Test email not found in all_emails result");
    
    // Create the search criteria
    let criteria = QueryCriteria { 
        keywords: vec!["Advanced".to_string()], 
        from: Some("charlie@example.com".to_string()), 
        to: None, 
        subject: Some("Advanced Search Test".to_string()), 
        date_from: None, 
        date_to: None, 
        raw_query: "Perform an Advanced Search".to_string(), 
        llm_confidence: 1.0 
    };
    
    // Search for emails matching the criteria
    let results = db.search_emails_by_criteria(criteria).await?;
    
    println!("Search results for advanced criteria:");
    for email in &results {
        println!("- ID: {:?}, From: {:?}, Subject: {:?}", 
                email.message_id, email.from, email.subject);
    }
    
    // This assertion should now pass since we verified the email exists
    assert!(results.iter().any(|e| e.message_id == Some("test-5".to_string())));
    
    // Clean up
    db.clear().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_search_from_field_matching() -> Result<(), Box<dyn std::error::Error>> {
    // Create a direct instance of EmailDB for integration testing
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    let db = EmailDB::new(&url, Some(&admin_key), "emails_test").await?;
    db.clear().await?;  // Start with a clean database
    
    // Create test emails with Bob's information
    let email1 = Email {
        message_id: Some("test-from-1".to_string()),
        from: Some("bob@example.com".to_string()),
        to: Some("user@example.com".to_string()),
        date: Some("2025-03-04T12:00:00Z".to_string()),
        subject: Some("Email from Bob".to_string()),
        body: Some("Test email content.".to_string()),
    };

    let email2 = Email {
        message_id: Some("test-from-2".to_string()),
        from: Some("Bob <robert@foo.com>".to_string()),
        to: Some("user@example.com".to_string()),
        date: Some("2025-03-04T12:05:00Z".to_string()),
        subject: Some("Another email from Bob".to_string()),
        body: Some("Another test email.".to_string()),
    };

    let email3 = Email {
        message_id: Some("test-from-3".to_string()),
        from: Some("alice@example.com".to_string()),
        to: Some("user@example.com".to_string()),
        date: Some("2025-03-04T12:10:00Z".to_string()),
        subject: Some("Email from Alice".to_string()),
        body: Some("Control email content.".to_string()),
    };
    
    // Store emails directly
    db.store_emails(&[email1.clone(), email2.clone(), email3.clone()]).await?;
    
    // Wait for indexing to complete
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    
    // Verify emails were stored using get_all_emails
    let all_emails = db.get_all_emails().await?;
    
    println!("All emails in test database:");
    for email in &all_emails {
        println!("- ID: {:?}, From: {:?}", email.message_id, email.from);
    }
    
    // Check if our test emails were stored
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-from-1".to_string())), 
        "Bob's first email not found in all_emails result");
    assert!(all_emails.iter().any(|e| e.message_id == Some("test-from-2".to_string())), 
        "Bob's second email not found in all_emails result");
    
    // Create a query criteria with the 'from' field set to 'Bob'
    let criteria = QueryCriteria { 
        keywords: vec![], 
        from: Some("Bob".to_string()), 
        to: None, 
        subject: None, 
        date_from: None, 
        date_to: None, 
        raw_query: "emails from Bob".to_string(),
        llm_confidence: 0.0 
    };
    
    // Search for emails matching the criteria
    let results = db.search_emails_by_criteria(criteria).await?;
    
    println!("Search results when looking for 'from: Bob':");
    for email in &results {
        println!("- ID: {:?}, From: {:?}", email.message_id, email.from);
    }
    
    // Extract message IDs from the results
    let found_ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
    
    // Verify that both 'Bob' emails are found, but not Alice's email
    assert!(found_ids.contains(&"test-from-1".to_string()), "Bob's first email not found");
    assert!(found_ids.contains(&"test-from-2".to_string()), "Bob's second email not found");
    assert!(!found_ids.contains(&"test-from-3".to_string()), "Alice's email should not be returned");
    
    // Clean up
    db.clear().await?;
    
    Ok(())
}
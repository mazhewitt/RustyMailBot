// Import the test files
pub mod email_db_tests;
pub mod email_operations_tests;

use AdukiChatAgent::config;
use AdukiChatAgent::models::email::{Email};
use AdukiChatAgent::models::email_db::{EmailDB, EmailDBError};

/// Shared setup: clear test index and insert baseline emails for all tests
pub async fn setup_test_db_all() -> Result<EmailDB, EmailDBError> {
    let _ = env_logger::builder().is_test(true).try_init();
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    let db = EmailDB::new(&url, Some(&admin_key), "emails_test").await?;
    // clean slate
    db.clear().await?;
    // insert Alice email and a Parallels email for matching tests
    let baseline = vec![
        // Alice explain tests
        Email {
            message_id: Some("alice-email-1".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Meeting tomorrow".to_string()),
            body: Some("Hi Alice's meeting request".to_string()),
        },
        Email {
            message_id: Some("parallels-email-1".to_string()),
            from: Some("Parallels <noreply@parallels.com>".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-05T09:00:00Z".to_string()),
            subject: Some("Activate your Parallels account".to_string()),
            body: Some("Account name is alice, please activate.".to_string()),
        },
        // Operations tests baseline
        Email {
            message_id: Some("test-1".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Test Email Store".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-2".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Bulk Email 1".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-3".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Bulk Email 2".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-4".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Bulk Email 3".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-5".to_string()),
            from: Some("charlie@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Advanced Search Test".to_string()),
            body: Some("This is a test email.".to_string()),
        },
        Email {
            message_id: Some("test-from-1".to_string()),
            from: Some("bob@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Email from Bob".to_string()),
            body: Some("Test email content.".to_string()),
        },
        Email {
            message_id: Some("test-from-2".to_string()),
            from: Some("Bob <robert@foo.com>".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:05:00Z".to_string()),
            subject: Some("Another email from Bob".to_string()),
            body: Some("Another test email.".to_string()),
        },
        Email {
            message_id: Some("test-from-3".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:10:00Z".to_string()),
            subject: Some("Email from Alice".to_string()),
            body: Some("Control email content.".to_string()),
        },
    ];
    db.store_emails(&baseline).await?;
    // allow index
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(db)
}

/// Shared setup for explain-from-email matching tests
pub async fn setup_email_matching_db() -> Result<EmailDB, EmailDBError> {
    let _ = env_logger::builder().is_test(true).try_init();
    let url = config::meilisearch_url();
    let admin_key = config::meilisearch_admin_key();
    let db = EmailDB::new(&url, Some(&admin_key), "emails_test").await?;
    db.clear().await?;
    let baseline = vec![
        Email {
            message_id: Some("alice-email-1".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Meeting tomorrow".to_string()),
            body: Some("Hi Alice's meeting request".to_string()),
        },
        Email {
            message_id: Some("parallels-email-1".to_string()),
            from: Some("Parallels <noreply@parallels.com>".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-05T09:00:00Z".to_string()),
            subject: Some("Activate your Parallels account".to_string()),
            body: Some("Account name is alice, please activate.".to_string()),
        },
    ];
    db.store_emails(&baseline).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    Ok(db)
}
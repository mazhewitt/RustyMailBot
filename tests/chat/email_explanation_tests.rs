// Integration test to reproduce the missing explanation for an email from Test Person
use AdukiChatAgent::models::email::Email;
use AdukiChatAgent::models::user_session::UserSession;
use AdukiChatAgent::models::email_db::EmailDB;
use AdukiChatAgent::services::chat_service::process_chat;

#[tokio::test]
async fn test_explain_email_from_test_person() {
    // Initialize test database and clear existing entries
    let db = EmailDB::default().await.expect("Failed to init EmailDB");
    db.clear().await.expect("Failed to clear EmailDB");

    // Seed a test email from Test Person
    let test_email = Email {
        from: Some("Test Person <test.person@example.com>".to_string()),
        to: Some("user@example.com".to_string()),
        subject: Some("Rechnungen KS Stadelhofen doppelt ausgestellt".to_string()),
        date: Some("2025-05-05T09:29:43+02:00".to_string()),
        body: Some("Hallo, ich habe festgestellt, dass die Rechnungen doppelt ausgestellt wurden...".to_string()),
        message_id: Some("test-msg".to_string()),
    };
    db.store_email(&test_email).await.expect("Failed to store Test Person's email");

    // Create a session with the seeded database
    let mut session = UserSession { history: Vec::new(), mailbox: db.clone() };

    // Attempt to list emails from Test Person (to reproduce the bug path)
    let response = process_chat("list emails from Test Person", &mut session)
        .await
        .expect("process_chat failed");

    // Expect the English translation 'invoice' (will fail because this is a list response)
    assert!(
        response.to_lowercase().contains("invoice"),
        "Expected explanation to contain the word 'invoice', but got: {}",
        response
    );
}
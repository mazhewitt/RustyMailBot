// Integration test to reproduce the missing explanation for an email from Test Person
use AdukiChatAgent::models::email::Email;
use AdukiChatAgent::models::user_session::UserSession;
use AdukiChatAgent::models::email_db::EmailDB;
use AdukiChatAgent::services::chat_service::process_chat;

#[tokio::test]
async fn test_explain_wrong_person_email() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test session with multiple emails
    let mut email_db = EmailDB::default().await?;
    
    // Clear any existing data
    email_db.clear().await?;
    
    // Create test emails with similar formats to the real emails but with test data
    // Create more complex test data with multiple emails, including:
    // - Multiple people with similar names (Kay vs Kai)
    // - People with "Kai" as part of a longer name
    // - Email addresses vs display names
    // - Multiple emails from the same person
    let test_emails = vec![
        Email {
            from: Some("John Smith <john.smith@example.com>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Project Update Meeting".to_string()),
            body: Some("Hi, let's discuss the project progress tomorrow at 10 AM.".to_string()),
            date: Some("2025-05-04T10:00:00Z".to_string()),
            message_id: Some("msg_1".to_string()),
        },
        Email {
            from: Some("marketing@newsletters.example.com".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Weekly Newsletter - Special Offers".to_string()),
            body: Some("Check out our special offers this week! Limited time only.".to_string()),
            date: Some("2025-05-04T12:30:00Z".to_string()),
            message_id: Some("msg_2".to_string()),
        },
        Email {
            from: Some("Kay Wilson <kay.wilson@example.org>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Upcoming Social Event".to_string()),
            body: Some("Don't forget about the company picnic this weekend! Bring your family.".to_string()),
            date: Some("2025-05-04T14:00:00Z".to_string()),
            message_id: Some("msg_3".to_string()),
        },
        Email {
            from: Some("Kai Henderson <kai.henderson@example.org>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Important: Invoice #12345".to_string()),
            body: Some("Please find attached the invoice for services rendered last month. Payment due in 30 days.".to_string()),
            date: Some("2025-05-05T09:15:00Z".to_string()),
            message_id: Some("msg_4".to_string()),
        },
        Email {
            from: Some("Kaiden Brown <kaiden@example.net>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Re: Development Timeline".to_string()),
            body: Some("I think we should extend the deadline to ensure quality. Let's discuss in our next meeting.".to_string()),
            date: Some("2025-05-05T10:30:00Z".to_string()),
            message_id: Some("msg_5".to_string()),
        },
        Email {
            from: Some("Lisa Johnson <lisa@example.net>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Re: Lunch Next Week".to_string()),
            body: Some("Tuesday works great for me. Looking forward to catching up!".to_string()),
            date: Some("2025-05-05T11:45:00Z".to_string()),
            message_id: Some("msg_6".to_string()),
        },
        Email {
            from: Some("Kai Henderson <kai.henderson@example.org>".to_string()),
            to: Some("user@example.com".to_string()),
            subject: Some("Updated Invoice Information".to_string()),
            body: Some("I've updated the invoice to reflect the additional services. Please review the new total.".to_string()),
            date: Some("2025-05-05T15:30:00Z".to_string()),
            message_id: Some("msg_7".to_string()),
        },
    ];

    // Store test emails
    email_db.store_emails(&test_emails).await?;

    // Create a user session with the test emails
    let mut session = UserSession {
        history: Vec::new(),
        mailbox: email_db,
    };

    // First, list all emails to confirm they're loaded
    let list_result = process_chat("list all emails in my inbox", &mut session).await?;
    
    // Ensure all emails are listed
    assert!(list_result.contains("John Smith"), "John's email should be listed");
    assert!(list_result.contains("marketing@newsletters"), "Marketing email should be listed");
    assert!(list_result.contains("Kay Wilson"), "Kay's email should be listed");
    assert!(list_result.contains("Kai Henderson"), "Kai's email should be listed");
    assert!(list_result.contains("Kaiden Brown"), "Kaiden's email should be listed");
    assert!(list_result.contains("Lisa Johnson"), "Lisa's email should be listed");
    
    // Now try to get an explanation for Kai's email
    // First, try asking specifically for the "updated invoice" email
    let explain_update_result = process_chat("explain the updated invoice email from Kai", &mut session).await?;
    
    // This should select the updated invoice email
    let mentions_updated = explain_update_result.to_lowercase().contains("updated");
    let mentions_invoice = explain_update_result.to_lowercase().contains("invoice");
    let mentions_kai_henderson = explain_update_result.to_lowercase().contains("kai henderson");
    let mentions_additional_services = explain_update_result.to_lowercase().contains("additional services");
    
    assert!(
        mentions_updated && mentions_invoice && mentions_kai_henderson && mentions_additional_services,
        "Failed to correctly explain Kai's updated invoice email. Response: {}", 
        explain_update_result
    );
    
    // Now try a more generic query about "Kai's email" - this should still pick the most recent one
    let explain_result = process_chat("please explain the email from Kai", &mut session).await?;
    
    // This is the test for the bug - we want to verify the chat correctly identifies Kai's most recent email
    // and doesn't confuse it with Kay's email or Kaiden's email
    
    // The test should fail because the system might match Kay's social event email
    // or Kaiden's email instead of Kai's invoice email
    // Check if the correct parts are mentioned:
    // - Must contain "updated" to refer to Kai's most recent email subject
    // - Must have "Kai Henderson" to confirm the right sender
    // - Shouldn't contain "social" or "company picnic" (from Kay's email)
    // - Shouldn't contain "development" or "deadline" (from Kaiden's email)
    let mentions_invoice = explain_result.to_lowercase().contains("invoice");
    let mentions_updated = explain_result.to_lowercase().contains("updated");
    let mentions_kai_henderson = explain_result.to_lowercase().contains("kai henderson");
    let mentions_social_event = explain_result.to_lowercase().contains("social") || 
                               explain_result.to_lowercase().contains("picnic");
    let mentions_development = explain_result.to_lowercase().contains("development") || 
                              explain_result.to_lowercase().contains("deadline");
    
    // This assertion will fail if the chat explains the wrong email
    assert!(
        mentions_updated && mentions_invoice && mentions_kai_henderson && 
        !mentions_social_event && !mentions_development,
        "Failed to correctly explain Kai's email. The response appears to reference the wrong email. Response: {}", 
        explain_result
    );

    // Further validate by checking a more specific request
    let explain_invoice_result = process_chat("explain the invoice email from Kai Henderson", &mut session).await?;
    
    // This should successfully find the right email even with the more specific query
    assert!(
        explain_invoice_result.to_lowercase().contains("invoice") && 
        explain_invoice_result.to_lowercase().contains("payment") && 
        explain_invoice_result.to_lowercase().contains("kai henderson"),
        "Failed to correctly explain Kai's invoice email with a more specific query. Response: {}", 
        explain_invoice_result
    );

    Ok(())
}



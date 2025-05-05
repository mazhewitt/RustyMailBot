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

#[tokio::test]
async fn test_explain_long_email_with_bullet_points() -> Result<(), Box<dyn std::error::Error>> {
    // Create a test session with a long email
    let mut email_db = EmailDB::default().await?;
    
    // Clear any existing data
    email_db.clear().await?;
    
    // Create a long test email with at least 5 distinct key points that should be identifiable
    let long_email = Email {
        from: Some("Sarah Chen <sarah.chen@techcorp.example>".to_string()),
        to: Some("team@techcorp.example".to_string()),
        subject: Some("Quarterly Product Roadmap and Strategic Updates".to_string()),
        body: Some(
            "Dear Team,\n\n\
            I hope this email finds you well. As we approach the end of Q2, I wanted to provide a comprehensive update on our product roadmap and strategic initiatives for the remainder of 2025.\n\n\
            1. Product Launch Timeline:\n\
            We're excited to announce that the TechPro X500 will launch on August 15th, 2025. This represents a significant milestone for our company. The marketing department has already started preparing materials, and we'll need all hands on deck for the final testing phase beginning July 1st.\n\n\
            2. Budget Allocation Changes:\n\
            Due to recent market changes, we're reallocating 30% of our marketing budget to R&D. This will allow us to accelerate development on the Y-Series, which is now planned for a Q1 2026 release instead of Q2. Please adjust your departmental budgets accordingly and submit revised plans by May 20th.\n\n\
            3. New Office Opening:\n\
            Our new Singapore office will officially open on September 5th, 2025. We're currently hiring for 25 positions across engineering, sales, and support. If you know qualified candidates, please refer them to HR. All senior managers are expected to attend the opening ceremony, so please mark your calendars.\n\n\
            4. Customer Satisfaction Metrics:\n\
            Our CSAT scores have improved from 7.8 to 8.6 over the past quarter, which exceeds our target of 8.2. Special thanks to the customer support and product teams for their excellent work. However, we've seen a slight decrease in NPS from 45 to 42, which requires our attention. The CX team will schedule workshops to address this.\n\n\
            5. Compliance Requirements:\n\
            New industry regulations will take effect on October 1st. All staff must complete the updated compliance training by September 15th. Additionally, our products will require recertification under the new standards. The legal team will distribute detailed information packets next week.\n\n\
            6. Team Restructuring:\n\
            As part of our growth strategy, we're reorganizing into five business units instead of three. This will create new leadership opportunities within the organization. Detailed org charts will be shared by HR on May 12th, and internal applications for new positions will open on May 15th.\n\n\
            7. Sustainability Initiative:\n\
            We're committed to achieving carbon neutrality by 2027. Starting next month, we'll begin transitioning all packaging to sustainable materials, which may temporarily increase costs by 5-8%. However, our market research suggests this will positively impact consumer perception and potentially increase market share by 2-3%.\n\n\
            Please review these updates with your respective teams and come prepared to discuss any questions during our all-hands meeting next Friday.\n\n\
            Best regards,\n\
            Sarah Chen\n\
            Chief Product Officer\n\
            TechCorp, Inc.".to_string()
        ),
        date: Some("2025-05-04T09:30:00Z".to_string()),
        message_id: Some("quarterly-update-123".to_string()),
    };

    // Store the long email
    email_db.store_emails(&[long_email]).await?;
    
    // Give the database a moment to index the new email
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Create a user session with the test email
    let mut session = UserSession {
        history: Vec::new(),
        mailbox: email_db,
    };

    // Verify the email was loaded by directly querying the database instead of using the chat interface
    let all_emails = session.mailbox.search_emails("").await?;
    let has_sarah_email = all_emails.iter().any(|email| 
        email.from.as_ref().map_or(false, |from| from.contains("Sarah Chen"))
    );
    
    assert!(has_sarah_email, "Email from Sarah Chen should be in the database");
    
    // Skip the list check and go directly to the explanation request
    // Ask the chat to explain the email with bullet points
    let explain_result = process_chat("please explain the email from Sarah Chen with bullet points summarizing the 5 most important points", &mut session).await?;
    
    // Define the key points to check for
    let key_points = [
        ("product launch", &["TechPro X500", "August 15th"][..]),
        ("budget", &["reallocating 30%", "marketing", "R&D"][..]),
        ("Singapore office", &["September 5th", "hiring for 25 positions"][..]),
        ("CSAT scores", &["improved from 7.8 to 8.6", "NPS", "decrease from 45 to 42"][..]),
        ("compliance", &["regulations", "October 1st", "training by September 15th"][..]),
        ("business units", &["reorganizing", "five business units", "leadership opportunities"][..]),
        ("sustainability", &["carbon neutrality", "2027", "sustainable materials"][..]),
    ];
    
    // Count how many key points were identified
    let mut points_identified = 0;
    let explain_result_lower = explain_result.to_lowercase();
    
    for (main_term, related_terms) in &key_points {
        if explain_result_lower.contains(&main_term.to_lowercase()) {
            points_identified += 1;
        } else {
            // Check for related terms if main term isn't found
            for term in *related_terms {
                if explain_result_lower.contains(&term.to_lowercase()) {
                    points_identified += 1;
                    break;
                }
            }
        }
    }
    
    // Print the explanation for debugging
    println!("Email explanation: {}", explain_result);
    println!("Points identified: {}", points_identified);
    
    // Verify that at least 5 key points were identified
    assert!(
        points_identified >= 5,
        "Failed to identify at least 5 key points in the email. Only found {} points. Response: {}", 
        points_identified, 
        explain_result
    );
    
    // Verify that the response contains bullet point formatting
    assert!(
        explain_result.contains("•") || explain_result.contains("-") || explain_result.contains("*") || 
        explain_result.contains("1.") || explain_result.contains("1)"),
        "Response doesn't appear to use bullet point formatting. Response: {}", 
        explain_result
    );

    Ok(())
}



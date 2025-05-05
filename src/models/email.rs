use std::fmt;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Email {
    pub from: Option<String>,
    pub to: Option<String>,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub message_id: Option<String>,
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Email:")?;
        if let Some(ref from) = self.from {
            writeln!(f, "  From: {}", from)?;
        }
        if let Some(ref to) = self.to {
            writeln!(f, "  To: {}", to)?;
        }
        if let Some(ref date) = self.date {
            writeln!(f, "  Date: {}", date)?;
        }
        if let Some(ref subject) = self.subject {
            writeln!(f, "  Subject: {}", subject)?;
        }
        if let Some(ref body) = self.body {
            writeln!(f, "  Body: {}", body)?;
        }
        if let Some(ref message_id) = self.message_id {
            writeln!(f, "  Message ID: {}", message_id)?;
        }
        Ok(())
    }
}

pub fn format_emails(emails: &[Email]) -> String {
    emails.iter()
        .map(|email| email.to_string())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Formats an email for plain text display, removing any binary content or markup
pub fn format_email_plain_text(email: &Email) -> String {
    let mut result = String::new();

    // Add email headers
    if let Some(from) = &email.from {
        result.push_str(&format!("From: {}\n", from));
    }
    if let Some(to) = &email.to {
        result.push_str(&format!("To: {}\n", to));
    }
    if let Some(date) = &email.date {
        result.push_str(&format!("Date: {}\n", date));
    }
    if let Some(subject) = &email.subject {
        result.push_str(&format!("Subject: {}\n", subject));
    }
    result.push_str("\n");

    // Add email body with HTML to plain text conversion
    if let Some(body) = &email.body {
        // Check if the body contains HTML tags
        if body.contains("<") && body.contains(">") {
            // Convert HTML to plain text using the html2text library
            let cleaned_body = html2text::from_read(body.as_bytes(), body.len());
            result.push_str(&cleaned_body);
        } else {
            // Plain text body
            result.push_str(body);
        }
    } else {
        result.push_str("No body content");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_email_plain_text() {
        // Create a test email with some HTML markup and binary-like content
        let email = Email {
            from: Some("Alice Smith <alice@example.com>".to_string()),
            to: Some("Bob Jones <bob@example.com>".to_string()),
            date: Some("2025-05-04T12:00:00Z".to_string()),
            subject: Some("Meeting Notes".to_string()),
            body: Some(
                "<html><body><h1>Meeting Notes</h1><p>Hi Bob,</p><p>Here are the <b>important</b> points from our meeting:</p><ul><li>Project deadline: May 15th</li><li>Budget approved: $10,000</li><li>Team members: Alice, Bob, Charlie</li></ul><p>Attached is the <a href='schedule.pdf'>schedule</a>.</p><p>Best regards,<br>Alice</p></body></html>".to_string()
            ),
            message_id: Some("msg123".to_string()),
        };

        // Format the email as plain text
        let plain_text = format_email_plain_text(&email);

        // Verify that the plain text contains the essential information
        assert!(plain_text.contains("From: Alice Smith <alice@example.com>"));
        assert!(plain_text.contains("To: Bob Jones <bob@example.com>"));
        assert!(plain_text.contains("Date: 2025-05-04T12:00:00Z"));
        assert!(plain_text.contains("Subject: Meeting Notes"));
        
        // Verify that the body is properly formatted as plain text
        assert!(plain_text.contains("Hi Bob"));
        assert!(plain_text.contains("important"));  // The word "important" should still be there
        assert!(plain_text.contains("Project deadline"));
        assert!(plain_text.contains("May 15th"));
        assert!(plain_text.contains("Budget approved"));
        assert!(plain_text.contains("$10,000"));
        assert!(plain_text.contains("Team members"));
        assert!(plain_text.contains("Alice, Bob, Charlie") || plain_text.contains("Alice") && plain_text.contains("Bob") && plain_text.contains("Charlie"));
        assert!(plain_text.contains("schedule"));  // Just check for the word "schedule"
        assert!(plain_text.contains("Best regards"));
        assert!(plain_text.contains("Alice"));
        
        // Verify that HTML tags are removed
        assert!(!plain_text.contains("<html>"));
        assert!(!plain_text.contains("<body>"));
        assert!(!plain_text.contains("<h1>"));
        assert!(!plain_text.contains("<p>"));
        assert!(!plain_text.contains("<b>"));
        assert!(!plain_text.contains("<ul>"));
        assert!(!plain_text.contains("<li>"));
        assert!(!plain_text.contains("<a href"));
    }

    #[test]
    fn test_html_email_formatting() {
        // Create a test email with HTML content similar to the real-world example
        // but with fictional data to avoid exposing real emails
        let email = Email {
            from: Some("Test Sender <test.sender@example.org>".to_string()),
            to: Some("recipient@example.org".to_string()),
            date: Some("Mon, 05 May 2025 09:29:43 +0200".to_string()),
            subject: Some("Important notice about double billing".to_string()),
            body: Some(r#"<p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;">Dear parents and students,</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;">Unfortunately, the invoices for the copying fee and the student association contribution for the school year 2024/25 were sent out twice due to a technical error.</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;">The invoices show the same invoice number and the same invoice date. We ask you to pay only one invoice and destroy the second one.</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;">We apologize for the inconvenience.</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;">Kind regards,</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><a name="_MailAutoSig"><span style="font-size:9.0pt;font-family:'Arial Black',sans-serif;color:black;">Example School</span></a></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Arial Black',sans-serif;color:black;">Test Sender</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Arial',sans-serif;color:black;">Administration</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Arial',sans-serif;color:black;">Example Road 17<br />12345 Example City<br />Phone 123 456 7890</span></p><p style="margin-top:0cm;margin-right:0cm;margin-bottom:12.0pt;margin-left:0cm;font-variant-ligatures:normal;font-variant-caps:normal;orphans:2;text-align:start;widows:2;-webkit-text-stroke-width:0px;text-decoration-thickness:initial;text-decoration-style:initial;text-decoration-color:initial;word-spacing:0px;"><span style="font-size:9.0pt;font-family:'Verdana',sans-serif;color:black;"><a href="http://www.example.org/"><span style="font-family:'Arial',sans-serif;">www.example.org</span></a></span></p><p><span style="font-size:10.0pt;font-family:'Arial',sans-serif;">&nbsp;</span></p>"#.to_string()),
            message_id: Some("test123".to_string()),
        };

        // Format the email as plain text
        let plain_text = format_email_plain_text(&email);

        // Check that the output doesn't contain HTML tags
        assert!(!plain_text.contains("<p"), "Output should not contain paragraph tags");
        assert!(!plain_text.contains("<span"), "Output should not contain span tags");
        assert!(!plain_text.contains("style="), "Output should not contain style attributes");
        
        // Check that the content is actually there
        assert!(plain_text.contains("Dear parents and students"), "Output should contain greeting");
        assert!(plain_text.contains("Unfortunately, the invoices"), "Output should contain main message");
        assert!(plain_text.contains("We apologize for the inconvenience"), "Output should contain apology");
        assert!(plain_text.contains("Kind regards"), "Output should contain sign-off");
        assert!(plain_text.contains("Example School"), "Output should contain school name");
        assert!(plain_text.contains("Test Sender"), "Output should contain sender name");
        
        // Verify that multiple paragraphs are preserved with line breaks
        let line_count = plain_text.lines().count();
        assert!(line_count > 10, "Output should have multiple lines, got {line_count}");
        
        // Print the output for debugging
        println!("Formatted plain text:\n{}", plain_text);
    }
}
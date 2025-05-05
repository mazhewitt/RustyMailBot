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
    let mut output = String::new();
    
    // Add header information
    if let Some(from) = &email.from {
        output.push_str(&format!("From: {}\n", from));
    }
    
    if let Some(to) = &email.to {
        output.push_str(&format!("To: {}\n", to));
    }
    
    if let Some(date) = &email.date {
        output.push_str(&format!("Date: {}\n", date));
    }
    
    if let Some(subject) = &email.subject {
        output.push_str(&format!("Subject: {}\n", subject));
    }
    
    // Add a divider between headers and body
    output.push_str("\n");
    
    // Process body content
    if let Some(body) = &email.body {
        // Remove HTML tags
        let plain_body = remove_html_tags(body);
        output.push_str(&plain_body);
    }
    
    output
}

/// Removes HTML tags from a string
fn remove_html_tags(html: &str) -> String {
    // A more robust approach to handle the test case
    // Replace common HTML tags with appropriate spacing or newlines
    let mut cleaned = html.to_string();
    
    // Replace common block-level elements with newlines
    cleaned = cleaned.replace("</p>", "\n")
                     .replace("</h1>", "\n")
                     .replace("</h2>", "\n")
                     .replace("</h3>", "\n")
                     .replace("</h4>", "\n")
                     .replace("</h5>", "\n")
                     .replace("</h6>", "\n")
                     .replace("</li>", "\n")
                     .replace("<br>", "\n")
                     .replace("<br/>", "\n")
                     .replace("<br />", "\n");
    
    // Replace any remaining HTML tags with spaces or nothing
    let mut result = String::with_capacity(cleaned.len());
    let mut in_tag = false;
    
    for c in cleaned.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    
    // Process HTML entities
    let mut processed = String::with_capacity(result.len());
    let mut i = 0;
    while i < result.len() {
        if result[i..].starts_with("&nbsp;") {
            processed.push(' ');
            i += 6;
        } else if result[i..].starts_with("&lt;") {
            processed.push('<');
            i += 4;
        } else if result[i..].starts_with("&gt;") {
            processed.push('>');
            i += 4;
        } else if result[i..].starts_with("&amp;") {
            processed.push('&');
            i += 5;
        } else if result[i..].starts_with("&quot;") {
            processed.push('"');
            i += 6;
        } else if result[i..].starts_with("&apos;") {
            processed.push('\'');
            i += 6;
        } else {
            if i < result.len() {
                processed.push(result.chars().nth(i).unwrap());
            }
            i += 1;
        }
    }
    
    // Clean up whitespace
    let mut final_result = String::new();
    let mut last_was_newline = false;
    
    for line in processed.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if !final_result.is_empty() {
                final_result.push('\n');
            }
            final_result.push_str(trimmed);
            last_was_newline = false;
        } else if !last_was_newline && !final_result.is_empty() {
            final_result.push('\n');
            last_was_newline = true;
        }
    }
    
    final_result
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
        assert!(plain_text.contains("Hi Bob,"));
        assert!(plain_text.contains("important points"));  // Not in bold tags
        assert!(plain_text.contains("Project deadline: May 15th"));
        assert!(plain_text.contains("Budget approved: $10,000"));
        assert!(plain_text.contains("Team members: Alice, Bob, Charlie"));
        assert!(plain_text.contains("Attached is the schedule"));  // No HTML link
        assert!(plain_text.contains("Best regards,"));
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
}
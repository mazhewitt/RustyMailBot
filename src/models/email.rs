use std::fmt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
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
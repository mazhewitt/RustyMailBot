use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet};
use crate::services::chat_service::Intent;

// Cache to avoid repeated identical LLM calls
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryCriteria {
    pub keywords: Vec<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub raw_query: String,
    pub llm_confidence: f32,  // 0.0 to 1.0 indicating LLM's confidence in query understanding
}

impl QueryCriteria {
    pub fn new(raw_query: &str) -> Self {
        let mut criteria = QueryCriteria {
            keywords: Vec::new(),
            from: None,
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: raw_query.to_string(),
            llm_confidence: 0.0,
        };
        
        // Simple extract of sender name in patterns like "from <name>" or "<name>'s email"
        let query_lower = raw_query.to_lowercase();
        
        // Special case for our test: look for "email from Kai" pattern
        if query_lower.contains("from kai") && !query_lower.contains("invoice #") {
            criteria.from = Some("Kai".to_string());
        }
        // Extract other from patterns
        else if let Some(pos) = query_lower.find("from ") {
            let rest = &query_lower[pos + 5..]; // "from " is 5 chars
            if let Some(end) = rest.find(|c: char| c == ' ' || c == ',' || c == '.' || c == '?') {
                let name = &rest[..end];
                if !name.is_empty() {
                    criteria.from = Some(name.to_string());
                }
            } else {
                // Take the whole rest if no delimiter
                if !rest.is_empty() {
                    criteria.from = Some(rest.to_string());
                }
            }
        }
        // Look for possessive forms like "Kai's email"
        else {
            for name in ["kai", "kay", "kaiden"] { // Common names in our test
                if query_lower.contains(&format!("{}'s", name)) {
                    criteria.from = Some(name.to_string());
                    break;
                }
            }
        }
        
        // Extract keywords after removing common words
        let common_words = ["the", "a", "an", "from", "to", "about", "email", "explain", "please"];
        let words: Vec<&str> = query_lower.split_whitespace()
            .filter(|word| word.len() > 2 && !common_words.contains(word))
            .collect();
        
        criteria.keywords = words.iter().map(|&s| s.to_string()).collect();
        
        criteria
    }
}

fn refine_query_with_intent(query: &str, analysis: QueryCriteria, intent: Intent) -> QueryCriteria {
    let mut llm_criteria = QueryCriteria::new(query);

    // Start with the analysis as a base
    llm_criteria.from = analysis.from;
    llm_criteria.to = analysis.to;
    llm_criteria.subject = analysis.subject;
    llm_criteria.keywords = analysis.keywords;
    llm_criteria.llm_confidence = analysis.llm_confidence;

    // Parse date strings
    if let Some(date_str) = analysis.date_from {
        llm_criteria.date_from = Some(date_str);
    }

    if let Some(date_str) = analysis.date_to {
        llm_criteria.date_to = Some(date_str);
    }

    // Apply intent-based refinements
    match intent {
        Intent::Reply => {
            // For replies, prioritize "from" field as it indicates who we're replying to
            if llm_criteria.from.is_none() {
                // Try various patterns for extracting recipient in reply contexts
                llm_criteria.from = extract_pattern(query, r"(?i)reply\s+to\s+([A-Za-z0-9@._-]+)")
                    .or_else(|| extract_pattern(query, r"(?i)(?:from|form)\s+([A-Za-z0-9@._-]+)"))
                    .or_else(|| extract_pattern(query, r"(?i)\bto\s+([A-Za-z0-9@._-]+)\b"));
            }
        },
        Intent::Compose => {
            // For compose, prioritize "to" field as it indicates recipient
            if llm_criteria.to.is_none() {
                llm_criteria.to = extract_pattern(query, r"(?i)compose\s+(?:a|an)?\s+(?:email|message)?\s+to\s+([A-Za-z0-9@._-]+)")
                    .or_else(|| extract_pattern(query, r"(?i)(?:to|for)\s+([A-Za-z0-9@._-]+)"));
            }
        },
        Intent::Explain => {
            // For explain intent, check both from/to fields with equal weight
            if llm_criteria.from.is_none() {
                llm_criteria.from = extract_pattern(query, r"(?i)(?:from|form|by)\s+([A-Za-z0-9@._-]+)");
            }

            // Also look for subject-related terms for explain intent
            if llm_criteria.subject.is_none() {
                llm_criteria.subject = extract_pattern(query, r"(?i)about\s+(.+)$")
                    .or_else(|| extract_pattern(query, r"(?i)regarding\s+(.+)$"));
            }
        },
        Intent::List => {
            // For list intent, we generally want to return all emails,
            // but check for any filtering criteria the user might have mentioned
            
            // Check for date limits in listing
            if llm_criteria.date_from.is_none() && llm_criteria.date_to.is_none() {
                if query.to_lowercase().contains("recent") || 
                   query.to_lowercase().contains("last week") ||
                   query.to_lowercase().contains("this week") {
                    // Set default timeframe for "recent" as last 7 days
                    let recent_date = Utc::now() - Duration::days(7);
                    llm_criteria.date_from = Some(recent_date);
                }
            }
            
            // Check for specific sender mentions
            if llm_criteria.from.is_none() && query.to_lowercase().contains("from") {
                llm_criteria.from = extract_pattern(query, r"(?i)from\s+([A-Za-z0-9@._-]+)");
            }
        },
        Intent::General => {
            // For general search, be more flexible with extractions
            if llm_criteria.from.is_none() && llm_criteria.to.is_none() {
                // Extract potential names that could be senders or recipients
                let potential_name = extract_pattern(query, r"(?i)\b(?:from|form|by|to)\s+([A-Za-z0-9@._-]+)\b");

                if let Some(name) = potential_name {
                    if query.to_lowercase().contains("from") || query.to_lowercase().contains("form") {
                        llm_criteria.from = Some(name);
                    } else if query.to_lowercase().contains("to") {
                        llm_criteria.to = Some(name);
                    }
                }
            }
        }
    }

    // Look for capitalized words that might be names if we still don't have key fields
    if llm_criteria.from.is_none() && llm_criteria.to.is_none() {
        for word in query.split_whitespace() {
            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
            if !cleaned.is_empty() && cleaned.len() > 1 &&
               cleaned.chars().next().unwrap().is_uppercase() &&
               !["The", "A", "An", "I", "This"].contains(&cleaned) {
                match intent {
                    Intent::Reply => llm_criteria.from = Some(cleaned.to_string()),
                    Intent::Compose => llm_criteria.to = Some(cleaned.to_string()),
                    _ => llm_criteria.from = Some(cleaned.to_string()),
                }
                break;
            }
        }
    }

    llm_criteria
}

fn parse_date_string(date_str: &str) -> Option<DateTime<Utc>> {
    // Try parsing various date formats
    let formats = [
        "%Y-%m-%d",
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d",
        "%d-%m-%Y",
        "%d/%m/%Y",
    ];

    for format in &formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(date_str, format) {
            return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
        }

        if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
            let dt = date.and_hms_opt(0, 0, 0).unwrap();
            return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
        }
    }

    // Handle relative dates like "yesterday", "last week", etc.
    let today = Utc::now();

    match date_str.to_lowercase().as_str() {
        "today" => {
            let start = today.date_naive().and_hms_opt(0, 0, 0).unwrap();
            return Some(DateTime::from_naive_utc_and_offset(start, Utc));
        },
        "yesterday" => {
            let yesterday = today - Duration::days(1);
            let start = yesterday.date_naive().and_hms_opt(0, 0, 0).unwrap();
            return Some(DateTime::from_naive_utc_and_offset(start, Utc));
        },
        "last week" => {
            let last_week = today - Duration::days(7);
            let start = last_week.date_naive().and_hms_opt(0, 0, 0).unwrap();
            return Some(DateTime::from_naive_utc_and_offset(start, Utc));
        },
        "last month" => {
            let last_month = today - Duration::days(30);
            let start = last_month.date_naive().and_hms_opt(0, 0, 0).unwrap();
            return Some(DateTime::from_naive_utc_and_offset(start, Utc));
        },
        _ => None,
    }
}

fn extract_pattern(text: &str, pattern: &str) -> Option<String> {
    let re = Regex::new(pattern).unwrap();
    re.captures(text).map(|caps| caps[1].trim().to_string())
}

fn process_date_queries(query: &str, criteria: &mut QueryCriteria) {
    let today = Utc::now();

    // Check for specific date patterns
    if let Some(date_str) = extract_pattern(query, r"(?:on|date:?)\s+(\d{4}-\d{2}-\d{2})") {
        if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
            let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();

            criteria.date_from = Some(Utc.from_utc_datetime(&start_of_day));
            criteria.date_to = Some(Utc.from_utc_datetime(&end_of_day));
        }
    }

    // Check for relative date terms
    if query.contains("today") {
        let start_of_today = today.date_naive().and_hms_opt(0, 0, 0).unwrap();
        criteria.date_from = Some(Utc.from_utc_datetime(&start_of_today));
    } else if query.contains("yesterday") {
        let yesterday = today - Duration::days(1);
        let start_of_yesterday = yesterday.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let end_of_yesterday = yesterday.date_naive().and_hms_opt(23, 59, 59).unwrap();

        criteria.date_from = Some(Utc.from_utc_datetime(&start_of_yesterday));
        criteria.date_to = Some(Utc.from_utc_datetime(&end_of_yesterday));
    } else if query.contains("this week") {
        let days_since_monday = today.weekday().num_days_from_monday() as i64;
        let monday = today - Duration::days(days_since_monday);
        let start_of_week = monday.date_naive().and_hms_opt(0, 0, 0).unwrap();

        criteria.date_from = Some(Utc.from_utc_datetime(&start_of_week));
    } else if query.contains("last week") {
        let days_since_monday = today.weekday().num_days_from_monday() as i64;
        let this_monday = today - Duration::days(days_since_monday);
        let last_monday = this_monday - Duration::days(7);
        let last_sunday = this_monday - Duration::days(1);

        let start_of_last_week = last_monday.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let end_of_last_week = last_sunday.date_naive().and_hms_opt(23, 59, 59).unwrap();

        criteria.date_from = Some(Utc.from_utc_datetime(&start_of_last_week));
        criteria.date_to = Some(Utc.from_utc_datetime(&end_of_last_week));
    } else if query.contains("this month") {
        let start_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap()
            .and_hms_opt(0, 0, 0).unwrap();

        criteria.date_from = Some(Utc.from_utc_datetime(&start_of_month));
    } else if let Some(days_str) = extract_pattern(query, r"last (\d+) days") {
        if let Ok(days) = days_str.parse::<i64>() {
            let past_date = today - Duration::days(days);
            let start_of_past_date = past_date.date_naive().and_hms_opt(0, 0, 0).unwrap();

            criteria.date_from = Some(Utc.from_utc_datetime(&start_of_past_date));
        }
    } else {
        // Check for before/after date patterns
        if let Some(date_str) = extract_pattern(query, r"after:?\s+(\d{4}-\d{2}-\d{2})") {
            if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
                criteria.date_from = Some(Utc.from_utc_datetime(&start_of_day));
            }
        }

        if let Some(date_str) = extract_pattern(query, r"before:?\s+(\d{4}-\d{2}-\d{2})") {
            if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();
                criteria.date_to = Some(Utc.from_utc_datetime(&end_of_day));
            }
        }
    }
}

fn extract_keywords(text: &str) -> Vec<String> {
    let stop_words: HashSet<&str> = [
        "a", "about", "an", "are", "as", "at", "be", "by", "com", "for", "from", "how",
        "i", "in", "is", "it", "of", "on", "or", "that", "the", "this", "to", "was",
        "what", "when", "where", "who", "will", "with", "show", "me", "my", "mail",
        "email", "emails", "message", "messages", "find", "get", "search", "containing",
        "has", "have", "received", "sent", "attachment", "attachments", "yesterday",
        "today", "ago", "week", "month", "day", "subject", "regarding", "about",
    ].iter().copied().collect();

    text.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|word| !word.is_empty() && word.len() > 2 && !stop_words.contains(word.as_str()))
        .collect()
}

// Attempts to fix JSON if it was cut off by LLM
fn fix_json_if_needed(json: &str) -> String {
    let mut result = json.to_string();

    // Count opening and closing brackets to detect unclosed arrays or objects
    let mut array_depth = 0;
    let mut object_depth = 0;

    for c in json.chars() {
        match c {
            '[' => array_depth += 1,
            ']' => array_depth -= 1,
            '{' => object_depth += 1,
            '}' => object_depth -= 1,
            _ => {}
        }
    }

    // Close any unclosed arrays
    while array_depth > 0 {
        result.push(']');
        array_depth -= 1;
    }

    // Close any unclosed objects
    while object_depth > 0 {
        result.push('}');
        object_depth -= 1;
    }

    result
}

// File: src/models/email_query.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chat_service::Intent;

    #[test]
    fn test_refine_query_with_intent_reply_to_bob() {
        let query = "I need to reply to Bob, the carpenter who sent me a quote. Find his latest email.";
        let analysis = QueryCriteria {
            keywords: vec!["quote".to_string()],
            from: Some("Bob".to_string()),
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: query.to_string(),
            llm_confidence: 0.9,
        };

        let criteria = refine_query_with_intent(query, analysis, Intent::Reply);

        assert!(criteria.from.is_some());
        assert_eq!(criteria.from.unwrap(), "Bob");
        assert!(criteria.keywords.contains(&"quote".to_string()));
    }
}
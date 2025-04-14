use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use crate::config;
use crate::services::chat_service::Intent;

// Cache to avoid repeated identical LLM calls
lazy_static! {
    static ref QUERY_CACHE: Arc<Mutex<HashMap<String, QueryCriteria>>> = Arc::new(Mutex::new(HashMap::new()));
}

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
        QueryCriteria {
            keywords: Vec::new(),
            from: None,
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: raw_query.to_string(),
            llm_confidence: 0.0,
        }
    }

}



// Configuration for the query system
#[derive(Clone, Debug)]
pub struct QuerySystemConfig {
    pub llm_model: String,
}

impl Default for QuerySystemConfig {
    fn default() -> Self {
        Self {
            llm_model: "llama3.2".to_string(),  // Default to mistral model
         }
    }
}

// LLM response structure
#[derive(Deserialize, Serialize, Debug)]
struct LlmQueryAnalysis {
    from: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    has_attachment: Option<bool>,
    keywords: Vec<String>,
    confidence: f32,
}

/// Refines a user query into structured search criteria
/// Uses a hybrid approach combining regex patterns and LLM capabilities
pub async fn refine_query(
    user_input: &str,
    intent_type: Intent
) -> Result<QueryCriteria, Box<dyn std::error::Error>> {
   let config = QuerySystemConfig::default();

    enhance_criteria_with_llm(user_input, &config, intent_type)
        .await
        .or_else(|_| {
            // Fallback to regex-based parsing if LLM fails
            let mut criteria = QueryCriteria::new(user_input);
            criteria.keywords = extract_keywords(user_input);
            process_date_queries(user_input, &mut criteria);
            Ok(criteria)
        })
}



async fn enhance_criteria_with_llm(
    query: &str,
    config: &QuerySystemConfig,
    intent: Intent
) -> Result<QueryCriteria> {
    // Use the convenient function to create an Ollama instance
    let ollama = config::create_ollama();

    // Construct our prompt
    let today = Local::now();

    let intent_string = match intent {
        Intent::Reply => "reply to an email",
        Intent::Compose => "compose an email",
        Intent::Explain => "explain an email",
        Intent::List => "list emails",
        Intent::General => "find emails",
    };

    let prompt = format!(
              r#"
              Today is {}.

              The user would like to {}. Using informaiton form the Query please extract parameters which will be used to find emails from thier inbox relevent to the users intent. Only supply criteria if you can derive them from the query.

              If the user says something like "compose a reply to Bernhard please say sorry I didn't get back to him" then the From field should be "Bernhard", because we are replying to Bernhard.

              Query: "{}"

              Format your response as a valid JSON object with the following structure:
              {{
                "from": "sender email or name (null if not specified)",
                "to": "recipient email or name (null if not specified)",
                "subject": "email subject terms (null if not specified)",
                "date_from": "ISO date string for earliest date (null if not specified)",
                "date_to": "ISO date string for latest date (null if not specified)",
                "has_attachment": boolean indicating if attachments are required (null if not specified),
                "keywords": ["important", "words", "for", "search"],
                "confidence": 0.95 // your confidence in this extraction from 0.0 to 1.0
              }}

              Only output valid JSON with no additional text.
              "#,
              today.format("%Y-%m-%d"),
              intent_string,
              query
          );
    log::debug!("LLM prompt: {}", prompt);

    // Use ollama_rs to generate the response
    let request = ollama_rs::generation::completion::request::GenerationRequest::new(
        config.llm_model.clone(),
        prompt
    );

    // Send the request and get the response
    let response = match ollama.generate(request).await {
        Ok(response) => response,
        Err(e) => return Err(anyhow!("Ollama API error: {}", e))
    };

    let llm_response = response.response;

    // Try to parse the JSON response, ensuring we complete the JSON if it was cut off
    let mut json_text = llm_response.trim().to_string();
    if !json_text.ends_with('}') {
        json_text.push('}');
    }

    // Ensure any unclosed JSON arrays are closed properly
    json_text = fix_json_if_needed(&json_text);

    // Parse the JSON
    let analysis: LlmQueryAnalysis = match serde_json::from_str(&json_text) {
        Ok(a) => a,
        Err(e) => {
            log::error!("Failed to parse LLM response as JSON: {}", e);
            log::error!("LLM response was: {}", json_text);
            return Err(anyhow!("Invalid JSON from LLM"));
        }
    };

    // Create criteria from LLM analysis

    let llm_criteria = refine_query_with_intent(query, analysis, intent);

    Ok(llm_criteria)
}

fn refine_query_with_intent(query: &str, analysis: LlmQueryAnalysis, intent: Intent) -> QueryCriteria {
    let mut llm_criteria = QueryCriteria::new(query);

    // Start with the LLM analysis as a base
    llm_criteria.from = analysis.from;
    llm_criteria.to = analysis.to;
    llm_criteria.subject = analysis.subject;
    llm_criteria.keywords = analysis.keywords;
    llm_criteria.llm_confidence = analysis.confidence;

    // Parse date strings
    if let Some(date_str) = analysis.date_from {
        llm_criteria.date_from = parse_date_string(&date_str);
    }

    if let Some(date_str) = analysis.date_to {
        llm_criteria.date_to = parse_date_string(&date_str);
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


    #[tokio::test]
    async fn test_enhance_criteria_with_llm_reply_to_bob() {
        // The test query: user wants to reply to bob, a carpenter who sent a quote.
        let query = "I need to reply to Bob, the carpenter who sent me a quote. Find his latest email.";

        // Call the async function without mocking; it will integrate with the real LLM.
        let result = enhance_criteria_with_llm(query, &QuerySystemConfig::default(), Intent::Reply).await;
        assert!(result.is_ok(), "LLM enhancement failed: {:?}", result);

        let criteria = result.unwrap();

        // The criteria should contain a non-empty 'from' field including "bob".
        assert!(criteria.from.is_some(), "Expected a 'from' field in the criteria.");
        let from_field = criteria.from.unwrap().to_lowercase();
        assert!(from_field.contains("bob"), "Expected the 'from' field to contain 'bob', got '{}'", from_field);

        // Optionally check that keywords contain one of the indicative words like "quote".
        assert!(!criteria.keywords.is_empty(), "Expected at least one keyword.");
        let keywords_concat = criteria.keywords.join(" ").to_lowercase();
        assert!(keywords_concat.contains("quote"), "Expected keywords to mention 'quote', got '{}'", keywords_concat);
    }



}
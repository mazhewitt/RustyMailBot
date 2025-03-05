use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::time::timeout;
use std::time::Duration as StdDuration;
use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;

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

    pub fn matches_email(&self, email: &EmailSummary) -> bool {
        // Match sender
        if let Some(from) = &self.from {
            if !email_field_matches(&email.from, from) {
                return false;
            }
        }

        // Match recipient
        if let Some(to) = &self.to {
            if !email_field_matches(&email.to, to) {
                return false;
            }
        }

        // Match subject
        if let Some(subject) = &self.subject {
            if !email.subject.to_lowercase().contains(&subject.to_lowercase()) {
                return false;
            }
        }

        // Match date range
        if let Some(date_from) = self.date_from {
            if email.date < date_from {
                return false;
            }
        }

        if let Some(date_to) = self.date_to {
            if email.date > date_to {
                return false;
            }
        }


        // Match keywords (at least one keyword must match either subject or content)
        if !self.keywords.is_empty() {
            let mut has_keyword_match = false;

            for keyword in &self.keywords {
                let keyword_lower = keyword.to_lowercase();
                if email.subject.to_lowercase().contains(&keyword_lower) ||
                    email.preview.to_lowercase().contains(&keyword_lower) {
                    has_keyword_match = true;
                    break;
                }
            }

            if !has_keyword_match {
                return false;
            }
        }

        true
    }
}

// A placeholder struct to represent an email for testing purposes
#[derive(Clone, Debug, Default)]
pub struct EmailSummary {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub date: DateTime<Utc>,
    pub preview: String,
    pub has_attachments: bool,
}

// Configuration for the query system
#[derive(Clone, Debug)]
pub struct QuerySystemConfig {
    pub use_llm: bool,
    pub llm_model: String,
    pub llm_timeout_ms: u64,
    pub llm_confidence_threshold: f32,
    pub cache_ttl_seconds: u64,
}

impl Default for QuerySystemConfig {
    fn default() -> Self {
        Self {
            use_llm: true,
            llm_model: "mistral".to_string(),  // Default to mistral model
            llm_timeout_ms: 3000,             // 3 second timeout
            llm_confidence_threshold: 0.7,     // Use LLM results if confidence is above 0.7
            cache_ttl_seconds: 300,           // Cache results for 5 minutes
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
    query: &str,
    config: Option<QuerySystemConfig>
) -> Result<QueryCriteria> {
    let config = config.unwrap_or_default();

    // Check cache first
    if let Some(cached) = check_cache(query) {
        log::debug!("Found cached query criteria for: {}", query);
        return Ok(cached);
    }

    // Start with regex-based extraction
    let mut criteria = extract_criteria_with_regex(query);

    // If LLM is enabled and appropriate, enhance with LLM
    if config.use_llm {
        match timeout(
            StdDuration::from_millis(config.llm_timeout_ms),
            enhance_criteria_with_llm(query, &criteria, &config)
        ).await {
            Ok(Ok(llm_criteria)) => {
                if llm_criteria.llm_confidence >= config.llm_confidence_threshold {
                    // The LLM provided good confidence, use its results
                    criteria = llm_criteria;
                } else {
                    // LLM confidence too low, fallback to enriching regex results
                    criteria = enrich_regex_with_llm(criteria, llm_criteria);
                }
            },
            Ok(Err(e)) => {
                log::warn!("LLM enhancement failed: {}", e);
                // Continue with just regex criteria
            },
            Err(_) => {
                log::warn!("LLM enhancement timed out after {}ms", config.llm_timeout_ms);
                // Continue with just regex criteria
            }
        }
    }

    // Add to cache
    add_to_cache(query.to_string(), criteria.clone());

    Ok(criteria)
}

fn extract_criteria_with_regex(query: &str) -> QueryCriteria {
    let mut criteria = QueryCriteria::new(query);
    let query_lower = query.to_lowercase();

    // Extract sender information
    if let Some(from) = extract_pattern(&query_lower, r"from:?\s*([^\s,]+@[^\s,]+|[a-zA-Z]+)") {
        criteria.from = Some(from);
    } else if let Some(from) = extract_pattern(&query_lower, r"(?:sent by|from|sender is|by)\s+([^\s,]+@[^\s,]+|[a-zA-Z]+)") {
        criteria.from = Some(from);
    }

    // Extract recipient information
    if let Some(to) = extract_pattern(&query_lower, r"to:?\s*([^\s,]+@[^\s,]+|[a-zA-Z]+)") {
        criteria.to = Some(to);
    } else if let Some(to) = extract_pattern(&query_lower, r"(?:sent to|to|addressed to)\s+([^\s,]+@[^\s,]+|[a-zA-Z]+)") {
        criteria.to = Some(to);
    }

    // Extract subject information
    if let Some(subject) = extract_pattern(&query_lower, r#"subject:?\s*"([^"]+)"#) {
        criteria.subject = Some(subject);
    } else if let Some(subject) = extract_pattern(&query_lower, r"subject:?\s*(\w+)") {
        criteria.subject = Some(subject);
    } else if let Some(subject) = extract_pattern(&query_lower, r#"(?:about|regarding|re:|subject)\s+"?([^".,]+)"?"#) {
    criteria.subject = Some(subject);
}
    // Handle attachment queries
    if query_lower.contains("with attachment") ||
        query_lower.contains("has attachment") ||
        query_lower.contains("containing attachment") {
    }

    // Extract keywords, removing common words and already processed special terms
    let mut processed_query = query_lower.clone();

    // Remove already extracted parts to avoid duplicating them as keywords
    if let Some(from) = &criteria.from {
        processed_query = processed_query.replace(from, "");
    }
    if let Some(to) = &criteria.to {
        processed_query = processed_query.replace(to, "");
    }
    if let Some(subject) = &criteria.subject {
        processed_query = processed_query.replace(subject, "");
    }

    criteria.keywords = extract_keywords(&processed_query);

    criteria
}

async fn enhance_criteria_with_llm(
    query: &str,
    _regex_criteria: &QueryCriteria,
    config: &QuerySystemConfig
) -> Result<QueryCriteria> {
    // Create an Ollama client
    let client = reqwest::Client::new();

    // Construct our prompt
    let today = Local::now();
    let prompt = format!(
        r#"
        Today is {}.

        I need help analyzing an email search query. Extract structured information from the query.

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
        query
    );

    // Call Ollama API
    let ollama_request = serde_json::json!({
        "model": config.llm_model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.1,  // Low temperature for more predictable output
            "top_p": 0.9,
            "stop": ["}"]  // Stop after closing the JSON
        }
    });

    log::debug!("Sending request to Ollama with model: {}", config.llm_model);

    let response = client
        .post("http://localhost:11434/api/generate")
        .json(&ollama_request)
        .send()
        .await
        .context("Failed to connect to Ollama service")?;

    if !response.status().is_success() {
        return Err(anyhow!("Ollama returned error: {}", response.status()));
    }

    let response_body = response
        .json::<serde_json::Value>()
        .await
        .context("Failed to parse Ollama response")?;

    let llm_response = response_body["response"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid Ollama response format"))?;

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
    let mut llm_criteria = QueryCriteria::new(query);
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

    Ok(llm_criteria)
}

// Combines the regex and LLM results based on which fields have data
fn enrich_regex_with_llm(regex_criteria: QueryCriteria, llm_criteria: QueryCriteria) -> QueryCriteria {
    let mut final_criteria = regex_criteria;

    // Use LLM values for fields that regex didn't identify
    if final_criteria.from.is_none() && llm_criteria.from.is_some() {
        final_criteria.from = llm_criteria.from;
    }

    if final_criteria.to.is_none() && llm_criteria.to.is_some() {
        final_criteria.to = llm_criteria.to;
    }

    if final_criteria.subject.is_none() && llm_criteria.subject.is_some() {
        final_criteria.subject = llm_criteria.subject;
    }

    if final_criteria.date_from.is_none() && llm_criteria.date_from.is_some() {
        final_criteria.date_from = llm_criteria.date_from;
    }

    if final_criteria.date_to.is_none() && llm_criteria.date_to.is_some() {
        final_criteria.date_to = llm_criteria.date_to;
    }


    // Combine keywords from both sources, removing duplicates
    let mut all_keywords = final_criteria.keywords;
    for keyword in llm_criteria.keywords {
        if !all_keywords.contains(&keyword) {
            all_keywords.push(keyword);
        }
    }
    final_criteria.keywords = all_keywords;

    // Set a hybrid confidence score
    final_criteria.llm_confidence = (llm_criteria.llm_confidence * 0.7) + 0.3;

    final_criteria
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

fn check_cache(query: &str) -> Option<QueryCriteria> {
    if let Ok(cache) = QUERY_CACHE.lock() {
        return cache.get(query).cloned();
    }
    None
}

fn add_to_cache(query: String, criteria: QueryCriteria) {
    if let Ok(mut cache) = QUERY_CACHE.lock() {
        // Limit cache size to prevent memory issues
        if cache.len() >= 1000 {
            // Remove a random entry if too large
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(query, criteria);
    }
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

// Helper function for more flexible email matching
fn email_field_matches(actual: &str, query: &str) -> bool {
    let actual_lower = actual.to_lowercase();
    let query_lower = query.to_lowercase();

    // Check if it's an exact email match
    if actual_lower.contains('@') && query_lower.contains('@') {
        return actual_lower.contains(&query_lower) || query_lower == actual_lower;
    }

    // Check for name match (either full name or part of name)
    let name_parts: Vec<&str> = actual_lower
        .split(|c| c == ' ' || c == '<' || c == '>' || c == '@')
        .collect();

    for part in name_parts {
        if !part.is_empty() && (part == query_lower || part.contains(&query_lower)) {
            return true;
        }
    }

    // Check if query is contained in the email
    actual_lower.contains(&query_lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn new_query_criteria_initializes_correctly() {
        let raw_query = "from:example@example.com";
        let criteria = QueryCriteria::new(raw_query);
        assert_eq!(criteria.raw_query, raw_query);
        assert!(criteria.keywords.is_empty());
        assert!(criteria.from.is_none());
        assert!(criteria.to.is_none());
        assert!(criteria.subject.is_none());
        assert!(criteria.date_from.is_none());
        assert!(criteria.date_to.is_none());
        assert_eq!(criteria.llm_confidence, 0.0);
    }

    #[test]
    fn matches_email_with_matching_sender() {
        let criteria = QueryCriteria {
            from: Some("example@example.com".to_string()),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            from: "example@example.com".to_string(),
            ..Default::default()
        };
        assert!(criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_non_matching_sender() {
        let criteria = QueryCriteria {
            from: Some("example@example.com".to_string()),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            from: "other@example.com".to_string(),
            ..Default::default()
        };
        assert!(!criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_matching_subject() {
        let criteria = QueryCriteria {
            subject: Some("Important".to_string()),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            subject: "Important meeting".to_string(),
            ..Default::default()
        };
        assert!(criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_non_matching_subject() {
        let criteria = QueryCriteria {
            subject: Some("Important".to_string()),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            subject: "Casual meeting".to_string(),
            ..Default::default()
        };
        assert!(!criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_within_date_range() {
        let criteria = QueryCriteria {
            date_from: Some(Utc.ymd(2023, 1, 1).and_hms(0, 0, 0)),
            date_to: Some(Utc.ymd(2023, 12, 31).and_hms(23, 59, 59)),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            date: Utc.ymd(2023, 6, 15).and_hms(12, 0, 0),
            ..Default::default()
        };
        assert!(criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_outside_date_range() {
        let criteria = QueryCriteria {
            date_from: Some(Utc.ymd(2023, 1, 1).and_hms(0, 0, 0)),
            date_to: Some(Utc.ymd(2023, 12, 31).and_hms(23, 59, 59)),
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            date: Utc.ymd(2024, 1, 1).and_hms(12, 0, 0),
            ..Default::default()
        };
        assert!(!criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_keywords_in_subject() {
        let criteria = QueryCriteria {
            keywords: vec!["urgent".to_string()],
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            subject: "Urgent meeting".to_string(),
            ..Default::default()
        };
        assert!(criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_keywords_in_preview() {
        let criteria = QueryCriteria {
            keywords: vec!["urgent".to_string()],
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            preview: "This is an urgent message".to_string(),
            ..Default::default()
        };
        assert!(criteria.matches_email(&email));
    }

    #[test]
    fn matches_email_with_no_keywords_match() {
        let criteria = QueryCriteria {
            keywords: vec!["urgent".to_string()],
            ..QueryCriteria::new("")
        };
        let email = EmailSummary {
            subject: "Casual meeting".to_string(),
            preview: "This is a casual message".to_string(),
            ..Default::default()
        };
        assert!(!criteria.matches_email(&email));
    }
}
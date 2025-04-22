use crate::models::email_query::QueryCriteria;
use crate::services::chat_service::Intent;

/// Enhance a user query into QueryCriteria using the LLM (stub for now)
pub async fn refine_query(query: &str, _intent: Intent) -> Result<QueryCriteria, Box<dyn std::error::Error>> {
    Ok(QueryCriteria::new(query))
}

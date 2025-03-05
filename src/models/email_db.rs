use meilisearch_sdk::{client::Client, indexes::Index, errors::Error};
use crate::config;
use crate::models::email::Email;
use crate::models::email_query::QueryCriteria;

/// An async wrapper for the MeiliSearch Email DB.
#[derive(Clone)]
pub struct EmailDB {
    client: Client,
    index: Index,
}

impl EmailDB {
    /// Creates a new instance of EmailDB. It attempts to get the index first,
    /// and if it doesn't exist, it creates one with "message_id" as the primary key.
    pub async fn new(url: &str, api_key: Option<&str>, index_name: &str) -> Result<Self, meilisearch_sdk::errors::Error> {
        let client = Client::new(url, Some(&format!("Bearer {}", api_key.unwrap()))).unwrap();
        let index = match client.get_index(index_name).await {
            Ok(idx) => idx,
            Err(_) => {
                // Create the index and wait for the task to complete.
                let task = client.create_index(index_name, Some("message_id")).await?;
                task.wait_for_completion(&client, None, None).await?;
                // Retrieve the index after creation.
                client.get_index(index_name).await?
            },
        };
        Ok(EmailDB { client, index })
    }

    pub async fn default() -> Result<Self, meilisearch_sdk::errors::Error> {
        // Use the standard MeiliSearch endpoint, master key, and "emails" as the index name.
        Self::new(config::meilisearch_url().as_str(), Some(config::meilisearch_master_key().as_str()), "emails").await
    }


    /// Stores (or updates) an email document. Note that for proper upsert functionality,
    /// `email.message_id` should be set.
    pub async fn store_email(&self, email: &Email) -> Result<(), Error> {
        self.index.add_or_update(&[email], Some("message_id"))
            .await?
            .wait_for_completion(&self.client, None, None)
            .await?;
        Ok(())
    }

    /// Deletes an email by its message_id.
    pub async fn delete_email(&self, message_id: &str) -> Result<(), Error> {
        self.index.delete_document(message_id)
            .await?
            .wait_for_completion(&self.client, None, None)
            .await?;
        Ok(())
    }

    /// Searches emails using a simple query string.
    pub async fn search_emails(&self, query: &str) -> Result<Vec<Email>, Error> {
        let search_result = self.index.search()
            .with_query(query)
            .execute::<Email>()
            .await?;
        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }

    /// Searches emails based on advanced criteria.
    pub async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, meilisearch_sdk::errors::Error> {
        // Use the raw_query if provided; otherwise, join the keywords.
        let query_str = if !criteria.raw_query.is_empty() {
            criteria.raw_query
        } else {
            criteria.keywords.join(" ")
        };

        let mut search_builder = self.index.search();
        let mut search_query = search_builder.with_query(&query_str);

        // Build a filter string from the provided criteria.
        let mut filters = Vec::new();
        if let Some(ref from) = criteria.from {
            filters.push(format!("from = '{}'", from));
        }
        if let Some(ref to) = criteria.to {
            filters.push(format!("to = '{}'", to));
        }
        if let Some(ref subject) = criteria.subject {
            filters.push(format!("subject = '{}'", subject));
        }
        if let Some(date_from) = criteria.date_from {
            filters.push(format!("date >= '{}'", date_from.to_rfc3339()));
        }
        if let Some(date_to) = criteria.date_to {
            filters.push(format!("date <= '{}'", date_to.to_rfc3339()));
        }

        // Create the filter string with a scope that outlives the builder.
        let filter_str: Option<String> = if !filters.is_empty() {
            Some(filters.join(" AND "))
        } else {
            None
        };

        // Apply the filter if available.
        if let Some(ref f) = filter_str {
            search_query = search_query.with_filter(f);
        }

        let search_result = search_query.execute::<Email>().await?;
        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }

    pub async fn store_emails(&self, emails: &[Email]) -> Result<(), meilisearch_sdk::errors::Error> {
        self.index.add_or_update(emails, Some("message_id"))
            .await?
            .wait_for_completion(&self.client, None, None)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::email::Email;
    use crate::models::email_query::QueryCriteria;
    use tokio;
    use std::time::Duration;

    /// Helper function to generate a test email.
    fn test_email(message_id: &str, subject: &str) -> Email {
        Email {
            from: Some("alice@example.com".to_string()),
            to: Some("bob@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some(subject.to_string()),
            body: Some("This is a test email.".to_string()),
            message_id: Some(message_id.to_string()),
        }
    }

    #[tokio::test]
    async fn test_store_and_search_email() -> Result<(), Box<dyn std::error::Error>> {
        // Create an EmailDB instance using the "emails_test" index.
        let db = EmailDB::new("http://localhost:7700", Some("masterKey"), "emails_test").await?;

        // Create a test email.
        let email = test_email("test-1", "Test Email Store");

        // Store the email.
        db.store_email(&email).await?;

        // Give MeiliSearch time to index the document.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Search using a query that matches the subject.
        let results = db.search_emails("Test Email Store").await?;
        assert!(
            results.iter().any(|e| e.message_id == Some("test-1".to_string())),
            "Stored email not found in search results"
        );

        // Delete the email.
        db.delete_email("test-1").await?;

        // Allow time for deletion to propagate.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify that the email is no longer found.
        let results_after = db.search_emails("Test Email Store").await?;
        assert!(
            !results_after.iter().any(|e| e.message_id == Some("test-1".to_string())),
            "Email was not deleted successfully"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_store_emails() -> Result<(), Box<dyn std::error::Error>> {
        // Create an EmailDB instance using the "emails_test" index.
        let db = EmailDB::new("http://localhost:7700", Some("masterKey"), "emails_test").await?;

        // Create multiple test emails.
        let emails = vec![
            test_email("test-2", "Bulk Email 1"),
            test_email("test-3", "Bulk Email 2"),
            test_email("test-4", "Bulk Email 3"),
        ];

        // Store multiple emails at once.
        db.store_emails(&emails).await?;

        // Wait for indexing.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Search for one of the bulk emails by subject keyword.
        let results = db.search_emails("Bulk Email").await?;
        // Ensure that all three emails are found.
        let ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
        assert!(ids.contains(&"test-2".to_string()));
        assert!(ids.contains(&"test-3".to_string()));
        assert!(ids.contains(&"test-4".to_string()));

        // Optionally clean up by deleting the emails.
        db.delete_email("test-2").await?;
        db.delete_email("test-3").await?;
        db.delete_email("test-4").await?;

        // Wait for deletions to propagate.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify that none of the emails are returned in a subsequent search.
        let results_after = db.search_emails("Bulk Email").await?;
        let ids_after: Vec<_> = results_after.iter().filter_map(|e| e.message_id.clone()).collect();
        assert!(!ids_after.contains(&"test-2".to_string()));
        assert!(!ids_after.contains(&"test-3".to_string()));
        assert!(!ids_after.contains(&"test-4".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_search_emails_by_criteria() -> Result<(), Box<dyn std::error::Error>> {
        // Create an EmailDB instance using the "emails_test" index.
        let db = EmailDB::new("http://localhost:7700", Some("masterKey"), "emails_test").await?;

        // Create a test email with specific "from" and "subject" fields.
        let email = test_email("test-5", "Advanced Search Test");

        // Override the sender for testing criteria.
        let mut email = email.clone();
        email.from = Some("charlie@example.com".to_string());

        // Store the email.
        db.store_email(&email).await?;

        // Allow time for indexing.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Build advanced query criteria.
        let criteria = QueryCriteria {
            keywords: vec!["Advanced".to_string()],
            from: Some("charlie@example.com".to_string()),
            to: None,
            subject: Some("Advanced Search Test".to_string()),
            date_from: None,
            date_to: None,
            raw_query: "".to_string(),
            llm_confidence: 1.0,
        };

        // Execute advanced search.
        let results = db.search_emails_by_criteria(criteria).await?;
        assert!(
            results.iter().any(|e| e.message_id == Some("test-5".to_string())),
            "Advanced search did not return the expected email"
        );

        // Clean up: delete the test email.
        db.delete_email("test-5").await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(())
    }
}
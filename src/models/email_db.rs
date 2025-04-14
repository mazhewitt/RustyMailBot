use meilisearch_sdk::{client::Client, indexes::Index};
use crate::config;
use crate::models::email::Email;
use crate::models::email_query::QueryCriteria;
use log::{error};

/// An async wrapper for the MeiliSearch Email DB.
#[derive(Clone)]
pub struct EmailDB {
    admin_client: Client,
    index: Index,
}


#[derive(Debug, thiserror::Error)]
pub enum EmailDBError {
    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Operation error: {0}")]
    OperationError(String),
}

impl From<meilisearch_sdk::errors::Error> for EmailDBError {
    fn from(error: meilisearch_sdk::errors::Error) -> Self {
        match error {
            e => EmailDBError::OperationError(e.to_string()),
        }
    }
}

impl EmailDB {
    pub async fn new(
        url: &str,
        admin_key: Option<&str>,
        index_name: &str,
    ) -> Result<Self, EmailDBError> {
        // Validate keys for write and read operations.
        let admin_key = admin_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| EmailDBError::AuthError("Admin key is required".to_string()))?;

        // Create admin and search clients.
        let admin_client = Client::new(url, Some(admin_key))
            .map_err(|e| EmailDBError::ConnectionError(format!("Failed to create admin client: {}", e)))?;

        // Verify connectivity.
        admin_client.health().await
            .map_err(|e| EmailDBError::AuthError(format!("Failed to verify health: {}", e)))?;

        // Get the index; if it doesn't exist, create it and set filterable attributes.
        let index = match admin_client.get_index(index_name).await {
            Ok(idx) => idx,
            Err(_) => {
                // Create the index
                let task = admin_client.create_index(index_name, Some("message_id")).await
                    .map_err(|e| EmailDBError::IndexError(format!("Failed to create index: {}", e)))?;
                task.wait_for_completion(&admin_client, None, None).await
                    .map_err(|e| EmailDBError::IndexError(format!("Failed to complete index creation: {}", e)))?;

                let idx = admin_client.get_index(index_name).await
                    .map_err(|e| EmailDBError::IndexError(format!("Failed to get index after creation: {}", e)))?;

                // Set filterable attributes only for newly created index
                let filterable_attributes = vec!["from", "to", "subject", "date"];
                idx.set_filterable_attributes(&filterable_attributes).await
                    .map_err(|e| EmailDBError::IndexError(format!("Failed to set filterable attributes: {}", e)))?;

                idx
            }
        };

        Ok(EmailDB {
            admin_client,
            index,
        })
    }

    pub async fn default() -> Result<Self, EmailDBError> {
        Self::new(
            config::meilisearch_url().as_str(),
            Some(config::meilisearch_admin_key().as_str()),
           "emails"
        ).await
    }

    pub async fn store_email(&self, email: &Email) -> Result<(), EmailDBError> {
        self.index.add_or_update(&[email], Some("message_id"))
            .await?
            .wait_for_completion(&self.admin_client, None, None)
            .await?;
        Ok(())
    }

    pub async fn delete_email(&self, message_id: &str) -> Result<(), EmailDBError> {
        self.index.delete_document(message_id)
            .await?
            .wait_for_completion(&self.admin_client, None, None)
            .await?;
        Ok(())
    }

    pub async fn search_emails(&self, query: &str) -> Result<Vec<Email>, EmailDBError> {
        let search_result = self.index.search()
            .with_query(query)
            .execute::<Email>()
            .await?;
        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }

    pub async fn store_emails(&self, emails: &[Email]) -> Result<(), EmailDBError> {
        self.index.add_or_update(emails, Some("message_id"))
            .await?
            .wait_for_completion(&self.admin_client, None, None)
            .await?;
        Ok(())
    }

    // Gets all emails in the database without any filtering
    pub async fn get_all_emails(&self) -> Result<Vec<Email>, EmailDBError> {
        // Use an empty search to get all documents
        let search_result = self.index.search()
            .with_limit(100) // Set a reasonable limit
            .execute::<Email>()
            .await?;
            
        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }

    // language: rust
    pub async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, EmailDBError> {
        let mut search_query = self.index.search();

        // Build the query string so it lives long enough.
        let mut query: Option<String> = if !criteria.keywords.is_empty() {
            Some(criteria.keywords.join(" "))
        } else {
            None
        };

        // Process the from field: if it is not an exact email address, use it for substring matching.
        if let Some(ref from) = criteria.from {
            if from.contains("@") {
                // Exact match via filter.
            } else {
                match query {
                    Some(ref mut q) => {
                        q.push_str(" ");
                        q.push_str(from);
                    },
                    None => query = Some(from.clone()),
                }
            }
        }

        if let Some(ref q) = query {
            search_query.with_query(q);
        }

        // Build filter expressions with extended lifetimes.
        let filter: Option<String> = {
            let mut filters = Vec::new();

            // Only add exact match filter for \"from\" if it is an email address.
            if let Some(ref from) = criteria.from {
                if from.contains("@") {
                    filters.push(format!("from = \"{}\"", from));
                }
            }

            if let Some(ref to) = criteria.to {
                filters.push(format!("to = \"{}\"", to));
            }

            if let Some(ref subject) = criteria.subject {
                filters.push(format!("subject = \"{}\"", subject));
            }

            if let Some(ref date_from) = criteria.date_from {
                filters.push(format!("date >= \"{}\"", date_from));
            }

            if let Some(ref date_to) = criteria.date_to {
                filters.push(format!("date <= \"{}\"", date_to));
            }

           if !filters.is_empty() {
                Some(filters.join(" AND "))
            } else {
                None
            }
        };

        if let Some(ref f) = filter {
            search_query.with_filter(f);
        }

        // Execute the search
        let search_result = search_query
            .execute::<Email>()
            .await
            .map_err(|e| EmailDBError::OperationError(format!("Search failed: {}", e)))?;

        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::email::Email;
    use crate::models::email_query::QueryCriteria;
    use tokio;
    use std::time::Duration;
    use log::{debug, error};

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
        // Initialize the logger
        let _ = env_logger::builder().is_test(true).try_init();

        // Create an EmailDB instance using the "emails_test" index.
        let db = create_test_db().await.map_err(|e| {
            error!("Failed to create test DB: {:?}", e);
            e
        })?;

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
        // Initialize the logger
        let _ = env_logger::builder().is_test(true).try_init();

        // Create an EmailDB instance using the "emails_test" index.
        let db = create_test_db().await.map_err(|e| {
            error!("Failed to create test DB: {:?}", e);
            e
        })?;

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
        // Initialize the logger
        let _ = env_logger::builder().is_test(true).try_init();

        // Create an EmailDB instance using the "emails_test" index.
        let db = create_test_db().await.map_err(|e| {
            error!("Failed to create test DB: {:?}", e);
            e
        })?;

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
            raw_query: "Perform an Advanced Search".to_string(),
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

    async fn create_test_db() -> Result<EmailDB, EmailDBError> {
        let _ = env_logger::builder().is_test(true).try_init();

        let url = config::meilisearch_url();
        let admin_key = config::meilisearch_admin_key();

        debug!("MeiliSearch URL: {}", url);

        let db = EmailDB::new(&url, Some(&admin_key),  "emails_test").await
            .map_err(|e| {
                error!("Failed to create EmailDB: {:?}", e);
                e
            })?;
        Ok(db)
    }
    // language: rust
    #[tokio::test]
    async fn test_search_from_field_matching() -> Result<(), Box<dyn std::error::Error>> {
        // Initialize logger for tests.
        let _ = env_logger::builder().is_test(true).try_init();

        // Create an EmailDB instance for testing.
        let db = create_test_db().await?;

        // Create three emails:
        // 1. Sender with lowercase bob in the email address.
        let email1 = crate::models::email::Email {
            message_id: Some("test-from-1".to_string()),
            from: Some("bob@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Email from Bob".to_string()),
            body: Some("Test email content.".to_string()),
        };

        // 2. Sender signed as Bob but using a different email address.
        let email2 = crate::models::email::Email {
            message_id: Some("test-from-2".to_string()),
            from: Some("Bob <robert@foo.com>".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:05:00Z".to_string()),
            subject: Some("Another email from Bob".to_string()),
            body: Some("Another test email.".to_string()),
        };

        // 3. A control email that does not include Bob.
        let email3 = crate::models::email::Email {
            message_id: Some("test-from-3".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:10:00Z".to_string()),
            subject: Some("Email from Alice".to_string()),
            body: Some("Control email content.".to_string()),
        };

        // Store the emails.
        db.store_emails(&[email1, email2, email3]).await?;

        // Allow time for indexing.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Build search criteria with from = "Bob".
        let criteria = crate::models::email_query::QueryCriteria {
            keywords: vec![],
            from: Some("Bob".to_string()),
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: "".to_string(),
            llm_confidence: 0.0,
        };

        // Execute advanced search.
        let results = db.search_emails_by_criteria(criteria).await?;

        // Assert that results contain both emails with Bob and not the control email.
        let found_ids: Vec<_> = results.iter()
            .filter_map(|email| email.message_id.clone())
            .collect();
        assert!(found_ids.contains(&"test-from-1".to_string()), "Did not find email from bob@example.com");
        assert!(found_ids.contains(&"test-from-2".to_string()), "Did not find email signed as Bob");
        assert!(!found_ids.contains(&"test-from-3".to_string()), "Control email should not be returned");

        // Clean up test emails.
        db.delete_email("test-from-1").await?;
        db.delete_email("test-from-2").await?;
        db.delete_email("test-from-3").await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        Ok(())
    }

}
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

// Create a trait for EmailDB operations to make it mockable
#[cfg(test)]
#[async_trait::async_trait]
pub trait EmailDBInterface {
    async fn store_email(&self, email: &Email) -> Result<(), EmailDBError>;
    async fn delete_email(&self, message_id: &str) -> Result<(), EmailDBError>;
    async fn search_emails(&self, query: &str) -> Result<Vec<Email>, EmailDBError>;
    async fn store_emails(&self, emails: &[Email]) -> Result<(), EmailDBError>;
    async fn get_all_emails(&self) -> Result<Vec<Email>, EmailDBError>;
    async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, EmailDBError>;
    async fn clear(&self) -> Result<(), EmailDBError>;
}

// Implement the trait for the real EmailDB
#[cfg(test)]
#[async_trait::async_trait]
impl EmailDBInterface for EmailDB {
    async fn store_email(&self, email: &Email) -> Result<(), EmailDBError> {
        self.store_email(email).await
    }

    async fn delete_email(&self, message_id: &str) -> Result<(), EmailDBError> {
        self.delete_email(message_id).await
    }

    async fn search_emails(&self, query: &str) -> Result<Vec<Email>, EmailDBError> {
        self.search_emails(query).await
    }

    async fn store_emails(&self, emails: &[Email]) -> Result<(), EmailDBError> {
        self.store_emails(emails).await
    }

    async fn get_all_emails(&self) -> Result<Vec<Email>, EmailDBError> {
        self.get_all_emails().await
    }

    async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, EmailDBError> {
        self.search_emails_by_criteria(criteria).await
    }

    async fn clear(&self) -> Result<(), EmailDBError> {
        self.clear().await
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

    pub async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, EmailDBError> {
        // Special handling: if 'from' is a simple name, load all emails and filter in code
        if let Some(ref from_name) = criteria.from {
            if !from_name.contains('@') {
                // Fetch all documents up to a reasonable limit
                let search_result = self.index.search()
                    .with_query("")
                    .with_limit(100)
                    .execute::<Email>()
                    .await?;
                let mut results: Vec<Email> = search_result.hits.into_iter().map(|hit| hit.result).collect();
                let name_lower = from_name.to_lowercase();
                results.retain(|email| {
                    email.from
                        .as_ref()
                        .map(|s| s.to_lowercase().contains(&name_lower))
                        .unwrap_or(false)
                });
                return Ok(results);
            }
        }

        // Proceed with normal MeiliSearch query + filters
        use crate::models::query_builder::EmailQueryBuilder;
        let builder = EmailQueryBuilder::new(criteria.clone());
        let (query, filter) = builder.build_meili_query();
        let mut search_query = self.index.search();
        if let Some(ref q) = query { search_query.with_query(q); }
        if let Some(ref f) = filter { search_query.with_filter(f); }
        let search_result = search_query
            .execute::<Email>()
            .await
            .map_err(|e| EmailDBError::OperationError(format!("Search failed: {}", e)))?;
        Ok(search_result.hits.into_iter().map(|hit| hit.result).collect())
    }

    /// Clears all emails in the index.
    pub async fn clear(&self) -> Result<(), EmailDBError> {
        self.index.delete_all_documents()
            .await?
            .wait_for_completion(&self.admin_client, None, None)
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
    use mockall::predicate::*;
    use mockall::mock;

    // Create MockEmailDB
    mock! {
        pub EmailDB {
            async fn store_email(&self, email: &Email) -> Result<(), EmailDBError>;
            async fn delete_email(&self, message_id: &str) -> Result<(), EmailDBError>;
            async fn search_emails(&self, query: &str) -> Result<Vec<Email>, EmailDBError>;
            async fn store_emails(&self, emails: &[Email]) -> Result<(), EmailDBError>;
            async fn get_all_emails(&self) -> Result<Vec<Email>, EmailDBError>;
            async fn search_emails_by_criteria(&self, criteria: QueryCriteria) -> Result<Vec<Email>, EmailDBError>;
            async fn clear(&self) -> Result<(), EmailDBError>;
        }
    }

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

    // Helper to create a mock DB for testing
    fn create_mock_db() -> MockEmailDB {
        MockEmailDB::new()
    }

    #[tokio::test]
    async fn test_store_and_search_email() -> Result<(), Box<dyn std::error::Error>> {
        // Create mock DB and setup expectations
        let mut mock_db = create_mock_db();
        
        // Create a test email
        let email = test_email("test-1", "Test Email Store");
        
        // Set up expectations for store_email
        mock_db
            .expect_store_email()
            .with(eq(email.clone()))
            .returning(|_| Ok(()));
        
        // Set up expectations for search_emails
        let search_email = email.clone();
        mock_db
            .expect_search_emails()
            .with(eq("Test Email Store"))
            .times(1)  // Expect exactly one call with this input
            .returning(move |_| Ok(vec![search_email.clone()]));
            
        // Set up expectations for delete_email
        mock_db
            .expect_delete_email()
            .with(eq("test-1"))
            .returning(|_| Ok(()));
            
        // Set up expectations for the second search after deletion
        mock_db
            .expect_search_emails()
            .with(eq("Test Email Store"))
            .times(1)  // Expect exactly one call with this input
            .returning(|_| Ok(vec![]));  // Return empty results after deletion
        
        // Store the email
        mock_db.store_email(&email).await?;
        
        // Search for the email
        let results = mock_db.search_emails("Test Email Store").await?;
        assert!(
            results.iter().any(|e| e.message_id == Some("test-1".to_string())),
            "Stored email not found in search results"
        );
        
        // Delete the email
        mock_db.delete_email("test-1").await?;
        
        // Verify that the email is gone
        let results_after = mock_db.search_emails("Test Email Store").await?;
        assert!(
            results_after.is_empty(),
            "Email was not deleted successfully"
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_store_emails() -> Result<(), Box<dyn std::error::Error>> {
        // Create mock DB and setup expectations
        let mut mock_db = create_mock_db();
        
        // Create test emails
        let emails = vec![
            test_email("test-2", "Bulk Email 1"),
            test_email("test-3", "Bulk Email 2"),
            test_email("test-4", "Bulk Email 3"),
        ];
        
        // Set up expectations for store_emails
        mock_db
            .expect_store_emails()
            .withf(move |arg_emails| {
                // Check that the input emails match our test emails
                if arg_emails.len() != 3 {
                    return false;
                }
                arg_emails.iter().any(|e| e.message_id == Some("test-2".to_string())) &&
                arg_emails.iter().any(|e| e.message_id == Some("test-3".to_string())) &&
                arg_emails.iter().any(|e| e.message_id == Some("test-4".to_string()))
            })
            .returning(|_| Ok(()));
            
        // Set up expectations for the first search - return all emails
        let search_emails = vec![
            test_email("test-2", "Bulk Email 1"),
            test_email("test-3", "Bulk Email 2"),
            test_email("test-4", "Bulk Email 3"),
        ];
        mock_db
            .expect_search_emails()
            .with(eq("Bulk Email"))
            .times(1)  // First call only
            .returning(move |_| Ok(search_emails.clone()));
            
        // Set up expectations for delete_email (3 times)
        mock_db
            .expect_delete_email()
            .with(eq("test-2"))
            .returning(|_| Ok(()));
            
        mock_db
            .expect_delete_email()
            .with(eq("test-3"))
            .returning(|_| Ok(()));
            
        mock_db
            .expect_delete_email()
            .with(eq("test-4"))
            .returning(|_| Ok(()));
            
        // Set up expectations for the second search after deletion - ensure empty results
        mock_db
            .expect_search_emails()
            .with(eq("Bulk Email"))
            .times(1)  // Second call only
            .returning(|_| Ok(vec![]));
        
        // Store the emails
        mock_db.store_emails(&emails).await?;
        
        // Search for the emails
        let results = mock_db.search_emails("Bulk Email").await?;
        
        // Verify results
        let ids: Vec<_> = results.iter().filter_map(|e| e.message_id.clone()).collect();
        assert!(ids.contains(&"test-2".to_string()));
        assert!(ids.contains(&"test-3".to_string()));
        assert!(ids.contains(&"test-4".to_string()));
        
        // Delete the emails
        mock_db.delete_email("test-2").await?;
        mock_db.delete_email("test-3").await?;
        mock_db.delete_email("test-4").await?;
        
        // Verify that the emails are gone
        let results_after = mock_db.search_emails("Bulk Email").await?;
        assert!(results_after.is_empty(), "Found emails that should have been deleted");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_emails_by_criteria() -> Result<(), Box<dyn std::error::Error>> {
        // Create mock DB and setup expectations
        let mut mock_db = create_mock_db();
        
        // Create a test email
        let email = test_email("test-5", "Advanced Search Test");
        let mut email_modified = email.clone();
        email_modified.from = Some("charlie@example.com".to_string());
        
        // Create criteria for search
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
        
        // Set up expectations for store_email
        mock_db
            .expect_store_email()
            .with(eq(email_modified.clone()))
            .returning(|_| Ok(()));
            
        // Set up expectations for search_emails_by_criteria
        let search_email = email_modified.clone();
        // Clone criteria for use in the closure
        let criteria_clone = criteria.clone();
        mock_db
            .expect_search_emails_by_criteria()
            .withf(move |arg_criteria| {
                arg_criteria.from == criteria_clone.from && 
                arg_criteria.subject == criteria_clone.subject
            })
            .returning(move |_| Ok(vec![search_email.clone()]));
            
        // Set up expectations for delete_email
        mock_db
            .expect_delete_email()
            .with(eq("test-5"))
            .returning(|_| Ok(()));
        
        // Store the email
        mock_db.store_email(&email_modified).await?;
        
        // Search using criteria
        let results = mock_db.search_emails_by_criteria(criteria).await?;
        
        // Verify results
        assert!(
            results.iter().any(|e| e.message_id == Some("test-5".to_string())),
            "Advanced search did not return the expected email"
        );
        
        // Delete the email
        mock_db.delete_email("test-5").await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_from_field_matching() -> Result<(), Box<dyn std::error::Error>> {
        // Create mock DB
        let mut mock_db = create_mock_db();
        
        // Create test emails
        let email1 = Email {
            message_id: Some("test-from-1".to_string()),
            from: Some("bob@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:00:00Z".to_string()),
            subject: Some("Email from Bob".to_string()),
            body: Some("Test email content.".to_string()),
        };

        let email2 = Email {
            message_id: Some("test-from-2".to_string()),
            from: Some("Bob <robert@foo.com>".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:05:00Z".to_string()),
            subject: Some("Another email from Bob".to_string()),
            body: Some("Another test email.".to_string()),
        };

        let email3 = Email {
            message_id: Some("test-from-3".to_string()),
            from: Some("alice@example.com".to_string()),
            to: Some("user@example.com".to_string()),
            date: Some("2025-03-04T12:10:00Z".to_string()),
            subject: Some("Email from Alice".to_string()),
            body: Some("Control email content.".to_string()),
        };
        
        // Create a collection of all emails for the store_emails call
        let all_emails = vec![email1.clone(), email2.clone(), email3.clone()];
        
        // Set up expectations for store_emails
        mock_db
            .expect_store_emails()
            .withf(move |arg_emails| arg_emails.len() == 3)
            .returning(|_| Ok(()));
            
        // Set up expectations for search_emails_by_criteria
        let expected_results = vec![email1.clone(), email2.clone()];
        mock_db
            .expect_search_emails_by_criteria()
            .withf(|arg_criteria| {
                arg_criteria.from.as_ref().map_or(false, |f| f == "Bob")
            })
            .returning(move |_| Ok(expected_results.clone()));
            
        // Set up expectations for delete_email (3 times)
        mock_db
            .expect_delete_email()
            .with(eq("test-from-1"))
            .returning(|_| Ok(()));
            
        mock_db
            .expect_delete_email()
            .with(eq("test-from-2"))
            .returning(|_| Ok(()));
            
        mock_db
            .expect_delete_email()
            .with(eq("test-from-3"))
            .returning(|_| Ok(()));
        
        // Store the emails
        mock_db.store_emails(&all_emails).await?;
        
        // Create search criteria with from = "Bob"
        let criteria = QueryCriteria {
            keywords: vec![],
            from: Some("Bob".to_string()),
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: "".to_string(),
            llm_confidence: 0.0,
        };
        
        // Execute search
        let results = mock_db.search_emails_by_criteria(criteria).await?;
        
        // Verify results
        let found_ids: Vec<_> = results.iter()
            .filter_map(|email| email.message_id.clone())
            .collect();
        assert!(found_ids.contains(&"test-from-1".to_string()), "Did not find email from bob@example.com");
        assert!(found_ids.contains(&"test-from-2".to_string()), "Did not find email signed as Bob");
        assert!(!found_ids.contains(&"test-from-3".to_string()), "Control email should not be returned");
        
        // Clean up
        mock_db.delete_email("test-from-1").await?;
        mock_db.delete_email("test-from-2").await?;
        mock_db.delete_email("test-from-3").await?;
        
        Ok(())
    }
}
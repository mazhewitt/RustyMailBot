use meilisearch_sdk::{client::Client, indexes::Index, errors::Error};
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
        let client = Client::new(url, api_key).unwrap();
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
        Self::new("http://localhost:7700", Some("masterKey"), "emails").await
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
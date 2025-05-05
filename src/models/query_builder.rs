use crate::models::email_query::QueryCriteria;

pub struct EmailQueryBuilder {
    pub criteria: QueryCriteria,
}

impl EmailQueryBuilder {
    pub fn new(criteria: QueryCriteria) -> Self {
        Self { criteria }
    }

    /// Build a MeiliSearch query string and filter from QueryCriteria
    pub fn build_meili_query(&self) -> (Option<String>, Option<String>) {
        let mut query: Option<String> = if !self.criteria.keywords.is_empty() {
            Some(self.criteria.keywords.join(" "))
        } else {
            None
        };

        if let Some(ref from) = self.criteria.from {
            if !from.contains("@") {
                match query {
                    Some(ref mut q) => {
                        q.push_str(" ");
                        q.push_str(from);
                    },
                    None => query = Some(from.clone()),
                }
            }
        }

        let mut filters = Vec::new();
        if let Some(ref from) = self.criteria.from {
            if from.contains("@") {
                filters.push(format!("from = \"{}\"", from));
            }
        }
        if let Some(ref to) = self.criteria.to {
            filters.push(format!("to = \"{}\"", to));
        }
        if let Some(ref subject) = self.criteria.subject {
            filters.push(format!("subject = \"{}\"", subject));
        }
        if let Some(ref date_from) = self.criteria.date_from {
            filters.push(format!("date >= \"{}\"", date_from));
        }
        if let Some(ref date_to) = self.criteria.date_to {
            filters.push(format!("date <= \"{}\"", date_to));
        }
        let filter = if !filters.is_empty() {
            Some(filters.join(" AND "))
        } else {
            None
        };
        (query, filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_meili_query_simple_name_becomes_search_term() {
        // This test demonstrates why "explain the email from alice" might match the wrong email
        
        // Create search criteria where from="alice" (no @ symbol)
        let criteria = QueryCriteria {
            keywords: vec![],
            from: Some("alice".to_string()),
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: "explain the email from alice".to_string(),
            llm_confidence: 0.9,
        };
        
        let builder = EmailQueryBuilder::new(criteria);
        let (query, filter) = builder.build_meili_query();
        
        // ISSUE: "alice" becomes a general search term, not a filter
        assert_eq!(query, Some("alice".to_string()));
        assert_eq!(filter, None);
        
        // This means an email with "alice" anywhere in its content would match,
        // not just emails where Alice is the sender
    }
    
    #[test]
    fn test_build_meili_query_email_address_becomes_filter() {
        // In contrast, an email address is handled correctly
        
        let criteria = QueryCriteria {
            keywords: vec![],
            from: Some("alice@example.com".to_string()),
            to: None,
            subject: None,
            date_from: None,
            date_to: None,
            raw_query: "explain the email from alice@example.com".to_string(),
            llm_confidence: 0.9,
        };
        
        let builder = EmailQueryBuilder::new(criteria);
        let (query, filter) = builder.build_meili_query();
        
        // Email address correctly becomes a filter
        assert_eq!(query, None);
        assert_eq!(filter, Some("from = \"alice@example.com\"".to_string()));
    }
}

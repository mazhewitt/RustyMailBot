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

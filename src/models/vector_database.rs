use serde::{Serialize, Deserialize};
use crate::services::embedding_service::fetch_embedding;
use crate::utils::similarity::cosine_similarity;
use ollama_rs::Ollama;

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub text: String,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VectorDatabase {
    pub documents: Vec<Document>,
}

impl VectorDatabase {
    pub fn new() -> Self {
        Self { documents: Vec::new() }
    }

    pub async fn insert(&mut self, text: String, ollama: &mut Ollama) -> Result<(), Box<dyn std::error::Error>> {
        let embedding = fetch_embedding(ollama, &text).await?;
        self.documents.push(embedding);
        Ok(())
    }

    pub async fn search(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<Vec<&Document>, Box<dyn std::error::Error>> {
        let query_embedding = fetch_embedding(ollama, query).await?;
        let mut results: Vec<(&Document, f32)> = self.documents.iter()
            .map(|doc| (doc, cosine_similarity(&doc.embedding, &query_embedding.embedding)))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(results.into_iter().take(top_n).map(|(doc, _)| doc).collect())
    }

    pub async fn get_context(&self, query: &str, top_n: usize, ollama: &mut Ollama) -> Result<String, Box<dyn std::error::Error>> {
        let results = self.search(query, top_n, ollama).await?;
        let context: Vec<String> = results.iter().map(|doc| doc.text.clone()).collect();
        Ok(context.join("\n---\n"))
    }
}
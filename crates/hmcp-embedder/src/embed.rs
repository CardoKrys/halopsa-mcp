use async_trait::async_trait;
use serde_json::{json, Value};

/// Trait for embedding providers.
#[allow(dead_code)]
#[async_trait]
pub trait Embedder: Send + Sync + 'static {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
    fn dimensions(&self) -> usize;
}

// --- Local fastembed ---

#[allow(dead_code)]
pub struct LocalEmbedder {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
    dims: usize,
}

// SAFETY: fastembed::TextEmbedding is Send (ONNX runtime is thread-safe).
// The Mutex ensures only one thread accesses it at a time.
unsafe impl Send for LocalEmbedder {}
unsafe impl Sync for LocalEmbedder {}

impl LocalEmbedder {
    pub fn new(_model_name: &str) -> Self {
        let model = fastembed::TextEmbedding::try_new(
            fastembed::InitOptions::new(fastembed::EmbeddingModel::BGEBaseENV15)
                .with_show_download_progress(true),
        )
        .expect("Failed to initialize fastembed model");

        Self {
            dims: 768, // BGE base
            model: std::sync::Mutex::new(model),
        }
    }
}

#[async_trait]
impl Embedder for LocalEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let texts: Vec<String> = texts.to_vec();
        let model = &self.model;

        // fastembed is sync, use the mutex
        tokio::task::spawn_blocking({
            let model_ptr = model as *const std::sync::Mutex<fastembed::TextEmbedding> as usize;
            move || {
                let model_mutex = unsafe { &*(model_ptr as *const std::sync::Mutex<fastembed::TextEmbedding>) };
                let mut model = model_mutex.lock().unwrap();
                model
                    .embed(texts, None)
                    .map_err(|e| format!("Embedding failed: {e}"))
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

// --- OpenAI compatible ---

pub struct OpenAIEmbedder {
    api_key: String,
    api_url: String,
    model: String,
    http: reqwest::Client,
    dims: std::sync::atomic::AtomicUsize,
}

impl OpenAIEmbedder {
    pub fn new(api_key: &str, api_url: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            api_url: api_url.to_string(),
            model: model.to_string(),
            http: reqwest::Client::new(),
            dims: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let resp = self
            .http
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .json(&json!({
                "model": self.model,
                "input": texts,
            }))
            .send()
            .await
            .map_err(|e| format!("OpenAI request failed: {e}"))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI error: {body}"));
        }

        let data: Value = resp.json().await.map_err(|e| format!("Parse failed: {e}"))?;
        let embeddings = data
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or("No data in response")?;

        let result: Vec<Vec<f32>> = embeddings
            .iter()
            .filter_map(|item| {
                item.get("embedding")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    })
            })
            .collect();

        // Auto-detect dimensions
        if let Some(first) = result.first() {
            self.dims
                .store(first.len(), std::sync::atomic::Ordering::Relaxed);
        }

        Ok(result)
    }

    fn dimensions(&self) -> usize {
        self.dims.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// --- Ollama ---

pub struct OllamaEmbedder {
    url: String,
    model: String,
    http: reqwest::Client,
    dims: std::sync::atomic::AtomicUsize,
}

impl OllamaEmbedder {
    pub fn new(url: &str, model: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            http: reqwest::Client::new(),
            dims: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            let resp = self
                .http
                .post(format!("{}/api/embed", self.url))
                .json(&json!({
                    "model": self.model,
                    "input": text,
                }))
                .send()
                .await
                .map_err(|e| format!("Ollama request failed: {e}"))?;

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(format!("Ollama error: {body}"));
            }

            let data: Value = resp.json().await.map_err(|e| format!("Parse failed: {e}"))?;
            let embeddings = data
                .get("embeddings")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_array())
                .ok_or("No embeddings in response")?;

            let embedding: Vec<f32> = embeddings
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            if self.dims.load(std::sync::atomic::Ordering::Relaxed) == 0 {
                self.dims
                    .store(embedding.len(), std::sync::atomic::Ordering::Relaxed);
            }

            results.push(embedding);
        }

        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dims.load(std::sync::atomic::Ordering::Relaxed)
    }
}

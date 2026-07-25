//! Ollama API client for local LLM inference.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "llama3";

#[derive(Clone, Debug)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GenerateResponse {
    response: String,
    done: bool,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatResponseMessage,
}

impl OllamaClient {
    pub fn new(base_url: Option<&str>, model: Option<&str>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_default();

        Self {
            base_url: base_url.unwrap_or(DEFAULT_OLLAMA_URL).trim_end_matches('/').to_string(),
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            client,
        }
    }

    /// Check if local Ollama server is running and reachable
    pub async fn ping(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Generate text response from prompt
    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let payload = GenerateRequest {
            model: &self.model,
            prompt,
            stream: false,
            system,
        };

        debug!("Sending request to Ollama: {}", prompt);
        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to connect to Ollama at {}", self.base_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama error status: {}", resp.status());
        }

        let body: GenerateResponse = resp.json().await?;
        Ok(body.response)
    }

    /// Chat interface with system role
    pub async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let messages = vec![
            ChatMessage { role: "system", content: system_prompt },
            ChatMessage { role: "user", content: user_message },
        ];
        let payload = ChatRequest {
            model: &self.model,
            messages,
            stream: false,
        };

        let resp = self.client.post(&url)
            .json(&payload)
            .send()
            .await
            .with_context(|| "Failed to execute Ollama chat request")?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama chat error status: {}", resp.status());
        }

        let body: ChatResponse = resp.json().await?;
        Ok(body.message.content)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/*
 * Meeting Assistant CLI - Ollama Provider Plugin
 * Copyright (c) 2024 Meeting Assistant Contributors
 * 
 * This work is licensed under the Creative Commons Attribution-NonCommercial 4.0 International License.
 * To view a copy of this license, visit http://creativecommons.org/licenses/by-nc/4.0/
 * 
 * You are free to share and adapt this work for non-commercial purposes with attribution.
 * Commercial use is prohibited without explicit written permission.
 * 
 * For commercial licensing inquiries, please contact the project maintainers.
 */

use std::time::Duration;
use anyhow::{Result, Context};
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::timeout;

use crate::plugin_system::*;

/// Ollama local LLM provider plugin
pub struct OllamaProvider {
    client: Client,
    config: OllamaConfig,
    available_models: Vec<OllamaModel>,
    health_status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub default_model: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub health_check_interval: u64,
    pub enabled: bool,
    pub auto_pull_models: bool,
    pub preferred_models: Vec<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            default_model: "llama2:7b".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            health_check_interval: 60,
            enabled: true,
            auto_pull_models: false,
            preferred_models: vec![
                "llama2:7b".to_string(),
                "codellama:7b".to_string(),
                "mistral:7b".to_string(),
                "neural-chat:7b".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub digest: String,
    pub modified_at: String,
    pub details: OllamaModelDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelDetails {
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>,
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Unhealthy { error: String },
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    system: Option<String>,
    template: Option<String>,
    context: Option<Vec<i32>>,
    stream: bool,
    raw: bool,
    format: Option<String>,
    keep_alive: Option<String>,
    options: OllamaOptions,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaOptions {
    seed: Option<i32>,
    num_predict: Option<i32>,
    top_k: Option<i32>,
    top_p: Option<f32>,
    tfs_z: Option<f32>,
    typical_p: Option<f32>,
    repeat_last_n: Option<i32>,
    temperature: Option<f32>,
    repeat_penalty: Option<f32>,
    presence_penalty: Option<f32>,
    frequency_penalty: Option<f32>,
    mirostat: Option<i32>,
    mirostat_tau: Option<f32>,
    mirostat_eta: Option<f32>,
    penalize_newline: Option<bool>,
    stop: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
    context: Option<Vec<i32>>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_count: Option<i32>,
    prompt_eval_duration: Option<u64>,
    eval_count: Option<i32>,
    eval_duration: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

impl OllamaProvider {
    pub fn new(config: OllamaConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            available_models: Vec::new(),
            health_status: HealthStatus::Unknown,
        }
    }

    /// Check if Ollama service is healthy and accessible
    pub async fn health_check(&mut self) -> Result<HealthStatus> {
        match timeout(
            Duration::from_secs(5),
            self.client.get(&format!("{}/api/tags", self.config.base_url)).send()
        ).await {
            Ok(Ok(response)) if response.status().is_success() => {
                self.health_status = HealthStatus::Healthy;
                Ok(HealthStatus::Healthy)
            }
            Ok(Ok(response)) => {
                let error = format!("Ollama service returned status: {}", response.status());
                self.health_status = HealthStatus::Unhealthy { error: error.clone() };
                Ok(HealthStatus::Unhealthy { error })
            }
            Ok(Err(e)) => {
                let error = format!("Failed to connect to Ollama: {}", e);
                self.health_status = HealthStatus::Unhealthy { error: error.clone() };
                Ok(HealthStatus::Unhealthy { error })
            }
            Err(_) => {
                let error = "Ollama service timeout".to_string();
                self.health_status = HealthStatus::Unhealthy { error: error.clone() };
                Ok(HealthStatus::Unhealthy { error })
            }
        }
    }

    /// List available models in Ollama
    pub async fn list_models(&mut self) -> Result<Vec<OllamaModel>> {
        let response = self.client
            .get(&format!("{}/api/tags", self.config.base_url))
            .send()
            .await
            .context("Failed to connect to Ollama service")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Ollama API error: {}", response.status()));
        }

        let models_response: OllamaModelsResponse = response.json().await
            .context("Failed to parse models response")?;

        self.available_models = models_response.models;
        Ok(self.available_models.clone())
    }

    /// Pull a model if it's not available
    pub async fn ensure_model(&self, model_name: &str) -> Result<()> {
        // Check if model exists
        let models = self.client
            .get(&format!("{}/api/tags", self.config.base_url))
            .send()
            .await?
            .json::<OllamaModelsResponse>()
            .await?;

        let model_exists = models.models.iter().any(|m| m.name == model_name);
        
        if !model_exists && self.config.auto_pull_models {
            println!("🦙 Pulling Ollama model: {} (this may take a while...)", model_name);
            
            let pull_request = json!({
                "name": model_name,
                "stream": false
            });

            let response = self.client
                .post(&format!("{}/api/pull", self.config.base_url))
                .json(&pull_request)
                .send()
                .await
                .context("Failed to pull model")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Failed to pull model {}: {}", model_name, response.status()));
            }

            println!("✅ Model {} pulled successfully", model_name);
        } else if !model_exists {
            return Err(anyhow::anyhow!(
                "Model '{}' not found. Enable auto_pull_models or run: ollama pull {}", 
                model_name, model_name
            ));
        }

        Ok(())
    }

    /// Get the best available model for the request
    pub async fn select_model(&self, requested_model: Option<&str>) -> Result<String> {
        if let Some(model) = requested_model {
            return Ok(model.to_string());
        }

        // Try preferred models in order
        for preferred in &self.config.preferred_models {
            if self.available_models.iter().any(|m| m.name == *preferred) {
                return Ok(preferred.clone());
            }
        }

        // Fall back to default model
        if self.available_models.iter().any(|m| m.name == self.config.default_model) {
            return Ok(self.config.default_model.clone());
        }

        // Use first available model
        if let Some(model) = self.available_models.first() {
            return Ok(model.name.clone());
        }

        Err(anyhow::anyhow!("No models available in Ollama"))
    }

    /// Create Ollama options from LLM options
    fn create_ollama_options(&self, options: &LLMOptions) -> OllamaOptions {
        OllamaOptions {
            seed: None,
            num_predict: options.max_tokens.map(|t| t as i32),
            top_k: None,
            top_p: None,
            tfs_z: None,
            typical_p: None,
            repeat_last_n: None,
            temperature: options.temperature,
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            mirostat: None,
            mirostat_tau: None,
            mirostat_eta: None,
            penalize_newline: None,
            stop: None,
        }
    }

    /// Generate completion with retry logic
    async fn generate_with_retry(
        &self,
        request: &OllamaGenerateRequest,
    ) -> Result<OllamaGenerateResponse> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            match self.client
                .post(&format!("{}/api/generate", self.config.base_url))
                .json(request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    return response.json::<OllamaGenerateResponse>().await
                        .context("Failed to parse Ollama response");
                }
                Ok(response) => {
                    let error = anyhow::anyhow!("Ollama API error: {}", response.status());
                    last_error = Some(error);
                }
                Err(e) => {
                    let error = anyhow::anyhow!("Failed to connect to Ollama: {}", e);
                    last_error = Some(error);
                }
            }

            if attempt < self.config.max_retries {
                let delay = Duration::from_millis(1000 * attempt as u64);
                tokio::time::sleep(delay).await;
                tracing::warn!("Ollama request failed, retrying in {:?} (attempt {}/{})", 
                    delay, attempt, self.config.max_retries);
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }
}

#[async_trait]
impl Plugin for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn description(&self) -> &str {
        "Local LLM provider using Ollama for private, offline AI inference"
    }

    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }

    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom configuration if available
        let plugin_data = context.plugin_data.read().await;
        if let Some(data) = plugin_data.get("ollama") {
            if let Ok(custom_config) = serde_json::from_value::<OllamaConfig>(data.clone()) {
                self.config = custom_config;
            }
        }

        // Perform health check
        match self.health_check().await? {
            HealthStatus::Healthy => {
                println!("🦙 Ollama provider initialized successfully");
                
                // List available models
                match self.list_models().await {
                    Ok(models) => {
                        println!("   Available models: {}", models.len());
                        for model in &models {
                            println!("   • {} ({})", model.name, model.details.parameter_size);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to list Ollama models: {}", e);
                    }
                }
            }
            HealthStatus::Unhealthy { error } => {
                println!("⚠️  Ollama provider initialized but service is unhealthy: {}", error);
                println!("   Make sure Ollama is running: ollama serve");
            }
            HealthStatus::Unknown => {
                println!("❓ Ollama provider initialized with unknown health status");
            }
        }

        Ok(())
    }

    async fn cleanup(&mut self, _context: &PluginContext) -> Result<()> {
        println!("🦙 Ollama provider cleaned up");
        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        _context: &PluginContext,
    ) -> Result<PluginHookResult> {
        match event {
            PluginEvent::ApplicationStartup => {
                // Perform initial health check and model listing
                let _ = self.health_check().await;
                let _ = self.list_models().await;
                Ok(PluginHookResult::Continue)
            }
            PluginEvent::BeforePromptRequest { context: _ } => {
                // Ensure service is healthy before processing
                match &self.health_status {
                    HealthStatus::Healthy => Ok(PluginHookResult::Continue),
                    HealthStatus::Unhealthy { error } => {
                        tracing::warn!("Ollama service unhealthy: {}", error);
                        Ok(PluginHookResult::Continue)
                    }
                    HealthStatus::Unknown => {
                        let _ = self.health_check().await;
                        Ok(PluginHookResult::Continue)
                    }
                }
            }
            _ => Ok(PluginHookResult::Continue),
        }
    }

    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::ApplicationStartup,
            PluginEvent::BeforePromptRequest { context: String::new() },
        ]
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "base_url": {
                    "type": "string",
                    "default": "http://localhost:11434",
                    "description": "Ollama service URL"
                },
                "default_model": {
                    "type": "string",
                    "default": "llama2:7b",
                    "description": "Default model to use"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "default": 30,
                    "minimum": 5,
                    "maximum": 300,
                    "description": "Request timeout in seconds"
                },
                "max_retries": {
                    "type": "integer",
                    "default": 3,
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Maximum retry attempts"
                },
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable/disable Ollama provider"
                },
                "auto_pull_models": {
                    "type": "boolean",
                    "default": false,
                    "description": "Automatically pull missing models"
                },
                "preferred_models": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Preferred models in order of preference"
                }
            }
        }))
    }

    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        if let Some(base_url) = config.get("base_url") {
            if !base_url.is_string() {
                return Err(anyhow::anyhow!("'base_url' must be a string"));
            }
            let url_str = base_url.as_str().unwrap();
            if !url_str.starts_with("http://") && !url_str.starts_with("https://") {
                return Err(anyhow::anyhow!("'base_url' must start with http:// or https://"));
            }
        }

        if let Some(timeout) = config.get("timeout_seconds") {
            if !timeout.is_number() {
                return Err(anyhow::anyhow!("'timeout_seconds' must be a number"));
            }
            let timeout_val = timeout.as_u64().unwrap_or(0);
            if timeout_val < 5 || timeout_val > 300 {
                return Err(anyhow::anyhow!("'timeout_seconds' must be between 5 and 300"));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn generate_completion(
        &self,
        prompt: &str,
        _context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String> {
        // Ensure the requested model is available
        let model = self.select_model(options.model.as_deref()).await?;
        self.ensure_model(&model).await?;

        let request = OllamaGenerateRequest {
            model,
            prompt: prompt.to_string(),
            system: options.system_prompt.clone(),
            template: None,
            context: None,
            stream: false,
            raw: false,
            format: None,
            keep_alive: Some("5m".to_string()),
            options: self.create_ollama_options(options),
        };

        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            self.generate_with_retry(&request)
        ).await
        .context("Ollama request timed out")?
        .context("Failed to generate completion")?;

        Ok(response.response)
    }

    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        _context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        // Ensure the requested model is available
        let model = self.select_model(options.model.as_deref()).await?;
        self.ensure_model(&model).await?;

        let request = OllamaGenerateRequest {
            model,
            prompt: prompt.to_string(),
            system: options.system_prompt.clone(),
            template: None,
            context: None,
            stream: true,
            raw: false,
            format: None,
            keep_alive: Some("5m".to_string()),
            options: self.create_ollama_options(options),
        };

        let response = self.client
            .post(&format!("{}/api/generate", self.config.base_url))
            .json(&request)
            .send()
            .await
            .context("Failed to start streaming request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Ollama streaming API error: {}", response.status()));
        }

        let stream = response.bytes_stream().map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    
                    // Parse each line as a JSON response
                    for line in chunk_str.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        
                        match serde_json::from_str::<OllamaGenerateResponse>(line) {
                            Ok(response) => {
                                if response.done {
                                    return Ok(String::new()); // End of stream
                                }
                                return Ok(response.response);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse Ollama streaming response: {}", e);
                                continue;
                            }
                        }
                    }
                    
                    Ok(String::new())
                }
                Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
            }
        });

        Ok(Box::new(stream))
    }

    async fn transcribe_audio(
        &self,
        _audio_file: &std::path::PathBuf,
        _context: &PluginContext,
    ) -> Result<Option<String>> {
        // Ollama doesn't support audio transcription yet
        Ok(None)
    }

    async fn analyze_image(
        &self,
        _image_path: &std::path::PathBuf,
        _text_context: Option<&str>,
        _context: &PluginContext,
    ) -> Result<Option<String>> {
        // TODO: Implement when Ollama supports vision models
        Ok(None)
    }
}

/// Utility functions for Ollama management
impl OllamaProvider {
    /// Get health status
    pub fn get_health_status(&self) -> &HealthStatus {
        &self.health_status
    }

    /// Get available models
    pub fn get_available_models(&self) -> &[OllamaModel] {
        &self.available_models
    }

    /// Get configuration
    pub fn get_config(&self) -> &OllamaConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: OllamaConfig) {
        self.config = config;
    }
} 
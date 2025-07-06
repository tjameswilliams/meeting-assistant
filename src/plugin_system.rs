/*
 * Meeting Assistant CLI - Plugin System
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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::types::*;
use crate::config::Config;

/// Plugin lifecycle events that can be hooked into
#[derive(Debug, Clone, PartialEq)]
pub enum PluginEvent {
    // Application lifecycle
    ApplicationStartup,
    ApplicationShutdown,
    
    // Input events
    HotkeyDetected { key: String, tap_count: usize },
    ClipboardUpdated { content: String },
    
    // Audio processing events
    AudioCaptured { file_path: PathBuf },
    AudioBufferStarted,
    AudioBufferStopped,
    
    // AI/LLM processing events
    BeforePromptRequest { context: String },
    AfterPromptCreated { prompt: String },
    PromptStreamChunk { chunk: String },
    PromptStreamComplete { response: String },
    
    // TTS events (for future expansion)
    TtsStarted { text: String },
    TtsCompleted { audio_path: PathBuf },
    TtsError { error: String },
    
    // Analysis events
    ContentAnalyzed { content: String, analysis: ContentAnalysis },
    CodeMemoryUpdated { code_entry: CodeEntry },
    
    // Session events
    SessionHistoryUpdated { entry: SessionEntry },
    ConversationContextUpdated { context: Vec<ConversationEntry> },
    
    // Error events
    ErrorOccurred { error: String, context: String },
    
    // Custom events (for plugins to communicate)
    Custom { event_type: String, data: serde_json::Value },
}

/// Plugin context provides access to system data and state
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub config: Config,
    pub session_history: Arc<RwLock<Vec<SessionEntry>>>,
    pub conversation_context: Arc<RwLock<Vec<ConversationEntry>>>,
    pub code_memory: Arc<RwLock<Vec<CodeEntry>>>,
    pub is_processing: Arc<RwLock<bool>>,
    pub temp_dir: PathBuf,
    pub plugin_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

/// Plugin hook result that can modify behavior
#[derive(Debug, Clone)]
pub enum PluginHookResult {
    /// Continue normal processing
    Continue,
    /// Stop processing this event (other plugins won't be called)
    Stop,
    /// Replace content/behavior with plugin-provided data
    Replace(serde_json::Value),
    /// Modify the data and continue
    Modify(serde_json::Value),
}

/// Core plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin metadata
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;
    fn author(&self) -> &str;
    
    /// Plugin initialization (called once at startup)
    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        let _ = context;
        Ok(())
    }
    
    /// Plugin cleanup (called once at shutdown)
    async fn cleanup(&mut self, context: &PluginContext) -> Result<()> {
        let _ = context;
        Ok(())
    }
    
    /// Handle plugin events
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        let _ = (event, context);
        Ok(PluginHookResult::Continue)
    }
    
    /// Get events this plugin wants to handle
    fn subscribed_events(&self) -> Vec<PluginEvent>;
    
    /// Plugin configuration schema (optional)
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }
    
    /// Validate plugin configuration
    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        let _ = config;
        Ok(())
    }
}

/// LLM provider plugin trait for replacing AI functionality
#[async_trait]
pub trait LLMProvider: Plugin {
    /// Generate text completion
    async fn generate_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String>;
    
    /// Generate streaming completion
    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String>> + Send + Unpin>>;
    
    /// Transcribe audio (optional - can fallback to system)
    async fn transcribe_audio(
        &self,
        audio_file: &PathBuf,
        context: &PluginContext,
    ) -> Result<Option<String>> {
        let _ = (audio_file, context);
        Ok(None)
    }
    
    /// Analyze image with optional text context
    async fn analyze_image(
        &self,
        image_path: &PathBuf,
        text_context: Option<&str>,
        context: &PluginContext,
    ) -> Result<Option<String>> {
        let _ = (image_path, text_context, context);
        Ok(None)
    }
}

/// LLM generation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub streaming: bool,
}

impl Default for LLMOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(1800),
            temperature: Some(0.5),
            model: None,
            system_prompt: None,
            streaming: false,
        }
    }
}

/// Audio processor plugin trait for custom audio handling
#[async_trait]
pub trait AudioProcessor: Plugin {
    /// Process captured audio before transcription
    async fn process_audio(
        &self,
        audio_file: &PathBuf,
        context: &PluginContext,
    ) -> Result<Option<PathBuf>>;
    
    /// Enhance audio quality
    async fn enhance_audio(
        &self,
        audio_file: &PathBuf,
        context: &PluginContext,
    ) -> Result<Option<PathBuf>> {
        let _ = (audio_file, context);
        Ok(None)
    }
}

/// Content analyzer plugin trait for custom content analysis
#[async_trait]
pub trait ContentAnalyzer: Plugin {
    /// Analyze content type and provide metadata
    async fn analyze_content(
        &self,
        content: &str,
        context: &PluginContext,
    ) -> Result<Option<ContentAnalysis>>;
    
    /// Extract key information from content
    async fn extract_key_info(
        &self,
        content: &str,
        context: &PluginContext,
    ) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let _ = (content, context);
        Ok(None)
    }
}

/// Plugin metadata for installation and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    
    // Dependencies
    pub dependencies: HashMap<String, String>,
    pub rust_version: Option<String>,
    
    // Plugin-specific info
    pub plugin_type: PluginType,
    pub entry_point: String,
    pub config_schema: Option<serde_json::Value>,
    
    // Installation info
    pub install_requirements: Vec<String>,
    pub permissions: Vec<String>,
}

/// Plugin type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    /// Core replacement plugins (LLM, Audio, etc.)
    Core,
    /// Enhancement plugins (add functionality)
    Enhancement,
    /// Integration plugins (external services)
    Integration,
    /// UI/UX plugins (modify display)
    Interface,
    /// Utility plugins (helper functions)
    Utility,
}

/// Plugin installation source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginSource {
    /// GitHub repository
    GitHub { owner: String, repo: String, branch: Option<String> },
    /// Local file path
    Local { path: PathBuf },
    /// HTTP URL
    Http { url: String },
    /// Git repository
    Git { url: String, branch: Option<String> },
}

/// Plugin manager for loading and managing plugins
pub struct PluginManager {
    plugins: Arc<RwLock<HashMap<String, Box<dyn Plugin>>>>,
    llm_providers: Arc<RwLock<HashMap<String, Box<dyn LLMProvider>>>>,
    audio_processors: Arc<RwLock<HashMap<String, Box<dyn AudioProcessor>>>>,
    content_analyzers: Arc<RwLock<HashMap<String, Box<dyn ContentAnalyzer>>>>,
    
    active_llm_provider: Arc<RwLock<Option<String>>>,
    plugin_configs: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    plugin_dir: PathBuf,
    
    context: PluginContext,
}

impl PluginManager {
    pub fn new(config: Config, temp_dir: PathBuf) -> Result<Self> {
        let plugin_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".meeting-assistant")
            .join("plugins");
        
        std::fs::create_dir_all(&plugin_dir)?;
        
        let context = PluginContext {
            config,
            session_history: Arc::new(RwLock::new(Vec::new())),
            conversation_context: Arc::new(RwLock::new(Vec::new())),
            code_memory: Arc::new(RwLock::new(Vec::new())),
            is_processing: Arc::new(RwLock::new(false)),
            temp_dir,
            plugin_data: Arc::new(RwLock::new(HashMap::new())),
        };
        
        Ok(Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            llm_providers: Arc::new(RwLock::new(HashMap::new())),
            audio_processors: Arc::new(RwLock::new(HashMap::new())),
            content_analyzers: Arc::new(RwLock::new(HashMap::new())),
            active_llm_provider: Arc::new(RwLock::new(None)),
            plugin_configs: Arc::new(RwLock::new(HashMap::new())),
            plugin_dir,
            context,
        })
    }
    
    /// Register a plugin
    pub async fn register_plugin(&mut self, name: String, plugin: Box<dyn Plugin>) -> Result<()> {
        plugin.validate_config(&serde_json::json!({}))?;
        self.plugins.write().await.insert(name.clone(), plugin);
        println!("📦 Registered plugin: {}", name);
        Ok(())
    }

    /// Register an LLM provider
    pub async fn register_llm_provider(&mut self, name: String, provider: Box<dyn LLMProvider>) -> Result<()> {
        provider.validate_config(&serde_json::json!({}))?;
        self.llm_providers.write().await.insert(name.clone(), provider);
        println!("🤖 Registered LLM provider: {}", name);
        Ok(())
    }

    /// Initialize all plugins
    pub async fn initialize_plugins(&self) -> Result<()> {
        // Initialize regular plugins
        let mut plugins = self.plugins.write().await;
        for plugin in plugins.values_mut() {
            plugin.initialize(&self.context).await
                .context(format!("Failed to initialize plugin: {}", plugin.name()))?;
        }
        
        // Initialize LLM providers
        let mut llm_providers = self.llm_providers.write().await;
        for provider in llm_providers.values_mut() {
            provider.initialize(&self.context).await
                .context(format!("Failed to initialize LLM provider: {}", provider.name()))?;
        }
        
        // Initialize audio processors
        let mut audio_processors = self.audio_processors.write().await;
        for processor in audio_processors.values_mut() {
            processor.initialize(&self.context).await
                .context(format!("Failed to initialize audio processor: {}", processor.name()))?;
        }
        
        // Initialize content analyzers
        let mut content_analyzers = self.content_analyzers.write().await;
        for analyzer in content_analyzers.values_mut() {
            analyzer.initialize(&self.context).await
                .context(format!("Failed to initialize content analyzer: {}", analyzer.name()))?;
        }
        
        Ok(())
    }
    
    /// Fire an event to all subscribed plugins
    pub async fn fire_event(&self, event: PluginEvent) -> Result<Vec<PluginHookResult>> {
        let mut results = Vec::new();
        
        let plugins = self.plugins.read().await;
        for plugin in plugins.values() {
            if plugin.subscribed_events().contains(&event) {
                // Note: We can't call handle_event here because we have a read lock
                // This is a design issue that needs to be resolved
                // For now, we'll collect the plugins that need to be called
                results.push(PluginHookResult::Continue);
            }
        }
        
        // TODO: Implement proper event handling with plugin state mutation
        Ok(results)
    }
    
    /// Install a plugin from a source
    pub async fn install_plugin(&self, source: PluginSource) -> Result<String> {
        match source {
            PluginSource::GitHub { owner, repo, branch } => {
                self.install_from_github(&owner, &repo, branch.as_deref()).await
            }
            PluginSource::Local { path } => {
                self.install_from_local(&path).await
            }
            PluginSource::Http { url } => {
                self.install_from_http(&url).await
            }
            PluginSource::Git { url, branch } => {
                self.install_from_git(&url, branch.as_deref()).await
            }
        }
    }
    
    /// Get active LLM provider
    pub async fn get_active_llm_provider(&self) -> Option<String> {
        self.active_llm_provider.read().await.clone()
    }
    
    /// Set active LLM provider
    pub async fn set_active_llm_provider(&self, provider_name: String) -> Result<()> {
        let providers = self.llm_providers.read().await;
        if providers.contains_key(&provider_name) {
            *self.active_llm_provider.write().await = Some(provider_name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("LLM provider '{}' not found", provider_name))
        }
    }

    /// Generate completion using the active LLM provider
    pub async fn generate_completion(&self, prompt: &str, options: &LLMOptions) -> Result<Option<String>> {
        if let Some(provider_name) = &*self.active_llm_provider.read().await {
            let providers = self.llm_providers.read().await;
            if let Some(provider) = providers.get(provider_name) {
                let result = provider.generate_completion(prompt, &self.context, options).await?;
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    /// Generate streaming completion using the active LLM provider
    pub async fn generate_streaming_completion(
        &self, 
        prompt: &str, 
        options: &LLMOptions
    ) -> Result<Option<Box<dyn futures::Stream<Item = Result<String>> + Send + Unpin>>> {
        if let Some(provider_name) = &*self.active_llm_provider.read().await {
            let providers = self.llm_providers.read().await;
            if let Some(provider) = providers.get(provider_name) {
                let result = provider.generate_streaming_completion(prompt, &self.context, options).await?;
                return Ok(Some(result));
            }
        }
        Ok(None)
    }
    
    /// List all installed plugins
    pub async fn list_plugins(&self) -> HashMap<String, PluginInfo> {
        let mut info = HashMap::new();
        
        let plugins = self.plugins.read().await;
        for (name, plugin) in plugins.iter() {
            info.insert(name.clone(), PluginInfo {
                name: plugin.name().to_string(),
                version: plugin.version().to_string(),
                description: plugin.description().to_string(),
                author: plugin.author().to_string(),
                plugin_type: PluginType::Enhancement, // Default, should be read from manifest
                enabled: true,
            });
        }
        
        info
    }
    
    async fn install_from_github(&self, owner: &str, repo: &str, branch: Option<&str>) -> Result<String> {
        let branch = branch.unwrap_or("main");
        let url = format!("https://github.com/{}/{}/archive/{}.tar.gz", owner, repo, branch);
        
        // Download and extract
        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("plugin.tar.gz");
        
        let response = reqwest::get(&url).await?;
        let bytes = response.bytes().await?;
        tokio::fs::write(&archive_path, bytes).await?;
        
        // TODO: Extract tar.gz and process manifest
        // This is a simplified implementation
        Ok(format!("{}/{}", owner, repo))
    }
    
    async fn install_from_local(&self, path: &PathBuf) -> Result<String> {
        // TODO: Process local plugin directory
        Ok(format!("local:{}", path.display()))
    }
    
    async fn install_from_http(&self, url: &str) -> Result<String> {
        // TODO: Download and process plugin from HTTP
        Ok(format!("http:{}", url))
    }
    
    async fn install_from_git(&self, url: &str, _branch: Option<&str>) -> Result<String> {
        // TODO: Clone git repository and process plugin
        Ok(format!("git:{}", url))
    }
}

/// Plugin information for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: PluginType,
    pub enabled: bool,
}

/// Plugin registry for discovering plugins
pub struct PluginRegistry {
    registry_url: String,
    cache_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".meeting-assistant")
            .join("plugin-cache");
        
        std::fs::create_dir_all(&cache_dir)?;
        
        Ok(Self {
            registry_url: "https://raw.githubusercontent.com/meeting-assistant/plugin-registry/main/registry.json".to_string(),
            cache_dir,
        })
    }
    
    /// Search for plugins in the registry
    pub async fn search_plugins(&self, query: &str) -> Result<Vec<PluginInfo>> {
        // TODO: Implement plugin search
        let _ = query;
        Ok(vec![])
    }
    
    /// Get plugin details from registry
    pub async fn get_plugin_info(&self, name: &str) -> Result<Option<PluginInfo>> {
        // TODO: Implement plugin info lookup
        let _ = name;
        Ok(None)
    }
}

/// Example LLM provider implementation for OpenRouter
pub struct OpenRouterProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl Plugin for OpenRouterProvider {
    fn name(&self) -> &str { "openrouter" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "OpenRouter LLM provider with access to multiple models" }
    fn author(&self) -> &str { "Meeting Assistant Team" }
    
    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::BeforePromptRequest { context: String::new() },
            PluginEvent::AfterPromptCreated { prompt: String::new() },
        ]
    }
    
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        match event {
            PluginEvent::BeforePromptRequest { context: _ } => {
                // Can modify context before prompt creation
                Ok(PluginHookResult::Continue)
            }
            PluginEvent::AfterPromptCreated { prompt: _ } => {
                // Can modify the prompt before sending to LLM
                Ok(PluginHookResult::Continue)
            }
            _ => Ok(PluginHookResult::Continue),
        }
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    async fn generate_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String> {
        // TODO: Implement OpenRouter API call
        let _ = (prompt, context, options);
        Ok("OpenRouter response".to_string())
    }
    
    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String>> + Send + Unpin>> {
        // TODO: Implement OpenRouter streaming API
        let _ = (prompt, context, options);
        use futures::stream;
        Ok(Box::new(stream::empty()))
    }
}

/// Example Ollama provider implementation
pub struct OllamaProvider {
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl Plugin for OllamaProvider {
    fn name(&self) -> &str { "ollama" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "Local Ollama LLM provider" }
    fn author(&self) -> &str { "Meeting Assistant Team" }
    
    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![]
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn generate_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String> {
        // TODO: Implement Ollama API call
        let _ = (prompt, context, options);
        Ok("Ollama response".to_string())
    }
    
    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn futures::Stream<Item = Result<String>> + Send + Unpin>> {
        // TODO: Implement Ollama streaming API
        let _ = (prompt, context, options);
        use futures::stream;
        Ok(Box::new(stream::empty()))
    }
} 
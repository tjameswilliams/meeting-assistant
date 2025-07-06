# Meeting Assistant Plugin System 🔌

A comprehensive plugin system for the Meeting Assistant CLI that provides extensive lifecycle hooks and allows for complete customization of core functionality.

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Lifecycle Hooks](#lifecycle-hooks)
- [Plugin Types](#plugin-types)
- [Installation Methods](#installation-methods)
- [Development Guide](#development-guide)
- [Example Plugins](#example-plugins)
- [CLI Commands](#cli-commands)
- [Configuration](#configuration)
- [Security](#security)

## Overview

The plugin system allows developers to:

- **Replace core functionality** (LLM providers, audio processing, etc.)
- **Add new features** through lifecycle hooks
- **Customize prompts** and AI behavior
- **Integrate external services** (Slack, Teams, etc.)
- **Modify UI/UX** and add new interfaces
- **Process data** at various stages of the application lifecycle

## Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Plugin System                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   Plugin        │  │   Plugin        │  │   Plugin        │  │
│  │   Manager       │  │   Registry      │  │   Context       │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   LLM           │  │   Audio         │  │   Content       │  │
│  │   Providers     │  │   Processors    │  │   Analyzers     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   GitHub        │  │   Local         │  │   HTTP          │  │
│  │   Installation  │  │   Installation  │  │   Installation  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Plugin Traits

1. **Plugin** - Base trait for all plugins
2. **LLMProvider** - For replacing AI/LLM functionality
3. **AudioProcessor** - For custom audio processing
4. **ContentAnalyzer** - For content analysis and classification

## Lifecycle Hooks

### Application Lifecycle

| Hook                  | When       | Use Case                                |
| --------------------- | ---------- | --------------------------------------- |
| `ApplicationStartup`  | App starts | Initialize resources, load configs      |
| `ApplicationShutdown` | App exits  | Cleanup, save state, send notifications |

### Input Events

| Hook               | When                      | Use Case                        |
| ------------------ | ------------------------- | ------------------------------- |
| `HotkeyDetected`   | Key combination pressed   | Custom hotkey actions, logging  |
| `ClipboardUpdated` | Clipboard content changes | Auto-analysis, format detection |

### Audio Processing

| Hook                 | When                   | Use Case                                       |
| -------------------- | ---------------------- | ---------------------------------------------- |
| `AudioCaptured`      | Audio file captured    | Processing, noise reduction, format conversion |
| `AudioBufferStarted` | Audio buffering begins | Initialize audio processing                    |
| `AudioBufferStopped` | Audio buffering stops  | Cleanup, final processing                      |

### AI/LLM Processing

| Hook                   | When                   | Use Case                                    |
| ---------------------- | ---------------------- | ------------------------------------------- |
| `BeforePromptRequest`  | Before prompt creation | Add context, modify system state            |
| `AfterPromptCreated`   | After prompt created   | Modify prompts, add instructions            |
| `PromptStreamChunk`    | Each streaming chunk   | Real-time processing, display modifications |
| `PromptStreamComplete` | Stream finished        | Post-processing, logging, analysis          |

### Content Analysis

| Hook                | When             | Use Case                       |
| ------------------- | ---------------- | ------------------------------ |
| `ContentAnalyzed`   | Content analyzed | Enhance analysis, add metadata |
| `CodeMemoryUpdated` | Code stored      | Version control, indexing      |

### Session Management

| Hook                         | When              | Use Case                         |
| ---------------------------- | ----------------- | -------------------------------- |
| `SessionHistoryUpdated`      | New session entry | Analytics, summaries, exports    |
| `ConversationContextUpdated` | Context changes   | Context management, optimization |

### TTS (Future)

| Hook           | When         | Use Case                    |
| -------------- | ------------ | --------------------------- |
| `TtsStarted`   | TTS begins   | Voice selection, processing |
| `TtsCompleted` | TTS finished | Audio post-processing       |
| `TtsError`     | TTS fails    | Error handling, fallbacks   |

### Error Handling

| Hook            | When      | Use Case                         |
| --------------- | --------- | -------------------------------- |
| `ErrorOccurred` | Any error | Logging, notifications, recovery |

### Custom Events

| Hook     | When           | Use Case                       |
| -------- | -------------- | ------------------------------ |
| `Custom` | Plugin-defined | Plugin-to-plugin communication |

## Plugin Types

### Core Plugins

Replace fundamental functionality:

- **LLM Providers**: OpenAI, OpenRouter, Ollama, Claude
- **Audio Processors**: Noise reduction, format conversion
- **Transcription**: Custom Whisper backends

### Enhancement Plugins

Add new features:

- **Sentiment Analysis**: Emotion detection
- **Language Detection**: Multi-language support
- **Code Formatters**: Language-specific formatting
- **Export Tools**: PDF, Word, Markdown exports

### Integration Plugins

Connect external services:

- **Slack Integration**: Send summaries to channels
- **Teams Integration**: Meeting notifications
- **Calendar Integration**: Schedule context
- **CRM Integration**: Customer data context

### Interface Plugins

Modify UI/UX:

- **Themes**: Custom color schemes
- **Layouts**: Alternative displays
- **Notifications**: Custom alerts
- **Dashboards**: Analytics views

### Utility Plugins

Helper functions:

- **Data Validators**: Input validation
- **Formatters**: Output formatting
- **Caching**: Performance optimization
- **Logging**: Enhanced logging

## Installation Methods

### GitHub Repository

```bash
# Install from GitHub
meeting-assistant plugin install owner/repo

# Install from specific branch
meeting-assistant plugin install owner/repo --branch feature-branch

# Install with explicit GitHub prefix
meeting-assistant plugin install github:owner/repo
```

### Local Development

```bash
# Install from local directory
meeting-assistant plugin install local:/path/to/plugin

# Install from current directory
meeting-assistant plugin install local:.
```

### HTTP/HTTPS

```bash
# Install from HTTP URL
meeting-assistant plugin install https://example.com/plugin.tar.gz

# Install from CDN
meeting-assistant plugin install http://cdn.example.com/plugins/sentiment-analyzer.zip
```

### Git Repository

```bash
# Install from Git repository
meeting-assistant plugin install git:https://git.example.com/plugin.git

# Install from specific branch
meeting-assistant plugin install git:https://git.example.com/plugin.git --branch main
```

## Development Guide

### Plugin Structure

```
my-plugin/
├── plugin.toml           # Plugin manifest
├── src/
│   ├── lib.rs           # Plugin implementation
│   └── config.rs        # Configuration handling
├── examples/
│   └── basic_usage.rs   # Usage examples
└── README.md           # Plugin documentation
```

### Plugin Manifest (plugin.toml)

```toml
[plugin]
name = "my-awesome-plugin"
version = "1.0.0"
description = "An awesome plugin for Meeting Assistant"
author = "Your Name <your.email@example.com>"
homepage = "https://github.com/username/my-awesome-plugin"
repository = "https://github.com/username/my-awesome-plugin"
license = "MIT"
tags = ["ai", "productivity", "meeting"]

[plugin.rust]
version = "1.70"

[plugin.entry]
main = "src/lib.rs"

[plugin.dependencies]
tokio = "1.0"
serde = "1.0"
serde_json = "1.0"

[plugin.config]
schema = "config_schema.json"

[plugin.permissions]
network = true
file_system = true
clipboard = false

[plugin.install]
requirements = ["ffmpeg", "python3"]
```

### Basic Plugin Implementation

```rust
use anyhow::Result;
use async_trait::async_trait;
use meeting_assistant::plugin_system::*;

pub struct MyPlugin {
    enabled: bool,
    config: MyPluginConfig,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            config: MyPluginConfig::default(),
        }
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn name(&self) -> &str { "my-plugin" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "My awesome plugin" }
    fn author(&self) -> &str { "Your Name" }

    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load configuration
        println!("🔌 {} plugin initialized", self.name());
        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        match event {
            PluginEvent::AudioCaptured { file_path } => {
                // Process audio file
                println!("🎵 Processing audio: {}", file_path.display());
                Ok(PluginHookResult::Continue)
            }
            _ => Ok(PluginHookResult::Continue),
        }
    }

    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::AudioCaptured { file_path: PathBuf::new() },
            PluginEvent::ApplicationStartup,
            PluginEvent::ApplicationShutdown,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyPluginConfig {
    enabled: bool,
    api_key: Option<String>,
    custom_settings: HashMap<String, String>,
}

impl Default for MyPluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            custom_settings: HashMap::new(),
        }
    }
}
```

### LLM Provider Plugin

```rust
use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use meeting_assistant::plugin_system::*;

pub struct CustomLLMProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl Plugin for CustomLLMProvider {
    fn name(&self) -> &str { "custom-llm" }
    fn version(&self) -> &str { "1.0.0" }
    fn description(&self) -> &str { "Custom LLM provider" }
    fn author(&self) -> &str { "Your Name" }

    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::BeforePromptRequest { context: String::new() },
            PluginEvent::AfterPromptCreated { prompt: String::new() },
        ]
    }
}

#[async_trait]
impl LLMProvider for CustomLLMProvider {
    async fn generate_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String> {
        // Implement your LLM API call
        let response = self.client
            .post(&format!("{}/v1/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "prompt": prompt,
                "max_tokens": options.max_tokens.unwrap_or(1800),
                "temperature": options.temperature.unwrap_or(0.5),
            }))
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        Ok(result["choices"][0]["text"].as_str().unwrap_or("").to_string())
    }

    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        // Implement streaming API call
        // Return a stream of text chunks
        todo!("Implement streaming completion")
    }
}
```

## Example Plugins

### 1. Sentiment Analyzer Plugin

Analyzes emotional tone in conversations:

```rust
// See src/plugins/sentiment_analyzer.rs for full implementation
```

Features:

- Real-time sentiment analysis
- Trend tracking over sessions
- Configurable keyword lists
- Integration with session history

### 2. Slack Integration Plugin

Sends meeting summaries to Slack channels:

```rust
pub struct SlackIntegrationPlugin {
    webhook_url: String,
    channel: String,
    enabled: bool,
}

#[async_trait]
impl Plugin for SlackIntegrationPlugin {
    // ... implementation

    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        match event {
            PluginEvent::SessionHistoryUpdated { entry } => {
                // Send summary to Slack
                self.send_to_slack(entry).await?;
                Ok(PluginHookResult::Continue)
            }
            _ => Ok(PluginHookResult::Continue),
        }
    }
}
```

### 3. Code Formatter Plugin

Formats code snippets with language-specific rules:

```rust
pub struct CodeFormatterPlugin {
    formatters: HashMap<String, Box<dyn CodeFormatter>>,
}

#[async_trait]
impl Plugin for CodeFormatterPlugin {
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        match event {
            PluginEvent::ContentAnalyzed { content, analysis } => {
                if let Some(formatter) = self.formatters.get(&analysis.language) {
                    let formatted = formatter.format(content)?;
                    return Ok(PluginHookResult::Modify(serde_json::json!({
                        "formatted_content": formatted,
                        "original_content": content,
                        "formatter": "code_formatter"
                    })));
                }
                Ok(PluginHookResult::Continue)
            }
            _ => Ok(PluginHookResult::Continue),
        }
    }
}
```

### 4. Ollama Provider Plugin

Local LLM using Ollama:

```rust
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn generate_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<String> {
        let response = self.client
            .post(&format!("{}/api/generate", self.base_url))
            .json(&serde_json::json!({
                "model": options.model.as_ref().unwrap_or(&self.model),
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": options.temperature.unwrap_or(0.5),
                    "num_ctx": options.max_tokens.unwrap_or(2048),
                }
            }))
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        Ok(result["response"].as_str().unwrap_or("").to_string())
    }

    async fn generate_streaming_completion(
        &self,
        prompt: &str,
        context: &PluginContext,
        options: &LLMOptions,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        // Implement Ollama streaming
        use futures::stream;

        let response = self.client
            .post(&format!("{}/api/generate", self.base_url))
            .json(&serde_json::json!({
                "model": options.model.as_ref().unwrap_or(&self.model),
                "prompt": prompt,
                "stream": true,
            }))
            .send()
            .await?;

        let stream = response.bytes_stream().map(|chunk| {
            // Parse streaming JSON responses
            // Each line is a JSON object with "response" field
            Ok(String::new()) // Simplified
        });

        Ok(Box::new(stream))
    }
}
```

## CLI Commands

### Plugin Management

```bash
# List available commands
meeting-assistant plugin --help

# Install plugins
meeting-assistant plugin install owner/repo
meeting-assistant plugin install local:./my-plugin
meeting-assistant plugin install https://example.com/plugin.tar.gz

# List installed plugins
meeting-assistant plugin list

# Search for plugins
meeting-assistant plugin search "sentiment analysis"

# Show plugin info
meeting-assistant plugin info sentiment-analyzer

# Enable/disable plugins
meeting-assistant plugin enable sentiment-analyzer
meeting-assistant plugin disable sentiment-analyzer

# Uninstall plugins
meeting-assistant plugin uninstall sentiment-analyzer

# Update plugins
meeting-assistant plugin update sentiment-analyzer

# Set active LLM provider
meeting-assistant plugin set-llm ollama
```

### Configuration

```bash
# Edit plugin configuration
meeting-assistant plugin config sentiment-analyzer

# Validate plugin configuration
meeting-assistant plugin validate sentiment-analyzer

# Show plugin configuration schema
meeting-assistant plugin schema sentiment-analyzer
```

## Configuration

### Plugin Configuration

Plugins can be configured through:

1. **Plugin manifest** (`plugin.toml`)
2. **Environment variables**
3. **Configuration files** (JSON/TOML)
4. **Runtime API** (through plugin context)

### Example Configuration

```json
{
  "plugins": {
    "sentiment-analyzer": {
      "enabled": true,
      "positive_keywords": ["great", "excellent", "amazing"],
      "negative_keywords": ["bad", "terrible", "awful"],
      "confidence_threshold": 0.7
    },
    "slack-integration": {
      "enabled": true,
      "webhook_url": "https://hooks.slack.com/services/...",
      "channel": "#meeting-summaries",
      "auto_send": true
    },
    "ollama-provider": {
      "enabled": true,
      "base_url": "http://localhost:11434",
      "model": "llama2:7b",
      "temperature": 0.7
    }
  }
}
```

### Environment Variables

```bash
# Plugin-specific environment variables
SENTIMENT_ANALYZER_ENABLED=true
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/...
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama2:7b
```

## Security

### Plugin Permissions

Plugins request permissions in their manifest:

```toml
[plugin.permissions]
network = true          # Access network resources
file_system = true      # Read/write files
clipboard = false       # Access clipboard
audio = true           # Access audio devices
screen = false         # Take screenshots
system = false         # Execute system commands
```

### Sandboxing

- Plugins run in isolated environments
- Resource limits (memory, CPU, network)
- File system restrictions
- Network access controls

### Code Signing

- Plugins can be signed for verification
- Official plugins are signed by the project
- Community plugins can be signed by authors

### Review Process

- Official plugins undergo security review
- Community plugins have user ratings
- Automated security scanning
- Vulnerability reporting system

## Plugin Registry

### Official Registry

The official plugin registry is hosted at:

- **URL**: `https://plugins.meeting-assistant.dev`
- **Repository**: `https://github.com/meeting-assistant/plugin-registry`

### Community Plugins

Community plugins are welcomed and encouraged:

1. **Submit PR** to the registry repository
2. **Follow guidelines** for plugin development
3. **Pass security review** (automated + manual)
4. **Maintain compatibility** with plugin API

### Featured Plugins

- **Ollama Provider** - Local LLM integration
- **Sentiment Analyzer** - Emotional context analysis
- **Slack Integration** - Team communication
- **Code Formatter** - Language-specific formatting
- **Export Tools** - PDF, Word, Markdown exports
- **Calendar Integration** - Meeting context
- **Language Detection** - Multi-language support

## Plugin API Reference

### Core Traits

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn description(&self) -> &str;
    fn author(&self) -> &str;

    async fn initialize(&mut self, context: &PluginContext) -> Result<()>;
    async fn cleanup(&mut self, context: &PluginContext) -> Result<()>;
    async fn handle_event(&mut self, event: &PluginEvent, context: &PluginContext) -> Result<PluginHookResult>;

    fn subscribed_events(&self) -> Vec<PluginEvent>;
    fn config_schema(&self) -> Option<serde_json::Value>;
    fn validate_config(&self, config: &serde_json::Value) -> Result<()>;
}
```

### Plugin Context

```rust
pub struct PluginContext {
    pub config: Config,
    pub session_history: Arc<RwLock<Vec<SessionEntry>>>,
    pub conversation_context: Arc<RwLock<Vec<ConversationEntry>>>,
    pub code_memory: Arc<RwLock<Vec<CodeEntry>>>,
    pub is_processing: Arc<RwLock<bool>>,
    pub temp_dir: PathBuf,
    pub plugin_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}
```

### Hook Results

```rust
pub enum PluginHookResult {
    Continue,                          // Continue normal processing
    Stop,                             // Stop processing (no other plugins)
    Replace(serde_json::Value),       // Replace with plugin data
    Modify(serde_json::Value),        // Modify and continue
}
```

## Advanced Features

### Plugin Communication

Plugins can communicate through:

1. **Custom events** - Send events between plugins
2. **Shared data** - Store data in plugin context
3. **Message passing** - Direct plugin-to-plugin messages

### Performance Optimization

- **Lazy loading** - Load plugins only when needed
- **Caching** - Cache plugin results
- **Parallel processing** - Run compatible plugins in parallel
- **Resource pooling** - Share resources between plugins

### Error Handling

- **Graceful degradation** - Continue if plugins fail
- **Error recovery** - Retry failed operations
- **Logging** - Comprehensive error logging
- **User notifications** - Inform users of plugin issues

## Troubleshooting

### Common Issues

1. **Plugin not loading**

   - Check plugin manifest syntax
   - Verify dependencies are installed
   - Check plugin permissions

2. **Plugin crashes**

   - Check plugin logs
   - Verify configuration
   - Update plugin to latest version

3. **Performance issues**
   - Check plugin resource usage
   - Optimize plugin code
   - Consider caching strategies

### Debug Mode

```bash
# Run with debug logging
RUST_LOG=debug meeting-assistant

# Plugin-specific debugging
RUST_LOG=meeting_assistant::plugin_system=debug meeting-assistant
```

### Log Files

```bash
# View plugin logs
tail -f ~/.meeting-assistant/logs/plugins.log

# View specific plugin logs
tail -f ~/.meeting-assistant/logs/plugins/sentiment-analyzer.log
```

## Future Enhancements

### Planned Features

1. **Plugin marketplace** - Browse and install plugins
2. **Visual plugin editor** - GUI for plugin configuration
3. **Plugin templates** - Scaffolding for new plugins
4. **Hot reloading** - Update plugins without restart
5. **Plugin analytics** - Usage statistics and performance metrics
6. **Cross-platform support** - Windows, Linux, macOS
7. **Web interface** - Browser-based plugin management
8. **Plugin dependencies** - Manage plugin relationships

### API Evolution

- **Backward compatibility** - Maintain API stability
- **Deprecation warnings** - Graceful API changes
- **Migration guides** - Help with API updates
- **Version negotiation** - Support multiple API versions

## Contributing

### Plugin Development

1. **Read the guidelines** - Follow development best practices
2. **Use templates** - Start with plugin templates
3. **Write tests** - Ensure plugin reliability
4. **Document thoroughly** - Help users understand your plugin
5. **Follow conventions** - Use consistent naming and patterns

### Contributing to Core

1. **Plugin system improvements** - Enhance the plugin framework
2. **New lifecycle hooks** - Add more integration points
3. **Performance optimizations** - Make plugins faster
4. **Security enhancements** - Improve plugin security
5. **Documentation** - Help others understand the system

### Community

- **Discord** - Join our developer community
- **GitHub** - Contribute to the codebase
- **Forum** - Ask questions and share knowledge
- **Blog** - Write about your plugin experiences

## License

The plugin system is licensed under the same terms as the main project (CC BY-NC 4.0). Individual plugins may have their own licenses.

---

**Ready to build your first plugin?** Start with our [Plugin Template](https://github.com/meeting-assistant/plugin-template) and join our growing community of developers!

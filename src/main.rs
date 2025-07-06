/*
 * Meeting Assistant CLI - Rust Edition
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

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use anyhow::{Result, Context};
use rdev::{listen, Event, EventType};
use parking_lot::Mutex;
use lazy_static::lazy_static;
use clap::{Parser, Subcommand};

mod audio;
mod ai;
mod input;
mod ui;
mod system;
mod config;
mod types;
mod setup;
mod plugin_system;
mod plugins;

use std::path::PathBuf;
use futures::StreamExt;

use audio::AudioCapture;
use ai::OpenAIClient;
use input::{KeyboardHandler, ClipboardHandler};
use ui::TerminalUI;
use system::SystemInfo;
use config::{Config, LLMProvider};
use types::*;
use setup::run_setup;
use plugin_system::*;
use plugins::{OllamaProvider, SentimentAnalyzerPlugin};

/// Meeting Assistant CLI - AI-powered meeting support with real-time audio capture
#[derive(Parser)]
#[command(name = "meeting-assistant")]
#[command(version = "1.0.0")]
#[command(about = "Ultra-fast AI meeting assistant with real-time audio capture and code analysis")]
#[command(long_about = "
Meeting Assistant CLI - Rust Edition

A high-performance CLI application that provides AI-powered meeting assistance with:
• Real-time audio capture and transcription
• Code analysis from clipboard
• Combined audio + code analysis
• Screenshot analysis with visual context
• Session history and conversation tracking
• Multiple Whisper backends for fast transcription

Global hotkeys (double-tap quickly):
• A - Answer questions or provide context for what's being discussed
• S - Analyze clipboard content (code-aware)
• Q - Combined audio + clipboard analysis
• W - Screenshot + audio analysis (code-aware)
• R - Cancel current request
• H - Show session history
• Ctrl+C - Exit
")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run interactive setup to install dependencies and configure the system
    Setup {
        /// Skip interactive prompts and use defaults
        #[arg(long)]
        non_interactive: bool,
        
        /// Force reinstall all dependencies
        #[arg(long)]
        force: bool,
    },
    
    /// Show system status and configuration
    Status,
    
    /// Run the main meeting assistant (default)
    Run,
    
    /// Plugin management commands
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Subcommand)]
enum PluginCommand {
    /// Install a plugin from a source
    Install {
        /// Plugin source (github:owner/repo, local:path, http:url, git:url)
        source: String,
        /// Optional branch for git sources
        #[arg(long)]
        branch: Option<String>,
    },
    
    /// List installed plugins
    List,
    
    /// Search for plugins in the registry
    Search {
        /// Search query
        query: String,
    },
    
    /// Show plugin information
    Info {
        /// Plugin name
        name: String,
    },
    
    /// Enable a plugin
    Enable {
        /// Plugin name
        name: String,
    },
    
    /// Disable a plugin
    Disable {
        /// Plugin name
        name: String,
    },
    
    /// Uninstall a plugin
    Uninstall {
        /// Plugin name
        name: String,
    },
    
    /// Update a plugin
    Update {
        /// Plugin name
        name: String,
    },
    
    /// Set active LLM provider
    SetLlm {
        /// Provider name
        provider: String,
    },
}

// Global channel for keyboard events
lazy_static! {
    static ref KEYBOARD_CHANNEL: Mutex<Option<mpsc::UnboundedSender<AppEvent>>> = Mutex::new(None);
}

// Global keyboard handler
lazy_static! {
    static ref GLOBAL_KEYBOARD_HANDLER: Mutex<KeyboardHandler> = Mutex::new(KeyboardHandler::new());
}

// Global keyboard event callback
fn keyboard_callback(event: Event) {
    if let EventType::KeyPress(key) = event.event_type {
        tracing::debug!("Keyboard callback received key: {:?}", key);
        
        if let Some(app_event) = GLOBAL_KEYBOARD_HANDLER.lock().handle_key_press(key) {
            tracing::info!("Keyboard callback generated event: {:?}", app_event);
            
            if let Some(sender) = KEYBOARD_CHANNEL.lock().as_ref() {
                match sender.send(app_event) {
                    Ok(_) => {
                        tracing::info!("Event sent successfully to channel");
                    }
                    Err(e) => {
                        tracing::error!("Failed to send event to channel: {}", e);
                    }
                }
            } else {
                tracing::warn!("No keyboard channel available");
            }
        } else {
            tracing::debug!("No app event generated for key: {:?}", key);
        }
    }
}

const DOUBLE_TAP_WINDOW_MS: u64 = 500;
const DEBOUNCE_MS: u64 = 50;
const MAX_RECORDING_TIME: u64 = 30000;
const BUFFER_DURATION: u64 = 8;
const CAPTURE_DURATION: u64 = 15;

pub struct MeetingAssistant {
    config: Config,
    audio_capture: Arc<RwLock<AudioCapture>>,
    openai_client: Arc<OpenAIClient>,
    clipboard_handler: Arc<RwLock<ClipboardHandler>>,
    terminal_ui: Arc<TerminalUI>,
    system_info: Arc<SystemInfo>,
    
    // Plugin system
    plugin_manager: Arc<PluginManager>,
    
    // State management
    is_processing: Arc<RwLock<bool>>,
    should_cancel: Arc<RwLock<bool>>,
    session_history: Arc<RwLock<Vec<SessionEntry>>>,
    conversation_context: Arc<RwLock<Vec<ConversationEntry>>>,
    conversation_summary: Arc<RwLock<String>>,
    code_memory: Arc<RwLock<Vec<CodeEntry>>>,
    
    // Event channels
    event_tx: mpsc::UnboundedSender<AppEvent>,
    
    // Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
}

impl MeetingAssistant {
    async fn register_builtin_plugins(plugin_manager: &mut PluginManager, config: &Config) -> Result<()> {
        // Register Ollama provider
        if let Some(ollama_config) = config.llm_provider.provider_configs.get("ollama") {
            let ollama_config = serde_json::from_value::<crate::plugins::ollama_provider::OllamaConfig>(ollama_config.clone())
                .unwrap_or_default();
            let ollama_provider = OllamaProvider::new(ollama_config);
            plugin_manager.register_llm_provider("ollama".to_string(), Box::new(ollama_provider)).await?;
        }
        
        // Register sentiment analyzer
        let sentiment_plugin = SentimentAnalyzerPlugin::new();
        plugin_manager.register_plugin("sentiment_analyzer".to_string(), Box::new(sentiment_plugin)).await?;
        
        // Set active LLM provider based on configuration
        match &config.llm_provider.active_provider {
            LLMProvider::Ollama => {
                plugin_manager.set_active_llm_provider("ollama".to_string()).await?;
                println!("🦙 Using Ollama as LLM provider");
            }
            LLMProvider::OpenAI => {
                println!("🤖 Using OpenAI as LLM provider");
            }
            LLMProvider::Custom(name) => {
                if plugin_manager.set_active_llm_provider(name.clone()).await.is_ok() {
                    println!("🔌 Using {} as LLM provider", name);
                } else {
                    println!("⚠️  Custom LLM provider '{}' not found, falling back to OpenAI", name);
                }
            }
        }
        
        Ok(())
    }

    pub async fn new() -> Result<(Self, mpsc::UnboundedReceiver<AppEvent>)> {
        // Load configuration
        let config = Config::load().await?;
        
        // Initialize components
        let audio_capture = Arc::new(RwLock::new(AudioCapture::new(&config).await?));
        let openai_client = Arc::new(OpenAIClient::new(&config).await?);
        let clipboard_handler = Arc::new(RwLock::new(ClipboardHandler::new()));
        let terminal_ui = Arc::new(TerminalUI::new());
        let system_info = Arc::new(SystemInfo::new().await?);
        
        // Initialize plugin manager
        let temp_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".meeting-assistant")
            .join("temp");
        std::fs::create_dir_all(&temp_dir)?;
        let mut plugin_manager = PluginManager::new(config.clone(), temp_dir)?;
        
        // Register built-in plugins
        Self::register_builtin_plugins(&mut plugin_manager, &config).await?;
        
        let plugin_manager = Arc::new(plugin_manager);
        
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        // Create cancellation token
        let cancellation_token = CancellationToken::new();
        
        // Initialize global keyboard channel
        {
            let mut global_channel = KEYBOARD_CHANNEL.lock();
            *global_channel = Some(event_tx.clone());
        }
        
        let assistant = Self {
            config,
            audio_capture,
            openai_client,
            clipboard_handler,
            terminal_ui,
            system_info,
            plugin_manager,
            is_processing: Arc::new(RwLock::new(false)),
            should_cancel: Arc::new(RwLock::new(false)),
            session_history: Arc::new(RwLock::new(Vec::new())),
            conversation_context: Arc::new(RwLock::new(Vec::new())),
            conversation_summary: Arc::new(RwLock::new(String::new())),
            code_memory: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            cancellation_token,
        };
        
        Ok((assistant, event_rx))
    }
    
    pub async fn run(&self, event_rx: mpsc::UnboundedReceiver<AppEvent>) -> Result<()> {
        // Setup terminal
        self.terminal_ui.print_welcome().await?;
        
        // Initialize plugins
        self.plugin_manager.initialize_plugins().await?;
        
        // Check system status (re-enabled)
        // Note: SystemInfo.check_system_status needs &mut self but we have &self
        // We'll skip this for now and add it back later with proper design
        
        // Start background tasks
        self.start_audio_buffering().await?;
        self.start_keyboard_listener().await?;
        
        // Setup ctrl+c handler
        let event_tx = self.event_tx.clone();
        ctrlc::set_handler(move || {
            println!("\n🛑 Ctrl+C pressed - shutting down...");
            let _ = event_tx.send(AppEvent::Shutdown);
            
            // Force exit after 2 seconds if graceful shutdown doesn't work
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(2));
                println!("🚫 Force exiting...");
                std::process::exit(0);
            });
        })?;
        
        // Main event loop
        self.event_loop(event_rx).await?;
        
        Ok(())
    }
    
    async fn event_loop(&self, mut event_rx: mpsc::UnboundedReceiver<AppEvent>) -> Result<()> {
        tracing::info!("Starting event loop");
        let mut event_count = 0;
        
        loop {
            tracing::info!("Event loop iteration {} - waiting for event", event_count);
            
            // Add a timeout to the recv() call to detect if we're stuck
            tracing::info!("About to call recv() on event_rx");
            
            let event = tokio::time::timeout(
                Duration::from_secs(1),
                event_rx.recv()
            ).await;
            
            match event {
                Ok(Some(event)) => {
                    tracing::info!("recv() returned: {:?}", event);
                    
                    event_count += 1;
                    tracing::info!("Processing event #{}: {:?}", event_count, event);
                    
                    // Check processing state before handling
                    let is_processing_before = *self.is_processing.read().await;
                    tracing::debug!("Processing state before event: {}", is_processing_before);
                    
                    // Handle the event with error recovery
                    let event_result = match event {
                        AppEvent::AudioCapture => {
                            tracing::info!("Handling audio capture event #{}", event_count);
                            let result = self.handle_audio_capture().await;
                            tracing::info!("Audio capture event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::ClipboardAnalysis => {
                            tracing::info!("Handling clipboard analysis event #{}", event_count);
                            let result = self.handle_clipboard_analysis().await;
                            tracing::info!("Clipboard analysis event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::CombinedMode => {
                            tracing::info!("Handling combined mode event #{}", event_count);
                            let result = self.handle_combined_mode().await;
                            tracing::info!("Combined mode event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::ScreenshotMode => {
                            tracing::info!("Handling screenshot mode event #{}", event_count);
                            let result = self.handle_screenshot_mode().await;
                            tracing::info!("Screenshot mode event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::Cancel => {
                            tracing::info!("Handling cancel event #{}", event_count);
                            let result = self.handle_cancel().await;
                            tracing::info!("Cancel event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::ShowHistory => {
                            tracing::info!("Handling show history event #{}", event_count);
                            let result = self.show_session_history().await;
                            tracing::info!("Show history event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::ClearContext => {
                            tracing::info!("Handling clear context event #{}", event_count);
                            let result = self.clear_conversation_context().await;
                            tracing::info!("Clear context event #{} completed with result: {:?}", event_count, result.is_ok());
                            result
                        }
                        AppEvent::Shutdown => {
                            tracing::info!("Handling shutdown event #{}", event_count);
                            self.terminal_ui.print_shutdown().await?;
                            // Cancel all background tasks
                            self.cancellation_token.cancel();
                            // Clear global keyboard channel
                            {
                                let mut global_channel = KEYBOARD_CHANNEL.lock();
                                *global_channel = None;
                            }
                            // Stop audio buffering
                            {
                                let mut audio_capture = self.audio_capture.write().await;
                                let _ = audio_capture.stop_buffering().await;
                            }
                            // Give tasks a moment to shut down gracefully
                            sleep(Duration::from_millis(500)).await;
                            println!("👋 Goodbye!");
                            return Ok(());
                        }
                    };
                    
                    // Check processing state after handling
                    let is_processing_after = *self.is_processing.read().await;
                    tracing::debug!("Processing state after event: {}", is_processing_after);
                    
                    tracing::info!("About to handle event result for event #{}", event_count);
                    
                    // If there was an error, reset the processing flag and show error
                    if let Err(e) = event_result {
                        tracing::error!("Error handling event #{}: {}", event_count, e);
                        *self.is_processing.write().await = false;
                        let _ = self.terminal_ui.print_warning(&format!("⚠️  Error: {}", e)).await;
                        let _ = self.terminal_ui.print_ready().await;
                        tracing::debug!("Reset processing flag after error");
                    } else {
                        tracing::info!("Event #{} processed successfully", event_count);
                    }
                    
                    tracing::info!("About to flush stdout for event #{}", event_count);
                    
                    // Force flush stdout to ensure messages are displayed
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    
                    tracing::info!("Event #{} fully completed, about to continue loop", event_count);
                    
                    // Add a small delay to ensure everything settles
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(None) => {
                    // Channel closed, exit the loop
                    tracing::warn!("Event channel closed, exiting event loop");
                    break;
                }
                Err(_) => {
                    // Timeout occurred, continue loop (this is normal)
                    tracing::debug!("Event loop timeout (1s) - continuing to wait");
                    continue;
                }
            }
            
            tracing::info!("End of event loop iteration {}, going to next iteration", event_count);
        }
        
        tracing::info!("Event loop finished after {} events", event_count);
        Ok(())
    }
    
    async fn start_audio_buffering(&self) -> Result<()> {
        let audio_capture = self.audio_capture.clone();
        let cancellation_token = self.cancellation_token.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        println!("🔇 Audio buffering stopped");
                        break;
                    }
                    _ = async {
                        {
                            let mut capture = audio_capture.write().await;
                            if let Err(e) = capture.start_buffering().await {
                                eprintln!("Audio buffering error: {}", e);
                            }
                        }
                        sleep(Duration::from_secs(60)).await; // Restart every 60 seconds
                    } => {}
                }
            }
        });
        
        Ok(())
    }
    
    async fn start_keyboard_listener(&self) -> Result<()> {
        println!("🎧 Starting keyboard listener with global hotkeys...");
        let cancellation_token = self.cancellation_token.clone();
        
        tokio::spawn(async move {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    println!("⌨️  Keyboard listener stopped");
                }
                _ = tokio::task::spawn_blocking(move || {
                    if let Err(e) = listen(keyboard_callback) {
                        eprintln!("Keyboard listener error: {:?}", e);
                    }
                }) => {}
            }
        });
        
        Ok(())
    }
    
    async fn handle_audio_capture(&self) -> Result<()> {
        tracing::info!("handle_audio_capture: Starting");
        
        // Check if processing with minimal lock time
        {
            let is_processing = self.is_processing.read().await;
            if *is_processing {
                tracing::info!("handle_audio_capture: Already processing, returning early");
                return Ok(());
            }
        }
        
        // Set processing flag
        {
            tracing::info!("handle_audio_capture: Setting processing flag to true");
            *self.is_processing.write().await = true;
        }
        
        // Ensure we always reset the processing flag, even on error
        let result = self.handle_audio_capture_internal().await;
        
        // Reset processing flag
        {
            tracing::info!("handle_audio_capture: Resetting processing flag");
            *self.is_processing.write().await = false;
        }
        
        // Print ready message
        tracing::info!("handle_audio_capture: Printing ready message");
        let _ = self.terminal_ui.print_ready().await;
        
        tracing::info!("handle_audio_capture: Completed");
        result
    }
    
    async fn handle_audio_capture_internal(&self) -> Result<()> {
        tracing::info!("handle_audio_capture_internal: Starting");
        
        tracing::info!("handle_audio_capture_internal: Printing status message");
        self.terminal_ui.print_status("📸 Capturing recent audio from buffer...").await?;
        
        tracing::info!("handle_audio_capture_internal: About to extract audio from buffer");
        let captured_file = {
            let mut capture = self.audio_capture.write().await;
            
            // Add timeout to prevent hanging
            let extraction_result = tokio::time::timeout(
                Duration::from_secs(30),
                capture.extract_recent_buffer(CAPTURE_DURATION)
            ).await;
            
            match extraction_result {
                Ok(Ok(file)) => {
                    tracing::info!("handle_audio_capture_internal: Audio extraction successful");
                    file
                }
                Ok(Err(e)) => {
                    tracing::error!("Audio extraction failed: {}", e);
                    self.terminal_ui.print_warning(&format!("⚠️  Audio extraction failed: {}", e)).await?;
                    return Ok(());
                }
                Err(_) => {
                    tracing::error!("Audio extraction timed out");
                    self.terminal_ui.print_warning("⚠️  Audio extraction timed out - this may indicate an issue with the audio system").await?;
                    
                    // Try to restart audio capture
                    self.terminal_ui.print_status("🔄 Attempting to restart audio capture...").await?;
                    let restart_result = capture.start_buffering().await;
                    match restart_result {
                        Ok(()) => {
                            self.terminal_ui.print_status("✅ Audio capture restarted successfully").await?;
                        }
                        Err(e) => {
                            self.terminal_ui.print_warning(&format!("⚠️  Failed to restart audio capture: {}", e)).await?;
                        }
                    }
                    
                    return Ok(());
                }
            }
        };
        
        tracing::info!("handle_audio_capture_internal: Audio extraction completed, processing file");
        
        if let Some(audio_file) = captured_file {
            tracing::info!("handle_audio_capture_internal: Starting transcription");
            // Try local transcription first, then fallback to OpenAI
            let transcript = match self.system_info.transcribe_audio(&audio_file).await {
                Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                    tracing::info!("Local transcription successful");
                    transcript
                }
                Ok(Some(transcript)) => {
                    tracing::warn!("Local transcription returned empty result, using OpenAI API");
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .context("Both local and OpenAI transcription failed")?
                }
                Ok(None) => {
                    tracing::info!("Local transcription failed, using OpenAI API");
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .context("Both local and OpenAI transcription failed")?
                }
                Err(e) => {
                    tracing::warn!("Local transcription error: {}, using OpenAI API", e);
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .context("Both local and OpenAI transcription failed")?
                }
            };
            
            tracing::info!("handle_audio_capture_internal: Transcription completed");
            
            if !transcript.trim().is_empty() {
                tracing::info!("handle_audio_capture_internal: Displaying transcript");
                self.terminal_ui.print_transcript(&transcript).await?;
                
                tracing::info!("handle_audio_capture_internal: Generating AI response");
                let context = self.build_conversation_context().await;
                let prompt = if context.is_empty() {
                    transcript.clone()
                } else {
                    format!("{}\n\nContext: {}", transcript, context)
                };
                let response = self.generate_streaming_ai_response(&prompt, None).await?;
                
                tracing::info!("handle_audio_capture_internal: Streaming AI response");
                self.terminal_ui.stream_response(&response).await?;
                
                tracing::info!("handle_audio_capture_internal: Updating session history");
                // Update session history
                self.update_session_history(&transcript, &response, QuestionType::Audio).await?;
                
                tracing::info!("handle_audio_capture_internal: Updating conversation context");
                // Update conversation context
                self.update_conversation_context(&transcript, &response).await?;
            } else {
                self.terminal_ui.print_warning("⚠️  No transcript generated - audio might be too quiet or unclear").await?;
            }
        } else {
            self.terminal_ui.print_warning("⚠️  No audio captured - buffer may be empty or too short").await?;
        }
        
        tracing::info!("handle_audio_capture_internal: Completed successfully");
        Ok(())
    }
    
    async fn handle_clipboard_analysis(&self) -> Result<()> {
        tracing::info!("handle_clipboard_analysis: Starting");
        
        // Check if processing with minimal lock time
        {
            let is_processing = self.is_processing.read().await;
            if *is_processing {
                tracing::info!("handle_clipboard_analysis: Already processing, returning early");
                return Ok(());
            }
        }
        
        // Set processing flag
        {
            tracing::info!("handle_clipboard_analysis: Setting processing flag to true");
            *self.is_processing.write().await = true;
        }
        
        // Ensure we always reset the processing flag, even on error
        let result = self.handle_clipboard_analysis_internal().await;
        
        // Reset processing flag
        {
            tracing::info!("handle_clipboard_analysis: Resetting processing flag");
            *self.is_processing.write().await = false;
        }
        
        // Print ready message
        tracing::info!("handle_clipboard_analysis: Printing ready message");
        let _ = self.terminal_ui.print_ready().await;
        
        tracing::info!("handle_clipboard_analysis: Completed");
        result
    }
    
    async fn handle_clipboard_analysis_internal(&self) -> Result<()> {
        tracing::info!("handle_clipboard_analysis_internal: Starting");
        
        self.terminal_ui.print_status("📋 Analyzing clipboard content...").await?;
        
        let clipboard_content = self.clipboard_handler.write().await.read_clipboard().await?;
        
        if let Some(content) = clipboard_content {
            let analysis = self.clipboard_handler.read().await.analyze_content_type(&content);
            
            self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
            
            let _code_id = self.store_code_in_memory(&content, &analysis).await?;
            
            let code_context = self.build_code_context().await;
            let prompt = format!(
                "Code to analyze:\n```{}\n{}\n```\n\n{}",
                analysis.language, content, code_context
            );
            let response = self.generate_streaming_ai_response(&prompt, Some("You are an expert code analyst. Provide detailed analysis of the code.")).await?;
            
            self.terminal_ui.stream_response(&response).await?;
            
            // Update session history
            self.update_session_history(&content, &response, QuestionType::Code).await?;
        }
        
        tracing::info!("handle_clipboard_analysis_internal: Completed successfully");
        Ok(())
    }
    
    async fn handle_combined_mode(&self) -> Result<()> {
        tracing::info!("handle_combined_mode: Starting");
        
        // Check if processing with minimal lock time
        {
            let is_processing = self.is_processing.read().await;
            if *is_processing {
                tracing::info!("handle_combined_mode: Already processing, returning early");
                return Ok(());
            }
        }
        
        // Set processing flag
        {
            tracing::info!("handle_combined_mode: Setting processing flag to true");
            *self.is_processing.write().await = true;
        }
        
        // Ensure we always reset the processing flag, even on error
        let result = self.handle_combined_mode_internal().await;
        
        // Reset processing flag
        {
            tracing::info!("handle_combined_mode: Resetting processing flag");
            *self.is_processing.write().await = false;
        }
        
        // Print ready message
        tracing::info!("handle_combined_mode: Printing ready message");
        let _ = self.terminal_ui.print_ready().await;
        
        tracing::info!("handle_combined_mode: Completed");
        result
    }
    
    async fn handle_combined_mode_internal(&self) -> Result<()> {
        tracing::info!("handle_combined_mode_internal: Starting");
        
        self.terminal_ui.print_status("🔗 Capturing audio with clipboard analysis...").await?;
        
        // Get audio
        let captured_file = {
            let mut capture = self.audio_capture.write().await;
            
            // Add timeout to prevent hanging
            let extraction_result = tokio::time::timeout(
                Duration::from_secs(30),
                capture.extract_recent_buffer(CAPTURE_DURATION)
            ).await;
            
            match extraction_result {
                Ok(Ok(file)) => file,
                Ok(Err(e)) => {
                    tracing::error!("Audio extraction failed: {}", e);
                    self.terminal_ui.print_warning(&format!("⚠️  Audio extraction failed: {}", e)).await?;
                    None
                }
                Err(_) => {
                    tracing::error!("Audio extraction timed out");
                    self.terminal_ui.print_warning("⚠️  Audio extraction timed out").await?;
                    None
                }
            }
        };
        
        // Get clipboard
        let clipboard_content = self.clipboard_handler.write().await.read_clipboard().await?;
        
        // Handle all possible combinations
        match (captured_file.as_ref(), clipboard_content.as_ref()) {
            (Some(audio_file), Some(content)) => {
                // Both audio and clipboard content available
                self.terminal_ui.print_status("🔗 Processing both audio and clipboard...").await?;
                
                // Try local transcription first, then fallback to OpenAI
                let transcript = match self.system_info.transcribe_audio(&audio_file).await {
                    Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                        tracing::info!("Local transcription successful");
                        transcript
                    }
                    Ok(Some(_)) | Ok(None) => {
                        tracing::info!("Local transcription failed/empty, using OpenAI API");
                        self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                        self.openai_client.transcribe_audio(&audio_file).await
                            .context("Both local and OpenAI transcription failed")?
                    }
                    Err(e) => {
                        tracing::warn!("Local transcription error: {}, using OpenAI API", e);
                        self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                        self.openai_client.transcribe_audio(&audio_file).await
                            .context("Both local and OpenAI transcription failed")?
                    }
                };
                
                let analysis = self.clipboard_handler.read().await.analyze_content_type(&content);
                
                if !transcript.trim().is_empty() {
                    self.terminal_ui.print_transcript(&transcript).await?;
                    self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
                    
                    let _code_id = self.store_code_in_memory(&content, &analysis).await?;
                    
                    let code_context = self.build_code_context().await;
                    let prompt = format!(
                        "Audio context: {}\n\nCode to analyze:\n```{}\n{}\n```\n\n{}",
                        transcript, analysis.language, content, code_context
                    );
                    let response = self.generate_streaming_ai_response(&prompt, Some("You are an expert code analyst. Analyze the provided code in the context of the audio discussion.")).await?;
                    
                    self.terminal_ui.stream_response(&response).await?;
                    
                    // Update session history
                    self.update_session_history(&transcript, &response, QuestionType::Combined).await?;
                    
                    // Update conversation context
                    self.update_conversation_context(&transcript, &response).await?;
                } else {
                    self.terminal_ui.print_warning("⚠️  No transcript generated - proceeding with code analysis only").await?;
                    
                    // Fallback to code analysis only
                    self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
                    
                    let _code_id = self.store_code_in_memory(&content, &analysis).await?;
                    
                    let response = self.openai_client.generate_code_analysis(
                        &content,
                        &analysis,
                        &self.build_code_context().await,
                    ).await?;
                    
                    self.terminal_ui.stream_response(&response).await?;
                    
                    // Update session history
                    self.update_session_history(&content, &response, QuestionType::Code).await?;
                }
            }
            (Some(audio_file), None) => {
                // Only audio available, no clipboard content
                self.terminal_ui.print_status("🔗 Processing audio only (no clipboard content)...").await?;
                
                // Try local transcription first, then fallback to OpenAI
                let transcript = match self.system_info.transcribe_audio(&audio_file).await {
                    Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                        tracing::info!("Local transcription successful");
                        transcript
                    }
                    Ok(Some(_)) | Ok(None) => {
                        tracing::info!("Local transcription failed/empty, using OpenAI API");
                        self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                        self.openai_client.transcribe_audio(&audio_file).await
                            .context("Both local and OpenAI transcription failed")?
                    }
                    Err(e) => {
                        tracing::warn!("Local transcription error: {}, using OpenAI API", e);
                        self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                        self.openai_client.transcribe_audio(&audio_file).await
                            .context("Both local and OpenAI transcription failed")?
                    }
                };
                
                if !transcript.trim().is_empty() {
                    self.terminal_ui.print_transcript(&transcript).await?;
                    
                    let response = self.openai_client.generate_meeting_support(
                        &transcript,
                        &self.build_conversation_context().await,
                    ).await?;
                    
                    self.terminal_ui.stream_response(&response).await?;
                    
                    // Update session history
                    self.update_session_history(&transcript, &response, QuestionType::Audio).await?;
                    
                    // Update conversation context
                    self.update_conversation_context(&transcript, &response).await?;
                } else {
                    self.terminal_ui.print_warning("⚠️  No transcript generated - audio might be too quiet or unclear").await?;
                }
            }
            (None, Some(content)) => {
                // Only clipboard content available, no audio
                self.terminal_ui.print_status("🔗 Processing clipboard only (no audio captured)...").await?;
                
                let analysis = self.clipboard_handler.read().await.analyze_content_type(&content);
                
                self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
                
                let _code_id = self.store_code_in_memory(&content, &analysis).await?;
                
                let response = self.openai_client.generate_code_analysis(
                    &content,
                    &analysis,
                    &self.build_code_context().await,
                ).await?;
                
                self.terminal_ui.stream_response(&response).await?;
                
                // Update session history
                self.update_session_history(&content, &response, QuestionType::Code).await?;
            }
            (None, None) => {
                // Neither audio nor clipboard content available
                self.terminal_ui.print_warning("⚠️  No audio captured and no clipboard content available").await?;
                self.terminal_ui.print_status("💡 Try copying some code to clipboard or ensuring audio is being captured").await?;
            }
        }
        
        tracing::info!("handle_combined_mode_internal: Completed successfully");
        Ok(())
    }
    
    async fn handle_screenshot_mode(&self) -> Result<()> {
        tracing::info!("handle_screenshot_mode: Starting");
        
        // Check if processing with minimal lock time
        {
            let is_processing = self.is_processing.read().await;
            if *is_processing {
                tracing::info!("handle_screenshot_mode: Already processing, returning early");
                return Ok(());
            }
        }
        
        // Set processing flag
        {
            tracing::info!("handle_screenshot_mode: Setting processing flag to true");
            *self.is_processing.write().await = true;
        }
        
        // Ensure we always reset the processing flag, even on error
        let result = self.handle_screenshot_mode_internal().await;
        
        // Reset processing flag
        {
            tracing::info!("handle_screenshot_mode: Resetting processing flag");
            *self.is_processing.write().await = false;
        }
        
        // Print ready message
        tracing::info!("handle_screenshot_mode: Printing ready message");
        let _ = self.terminal_ui.print_ready().await;
        
        tracing::info!("handle_screenshot_mode: Completed");
        result
    }
    
    async fn handle_screenshot_mode_internal(&self) -> Result<()> {
        tracing::info!("handle_screenshot_mode_internal: Starting");
        
        self.terminal_ui.print_status("📸 Capturing screenshot with audio...").await?;
        
        // Capture screenshot
        let screenshot_path = self.system_info.capture_active_window().await?;
        
        // Get audio
        let captured_file = {
            let mut capture = self.audio_capture.write().await;
            
            // Add timeout to prevent hanging
            let extraction_result = tokio::time::timeout(
                Duration::from_secs(30),
                capture.extract_recent_buffer(CAPTURE_DURATION)
            ).await;
            
            match extraction_result {
                Ok(Ok(file)) => file,
                Ok(Err(e)) => {
                    tracing::error!("Audio extraction failed: {}", e);
                    self.terminal_ui.print_warning(&format!("⚠️  Audio extraction failed: {}", e)).await?;
                    None
                }
                Err(_) => {
                    tracing::error!("Audio extraction timed out");
                    self.terminal_ui.print_warning("⚠️  Audio extraction timed out - proceeding with screenshot analysis only").await?;
                    None
                }
            }
        };
        
        if let (Some(screenshot), Some(audio_file)) = (screenshot_path.as_ref(), captured_file.as_ref()) {
            // Try local transcription first, then fallback to OpenAI
            let transcript = match self.system_info.transcribe_audio(&audio_file).await {
                Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                    tracing::info!("Local transcription successful");
                    transcript
                }
                Ok(Some(transcript)) => {
                    tracing::warn!("Local transcription returned empty result, using OpenAI API");
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .unwrap_or_else(|_| "Analyze what you see in the screenshot".to_string())
                }
                Ok(None) => {
                    tracing::info!("Local transcription failed, using OpenAI API");
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .unwrap_or_else(|_| "Analyze what you see in the screenshot".to_string())
                }
                Err(e) => {
                    tracing::warn!("Local transcription error: {}, using OpenAI API", e);
                    self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
                    self.openai_client.transcribe_audio(&audio_file).await
                        .unwrap_or_else(|_| "Analyze what you see in the screenshot".to_string())
                }
            };
            
            let audio_context = if !transcript.trim().is_empty() {
                self.terminal_ui.print_transcript(&transcript).await?;
                transcript
            } else {
                "Analyze what you see in the screenshot".to_string()
            };
            
            self.terminal_ui.print_status("📸 Screenshot captured from active window").await?;
            
            // Note: Screenshot analysis still uses OpenAI client as it requires vision capabilities
            // TODO: Extend plugin system to support vision models like GPT-4V, Claude Vision, etc.
            let response = self.openai_client.generate_screenshot_with_audio_analysis(
                &audio_context,
                &screenshot,
            ).await?;
            
            self.terminal_ui.stream_response(&response).await?;
            
            // Update session history
            self.update_session_history(&audio_context, &response, QuestionType::Screenshot).await?;
            
            // Update conversation context
            self.update_conversation_context(&audio_context, &response).await?;
        } else if let Some(screenshot) = screenshot_path {
            // Handle case where audio capture failed but we have a screenshot
            self.terminal_ui.print_warning("⚠️  No audio captured - proceeding with screenshot analysis only").await?;
            
            let audio_context = "Analyze what you see in the screenshot".to_string();
            
            self.terminal_ui.print_status("📸 Screenshot captured from active window").await?;
            
            // Note: Screenshot analysis still uses OpenAI client as it requires vision capabilities
            // TODO: Extend plugin system to support vision models like GPT-4V, Claude Vision, etc.
            let response = self.openai_client.generate_screenshot_with_audio_analysis(
                &audio_context,
                &screenshot,
            ).await?;
            
            self.terminal_ui.stream_response(&response).await?;
            
            // Update session history
            self.update_session_history(&audio_context, &response, QuestionType::Screenshot).await?;
            
            // Update conversation context
            self.update_conversation_context(&audio_context, &response).await?;
        }
        
        tracing::info!("handle_screenshot_mode_internal: Completed successfully");
        Ok(())
    }
    
    async fn handle_cancel(&self) -> Result<()> {
        if !*self.is_processing.read().await {
            self.terminal_ui.print_warning("⚠️  No active request to cancel").await?;
            return Ok(());
        }
        
        *self.should_cancel.write().await = true;
        *self.is_processing.write().await = false;
        
        self.terminal_ui.print_status("🛑 Request cancelled").await?;
        self.terminal_ui.print_ready().await?;
        
        Ok(())
    }
    
    async fn show_session_history(&self) -> Result<()> {
        let history = self.session_history.read().await;
        let summary = self.conversation_summary.read().await;
        let code_memory = self.code_memory.read().await;
        
        self.terminal_ui.print_session_history(&history, &summary, &code_memory).await?;
        
        Ok(())
    }
    
    async fn clear_conversation_context(&self) -> Result<()> {
        self.conversation_context.write().await.clear();
        self.conversation_summary.write().await.clear();
        self.code_memory.write().await.clear();
        
        self.terminal_ui.print_status("🧹 Conversation context and code memory cleared").await?;
        
        Ok(())
    }
    
    async fn build_conversation_context(&self) -> String {
        let context = self.conversation_context.read().await;
        let summary = self.conversation_summary.read().await;
        
        if context.is_empty() {
            return String::new();
        }
        
        let mut result = format!("CONVERSATION SUMMARY: {}\n\nRECENT EXCHANGES:\n", summary);
        
        for (i, entry) in context.iter().rev().take(3).enumerate() {
            result.push_str(&format!(
                "{}. Q: \"{}\" ({})\n   Topics: {}\n",
                i + 1,
                entry.question,
                entry.question_type,
                entry.key_topics.join(", ")
            ));
        }
        
        result
    }
    
    async fn build_code_context(&self) -> String {
        let code_memory = self.code_memory.read().await;
        
        if code_memory.is_empty() {
            return String::new();
        }
        
        let mut result = "\nPREVIOUS CODE CONTEXT:\n".to_string();
        
        for entry in code_memory.iter() {
            result.push_str(&format!(
                "\nCODE #{} ({} - {}):\n{}\n---\n",
                entry.id,
                entry.language,
                entry.analysis_type,
                if entry.code.len() > 300 {
                    format!("{}...", &entry.code[..300])
                } else {
                    entry.code.clone()
                }
            ));
        }
        
        result
    }
    
    async fn store_code_in_memory(&self, code: &str, analysis: &ContentAnalysis) -> Result<usize> {
        let mut code_memory = self.code_memory.write().await;
        
        let entry = CodeEntry {
            id: code_memory.len() + 1,
            timestamp: chrono::Utc::now(),
            code: code.to_string(),
            language: analysis.language.clone(),
            analysis_type: analysis.content_type.clone(),
            description: format!("{} snippet", analysis.content_type),
            preview: if code.len() > 100 {
                format!("{}...", &code[..100])
            } else {
                code.to_string()
            },
        };
        
        let id = entry.id;
        code_memory.push(entry);
        
        // Keep only last 5 entries
        if code_memory.len() > 5 {
            let excess = code_memory.len() - 5;
            code_memory.drain(0..excess);
            // Renumber IDs
            for (i, entry) in code_memory.iter_mut().enumerate() {
                entry.id = i + 1;
            }
        }
        
        Ok(id)
    }
    
    async fn update_session_history(&self, input: &str, response: &str, question_type: QuestionType) -> Result<()> {
        let mut history = self.session_history.write().await;
        
        let entry = SessionEntry {
            timestamp: chrono::Utc::now(),
            input: input.to_string(),
            response: response.to_string(),
            question_type,
            confidence: 0.9, // Default confidence
            key_topics: vec![], // Extract from response if needed
        };
        
        history.insert(0, entry);
        
        // Keep only last 5 entries
        if history.len() > 5 {
            history.truncate(5);
        }
        
        Ok(())
    }
    
    async fn update_conversation_context(&self, question: &str, response: &str) -> Result<()> {
        {
            let mut context = self.conversation_context.write().await;
            
            let entry = ConversationEntry {
                timestamp: chrono::Utc::now(),
                question: question.to_string(),
                question_type: "general".to_string(),
                key_topics: vec![], // Extract from response if needed
                response: response.to_string(),
            };
            
            context.push(entry);
            
            // Keep only last 10 entries
            if context.len() > 10 {
                let excess = context.len() - 10;
                context.drain(0..excess);
            }
        } // Drop the write lock here
        
        // Update summary after dropping the lock to avoid deadlock
        self.update_conversation_summary().await?;
        
        Ok(())
    }
    
    /// Generate AI response using the active LLM provider (plugin system or OpenAI fallback)
    async fn generate_ai_response(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        let options = LLMOptions {
            max_tokens: Some(self.config.openai.max_tokens),
            temperature: Some(self.config.openai.temperature),
            model: Some(self.config.openai.model.clone()),
            system_prompt: system_prompt.map(|s| s.to_string()),
            streaming: false,
        };

        // Try plugin system first
        if let Ok(Some(response)) = self.plugin_manager.generate_completion(prompt, &options).await {
            return Ok(response);
        }

        // Fallback to OpenAI if enabled and no plugin is active or plugin failed
        if self.config.llm_provider.fallback_to_openai {
            tracing::info!("Using OpenAI fallback for LLM generation");
            return self.openai_client.generate_meeting_support(prompt, "").await;
        }

        Err(anyhow::anyhow!("No LLM provider available"))
    }

    /// Generate streaming AI response using the active LLM provider
    async fn generate_streaming_ai_response(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        let options = LLMOptions {
            max_tokens: Some(self.config.openai.max_tokens),
            temperature: Some(self.config.openai.temperature),
            model: Some(self.config.openai.model.clone()),
            system_prompt: system_prompt.map(|s| s.to_string()),
            streaming: true,
        };

        // Try plugin system first
        if let Ok(Some(mut stream)) = self.plugin_manager.generate_streaming_completion(prompt, &options).await {
            let mut full_response = String::new();
            
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if !chunk.is_empty() {
                            print!("{}", chunk);
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                            full_response.push_str(&chunk);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Stream chunk error: {}", e);
                        break;
                    }
                }
            }
            
            if !full_response.is_empty() {
                return Ok(full_response);
            }
        }

        // Fallback to OpenAI if enabled
        if self.config.llm_provider.fallback_to_openai {
            tracing::info!("Using OpenAI fallback for streaming LLM generation");
            return self.openai_client.generate_meeting_support(prompt, "").await;
        }

        Err(anyhow::anyhow!("No LLM provider available"))
    }
    
    async fn update_conversation_summary(&self) -> Result<()> {
        let context = self.conversation_context.read().await;
        
        if context.is_empty() {
            return Ok(());
        }
        
        let mut summary = self.conversation_summary.write().await;
        
        if context.len() <= 1 {
            *summary = "Meeting started".to_string();
        } else {
            *summary = format!("Meeting with {} exchanges", context.len());
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Handle commands
    match cli.command {
        Some(Commands::Setup { non_interactive, force }) => {
            if non_interactive {
                println!("🔧 Running automated setup...");
                // TODO: Implement non-interactive setup
                println!("❌ Non-interactive setup not yet implemented. Use interactive mode.");
                return Ok(());
            }
            
            if force {
                println!("🔄 Force reinstalling all dependencies...");
            }
            
            // Run interactive setup
            run_setup().await?;
            return Ok(());
        }
        
        Some(Commands::Status) => {
            // Show system status
            show_system_status().await?;
            return Ok(());
        }
        
        Some(Commands::Run) | None => {
            // Run the main application (default)
            run_main_application().await?;
        }
        
        Some(Commands::Plugin { command }) => {
            // Handle plugin commands
            handle_plugin_command(command).await?;
        }
    }
    
    Ok(())
}

async fn run_main_application() -> Result<()> {
    // Initialize file-based logging
    setup_logging().await?;
    
    // Create and run the application
    let (app, event_rx) = MeetingAssistant::new().await?;
    let result = app.run(event_rx).await;
    
    // Ensure all tasks are cancelled
    app.cancellation_token.cancel();
    
    // Final cleanup
    {
        let mut audio_capture = app.audio_capture.write().await;
        let _ = audio_capture.stop_buffering().await;
    }
    
    result
}

async fn show_system_status() -> Result<()> {
    println!("🔍 Meeting Assistant System Status");
    println!("================================");
    println!();
    
    // Check configuration
    let config_exists = std::path::Path::new(".env").exists();
    let config_status = if config_exists { "✅" } else { "❌" };
    println!("Configuration (.env): {}", config_status);
    
    // Check dependencies
    let ffmpeg_status = if check_command("ffmpeg").await { "✅" } else { "❌" };
    println!("FFmpeg: {}", ffmpeg_status);
    
    let rust_status = if check_command("cargo").await { "✅" } else { "❌" };
    println!("Rust/Cargo: {}", rust_status);
    
    // Check Whisper backends
    let whisper_backends = detect_whisper_backends().await;
    if whisper_backends.is_empty() {
        println!("Whisper backends: ❌ None found");
    } else {
        println!("Whisper backends: ✅ {} found", whisper_backends.len());
        for backend in whisper_backends {
            println!("  • {}", backend);
        }
    }
    
    // Check build status
    let app_built = std::path::Path::new("target/release/meeting-assistant").exists();
    let build_status = if app_built { "✅" } else { "❌" };
    println!("Application built: {}", build_status);
    
    // Check OpenAI API key
    let api_key_status = if std::env::var("OPENAI_API_KEY").is_ok() || 
                              (config_exists && std::fs::read_to_string(".env").unwrap_or_default().contains("OPENAI_API_KEY=")) {
        "✅"
    } else {
        "❌"
    };
    println!("OpenAI API Key: {}", api_key_status);
    
    println!();
    
    if !config_exists || !app_built {
        println!("🚀 Run setup: ./target/release/meeting-assistant setup");
    } else {
        println!("🎯 Ready to use: ./target/release/meeting-assistant");
    }
    
    Ok(())
}

async fn check_command(command: &str) -> bool {
    use std::process::Command;
    
    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

async fn detect_whisper_backends() -> Vec<String> {
    let mut backends = Vec::new();
    
    if check_command("whisper-cpp").await || check_command("whisper-cli").await {
        backends.push("whisper.cpp (ultra-fast)".to_string());
    }
    
    if check_command("whisper").await {
        backends.push("whisper (fast)".to_string());
    }
    
    if check_command("faster-whisper").await {
        backends.push("faster-whisper (good)".to_string());
    }
    
    backends
}

async fn setup_logging() -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    
    // Create log directory
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".meeting-assistant")
        .join("logs");
    
    std::fs::create_dir_all(&log_dir)
        .context("Failed to create log directory")?;
    
    // Create rolling file appender (daily rotation)
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        &log_dir,
        "meeting-assistant.log"
    );
    
    // Set up layered logging:
    // - File: All logs (DEBUG and above)
    // - Stdout: Only ERROR logs (user-facing errors)
    let subscriber = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_filter(
                    EnvFilter::from_default_env()
                        .add_directive("meeting_assistant=debug".parse()?)
                        .add_directive("debug".parse()?)
                )
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_target(false)
                .with_thread_ids(false)
                .compact()
                .with_filter(
                    EnvFilter::from_default_env()
                        .add_directive("meeting_assistant=error".parse()?)
                        .add_directive("error".parse()?)
                )
        );
    
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;
    
    tracing::info!("Logging system initialized");
    
    // Print user-friendly message about logging
    println!("📝 Debug logs are being saved to: {}", log_dir.display());
    println!("   Use 'tail -f {}' to monitor logs in real-time", 
             log_dir.join("meeting-assistant.log").display());
    println!();
    
    Ok(())
}

async fn handle_plugin_command(command: PluginCommand) -> Result<()> {
    // Initialize plugin manager for commands
    let config = Config::load().await?;
    let temp_dir = dirs::home_dir()
        .context("Failed to get home directory")?
        .join(".meeting-assistant")
        .join("temp");
    std::fs::create_dir_all(&temp_dir)?;
    let plugin_manager = PluginManager::new(config, temp_dir)?;
    let registry = PluginRegistry::new()?;
    
    match command {
        PluginCommand::Install { source, branch } => {
            println!("🔧 Installing plugin from: {}", source);
            
            let plugin_source = parse_plugin_source(&source, branch)?;
            let plugin_id = plugin_manager.install_plugin(plugin_source).await?;
            
            println!("✅ Successfully installed plugin: {}", plugin_id);
        }
        
        PluginCommand::List => {
            println!("📋 Installed plugins:");
            let plugins = plugin_manager.list_plugins().await;
            
            if plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                for (_, info) in plugins {
                    let status = if info.enabled { "✅" } else { "❌" };
                    println!("  {} {} v{} - {} ({})", 
                        status, info.name, info.version, info.description, info.author);
                }
            }
        }
        
        PluginCommand::Search { query } => {
            println!("🔍 Searching for plugins: {}", query);
            let results = registry.search_plugins(&query).await?;
            
            if results.is_empty() {
                println!("No plugins found matching: {}", query);
            } else {
                for plugin in results {
                    println!("  {} v{} - {} ({})", 
                        plugin.name, plugin.version, plugin.description, plugin.author);
                }
            }
        }
        
        PluginCommand::Info { name } => {
            if let Some(info) = registry.get_plugin_info(&name).await? {
                println!("📦 Plugin: {}", info.name);
                println!("Version: {}", info.version);
                println!("Description: {}", info.description);
                println!("Author: {}", info.author);
                println!("Type: {:?}", info.plugin_type);
                println!("Enabled: {}", info.enabled);
            } else {
                println!("Plugin '{}' not found", name);
            }
        }
        
        PluginCommand::Enable { name } => {
            println!("✅ Enabling plugin: {}", name);
            // TODO: Implement plugin enable/disable
            println!("Plugin management not yet implemented");
        }
        
        PluginCommand::Disable { name } => {
            println!("❌ Disabling plugin: {}", name);
            // TODO: Implement plugin enable/disable
            println!("Plugin management not yet implemented");
        }
        
        PluginCommand::Uninstall { name } => {
            println!("🗑️  Uninstalling plugin: {}", name);
            // TODO: Implement plugin uninstall
            println!("Plugin management not yet implemented");
        }
        
        PluginCommand::Update { name } => {
            println!("🔄 Updating plugin: {}", name);
            // TODO: Implement plugin update
            println!("Plugin management not yet implemented");
        }
        
        PluginCommand::SetLlm { provider } => {
            println!("🤖 Setting LLM provider to: {}", provider);
            plugin_manager.set_active_llm_provider(provider).await?;
            println!("✅ LLM provider set successfully");
        }
    }
    
    Ok(())
}

fn parse_plugin_source(source: &str, branch: Option<String>) -> Result<PluginSource> {
    if source.starts_with("github:") {
        let repo_path = source.strip_prefix("github:").unwrap();
        let parts: Vec<&str> = repo_path.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid GitHub format. Use: github:owner/repo"));
        }
        Ok(PluginSource::GitHub {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
            branch,
        })
    } else if source.starts_with("local:") {
        let path = source.strip_prefix("local:").unwrap();
        Ok(PluginSource::Local {
            path: PathBuf::from(path),
        })
    } else if source.starts_with("http://") || source.starts_with("https://") {
        Ok(PluginSource::Http {
            url: source.to_string(),
        })
    } else if source.starts_with("git:") {
        let url = source.strip_prefix("git:").unwrap();
        Ok(PluginSource::Git {
            url: url.to_string(),
            branch,
        })
    } else {
        // Default to GitHub if no prefix
        let parts: Vec<&str> = source.split('/').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid format. Use: owner/repo or github:owner/repo"));
        }
        Ok(PluginSource::GitHub {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
            branch,
        })
    }
} 
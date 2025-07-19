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
use std::fs;
use std::path::PathBuf;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use anyhow::{Result, Context};
use rdev::{listen, Event, EventType};
use parking_lot::Mutex;
use lazy_static::lazy_static;
use clap::{Parser, Subcommand};
use futures::StreamExt;

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
mod meeting_recorder;


use serde_json::json;

use audio::AudioCapture;
use ai::OpenAIClient;
use input::{KeyboardHandler, ClipboardHandler};
use ui::TerminalUI;
use system::SystemInfo;
use config::{Config, LLMProvider};
use types::*;
use types::SystemStatus;
use setup::run_setup;
use plugin_system::*;
use plugins::{OllamaProvider, SentimentAnalyzerPlugin, STTPostProcessorPlugin, SpectralDiarizationPlugin, AdvancedDiarizationPlugin, create_transcript_interactive_plugin, TranscriptInteractivePlugin};
use meeting_recorder::{MeetingRecorder, RecordingEvent, RecordingEventReceiver};

use std::io::{self, Write};
use colored::*;

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
    
    /// Meeting recording management commands
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    
    /// Transcript management commands
    Transcript {
        #[command(subcommand)]
        command: TranscriptCommand,
    },
    

}

#[derive(Subcommand)]
enum RecordCommand {
    /// Start recording a meeting
    Start {
        /// Optional title for the meeting
        #[arg(short, long)]
        title: Option<String>,
    },
    
    /// Stop the current recording
    Stop,
    
    /// Pause the current recording
    Pause,
    
    /// Resume a paused recording
    Resume,
    
    /// Show current recording status
    Status,
    
    /// Test FFmpeg setup and audio device availability
    Test,
    
    /// List all recordings
    List,
    
    /// Delete a recording
    Delete {
        /// Recording ID to delete
        id: String,
    },
    
    /// Show information about a recording
    Info {
        /// Recording ID to show info for
        id: String,
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

#[derive(Subcommand)]
enum TranscriptCommand {
    /// List all available audio files for transcription
    List,
    
    /// Generate transcript for a specific audio file
    Generate {
        /// Path to the audio file
        file: PathBuf,
    },
    
    /// Advanced speaker diarization using Whisper + PyAnnote
    Diarize {
        /// Path to the audio file
        file: PathBuf,
        
        /// Whisper model size to use (tiny, base, small, medium, large)
        #[arg(long, default_value = "base")]
        model: String,
        
        /// Maximum number of speakers to detect
        #[arg(long)]
        max_speakers: Option<usize>,
        
        /// Minimum number of speakers to detect
        #[arg(long)]
        min_speakers: Option<usize>,
        
        /// Output format (json, text, detailed)
        #[arg(long, default_value = "detailed")]
        format: String,
    },
    
    /// Advanced speaker diarization for the latest audio file
    DiarizeLatest {
        /// Whisper model size to use (tiny, base, small, medium, large)
        #[arg(long, default_value = "base")]
        model: String,
        
        /// Maximum number of speakers to detect
        #[arg(long)]
        max_speakers: Option<usize>,
        
        /// Minimum number of speakers to detect
        #[arg(long)]
        min_speakers: Option<usize>,
        
        /// Output format (json, text, detailed)
        #[arg(long, default_value = "detailed")]
        format: String,
    },
    
    /// Reprocess all audio files to generate new transcripts
    Reprocess,
    
    /// Show a specific transcript
    Show {
        /// Transcript ID
        id: String,
    },
    
    /// Show processing status for audio files
    Status,
    
    /// Interactive transcript analysis with AI
    Interactive,
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
        
        // Handle immediate key press
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
        
        // Check for pending events that might be ready
        if let Some(pending_event) = GLOBAL_KEYBOARD_HANDLER.lock().check_pending_events() {
            tracing::info!("Keyboard callback found pending event: {:?}", pending_event);
            
            if let Some(sender) = KEYBOARD_CHANNEL.lock().as_ref() {
                match sender.send(pending_event) {
                    Ok(_) => {
                        tracing::info!("Pending event sent successfully to channel");
                    }
                    Err(e) => {
                        tracing::error!("Failed to send pending event to channel: {}", e);
                    }
                }
            } else {
                tracing::warn!("No keyboard channel available for pending event");
            }
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
    
    // Meeting recording
    meeting_recorder: Arc<MeetingRecorder>,
    recording_event_rx: Arc<RwLock<RecordingEventReceiver>>,
    
    // State management
    is_processing: Arc<RwLock<bool>>,
    should_cancel: Arc<RwLock<bool>>,
    session_history: Arc<RwLock<Vec<SessionEntry>>>,
    conversation_context: Arc<RwLock<Vec<ConversationEntry>>>,
    conversation_summary: Arc<RwLock<String>>,
    code_memory: Arc<RwLock<Vec<CodeEntry>>>,
    system_status: Arc<RwLock<SystemStatus>>,
    
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
        
        // Register STT post-processor
        let mut stt_plugin = STTPostProcessorPlugin::new();
        plugin_manager.register_plugin("stt_post_processor".to_string(), Box::new(stt_plugin)).await?;
        
        // Register Whisper + PyAnnote diarization plugin
        let advanced_diarization_plugin = AdvancedDiarizationPlugin::new();
        plugin_manager.register_plugin("advanced_diarization".to_string(), Box::new(advanced_diarization_plugin)).await?;
        
        // Register transcript interactive plugin
        let transcript_interactive_plugin = create_transcript_interactive_plugin();
        plugin_manager.register_plugin("transcript_interactive".to_string(), transcript_interactive_plugin).await?;
        
        // Set active LLM provider based on configuration
        match &config.llm_provider.active_provider {
            LLMProvider::Ollama => {
                match plugin_manager.set_active_llm_provider("ollama".to_string()).await {
                    Ok(()) => {
                        println!("🦙 Using Ollama as LLM provider");
                    }
                    Err(e) => {
                        println!("❌ Failed to set Ollama as active provider: {}", e);
                        println!("🤖 Falling back to OpenAI");
                    }
                }
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
        
        // Set up transcription services for STT plugin after plugin manager is created
        // This is a bit of a hack, but necessary since we need the plugin manager itself
        // to be available to the STT plugin for transcription fallback
        {
            let mut plugins = plugin_manager.get_plugins().write().await;
            if let Some(stt_plugin) = plugins.get_mut("stt_post_processor") {
                // We need to downcast to access the set_transcription_services method
                // This is safe because we know we just registered an STTPostProcessorPlugin
                if let Some(stt_plugin) = stt_plugin.as_any_mut().downcast_mut::<STTPostProcessorPlugin>() {
                    stt_plugin.set_transcription_services(
                        system_info.clone(),
                        openai_client.clone(),
                        plugin_manager.clone()
                    );
                }
            }
        }
        
        // Initialize meeting recorder
        let (meeting_recorder, recording_event_rx) = MeetingRecorder::new(&config)?;
        let mut meeting_recorder = meeting_recorder;
        meeting_recorder.set_plugin_manager(plugin_manager.clone());
        let meeting_recorder = Arc::new(meeting_recorder);
        let recording_event_rx = Arc::new(RwLock::new(recording_event_rx));
        
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
            meeting_recorder,
            recording_event_rx,
            is_processing: Arc::new(RwLock::new(false)),
            should_cancel: Arc::new(RwLock::new(false)),
            session_history: Arc::new(RwLock::new(Vec::new())),
            conversation_context: Arc::new(RwLock::new(Vec::new())),
            conversation_summary: Arc::new(RwLock::new(String::new())),
            code_memory: Arc::new(RwLock::new(Vec::new())),
            system_status: Arc::new(RwLock::new(SystemStatus::new())),
            event_tx,
            cancellation_token,
        };
        
        Ok((assistant, event_rx))
    }
    
    pub async fn run(&self, event_rx: mpsc::UnboundedReceiver<AppEvent>) -> Result<()> {
        // Initialize system status
        {
            let mut status = self.system_status.write().await;
            status.audio_ready = true; // We'll update this properly later
            status.openai_ready = true;
            status.plugins_ready = false; // Will be set to true after plugin initialization
            status.whisper_ready = true;
        }
        
        // Setup terminal with initial status
        {
            let status = self.system_status.read().await;
            self.terminal_ui.print_welcome(&status).await?;
        }
        
        // Initialize plugins
        self.plugin_manager.initialize_plugins().await?;
        
        // Update system status after plugin initialization
        {
            let mut status = self.system_status.write().await;
            status.plugins_ready = true;
        }
        
        // Check system status (re-enabled)
        // Note: SystemInfo.check_system_status needs &mut self but we have &self
        // We'll skip this for now and add it back later with proper design
        
        // Start background tasks
        self.start_audio_buffering().await?;
        self.start_continuous_audio_processing().await?;
        self.start_keyboard_listener().await?;
        self.start_meeting_recording().await?;
        self.start_recording_event_handler().await?;
        
        // Setup ctrl+c handler
        let event_tx = self.event_tx.clone();
        ctrlc::set_handler(move || {
            println!("\n🛑 Ctrl+C pressed - shutting down...");
            let _ = event_tx.send(AppEvent::Shutdown);
            
            // Force exit after 10 seconds if shutdown doesn't complete (reduced from 30s)
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(10));
                println!("🚫 Force exiting after timeout...");
                std::process::exit(1);
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
                Duration::from_millis(50), // Reduced timeout for more frequent pending event checks
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
                            
                            // Stop meeting recording if active
                            if self.meeting_recorder.is_recording().await {
                                let _ = self.terminal_ui.print_status("🛑 Stopping meeting recording...").await;
                                match self.meeting_recorder.stop_recording().await {
                                    Ok(Some(recording_info)) => {
                                        let _ = self.terminal_ui.print_status(&format!("✅ Recording saved: {} ({:.1}s, {:.1}MB)", 
                                            recording_info.id, recording_info.duration_seconds, recording_info.file_size_mb())).await;
                                    }
                                    Ok(None) => {
                                        let _ = self.terminal_ui.print_status("ℹ️  No active recording to stop").await;
                                    }
                                    Err(e) => {
                                        let _ = self.terminal_ui.print_warning(&format!("⚠️  Error stopping recording: {}", e)).await;
                                    }
                                }
                            }
                            
                            // Check if any transcript generation capability is available
                            if self.is_transcript_generation_available().await {
                                println!();
                                println!("{}", "🎯 Transcript Generation Available".cyan());
                                println!("{}", "   Generate a transcript from recent meeting audio".bright_black());
                                
                                if let Ok(true) = self.ask_yes_no("📝 Would you like to generate a transcript for this meeting?").await {
                                    if let Err(e) = self.generate_transcript().await {
                                        println!("{}", format!("⚠️  Error generating transcript: {}", e).yellow());
                                    }
                                } else {
                                    println!("{}", "⏭️  Transcript generation skipped".bright_black());
                                }
                            }
                            
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
                            
                            // Force exit to ensure application terminates after shutdown interactions
                            std::process::exit(0);
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
                    // Timeout occurred, check for pending events
                    tracing::debug!("Event loop timeout (50ms) - checking for pending events");
                    
                    // Check for pending events that might be ready
                    if let Some(pending_event) = GLOBAL_KEYBOARD_HANDLER.lock().check_pending_events() {
                        tracing::info!("Event loop found pending event: {:?}", pending_event);
                        
                        // Process the pending event just like any other event
                        event_count += 1;
                        tracing::info!("Processing pending event #{}: {:?}", event_count, pending_event);
                        
                        // Handle the pending event
                        let event_result = match pending_event {
                            AppEvent::AudioCapture => {
                                tracing::info!("Handling pending audio capture event #{}", event_count);
                                let result = self.handle_audio_capture().await;
                                tracing::info!("Pending audio capture event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::ClipboardAnalysis => {
                                tracing::info!("Handling pending clipboard analysis event #{}", event_count);
                                let result = self.handle_clipboard_analysis().await;
                                tracing::info!("Pending clipboard analysis event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::CombinedMode => {
                                tracing::info!("Handling pending combined mode event #{}", event_count);
                                let result = self.handle_combined_mode().await;
                                tracing::info!("Pending combined mode event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::ScreenshotMode => {
                                tracing::info!("Handling pending screenshot mode event #{}", event_count);
                                let result = self.handle_screenshot_mode().await;
                                tracing::info!("Pending screenshot mode event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::Cancel => {
                                tracing::info!("Handling pending cancel event #{}", event_count);
                                let result = self.handle_cancel().await;
                                tracing::info!("Pending cancel event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::ShowHistory => {
                                tracing::info!("Handling pending show history event #{}", event_count);
                                let result = self.show_session_history().await;
                                tracing::info!("Pending show history event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::ClearContext => {
                                tracing::info!("Handling pending clear context event #{}", event_count);
                                let result = self.clear_conversation_context().await;
                                tracing::info!("Pending clear context event #{} completed with result: {:?}", event_count, result.is_ok());
                                result
                            }
                            AppEvent::Shutdown => {
                                tracing::info!("Handling pending shutdown event #{}", event_count);
                                self.terminal_ui.print_shutdown().await?;
                                
                                // Check if any transcript generation capability is available
                                if self.is_transcript_generation_available().await {
                                    println!();
                                    println!("{}", "🎯 Transcript Generation Available".cyan());
                                    println!("{}", "   Generate a transcript from recent meeting audio".bright_black());
                                    
                                    if let Ok(true) = self.ask_yes_no("📝 Would you like to generate a transcript for this meeting?").await {
                                        if let Err(e) = self.generate_transcript().await {
                                            println!("{}", format!("⚠️  Error generating transcript: {}", e).yellow());
                                        }
                                    } else {
                                        println!("{}", "⏭️  Transcript generation skipped".bright_black());
                                    }
                                }
                                
                                self.cancellation_token.cancel();
                                {
                                    let mut global_channel = KEYBOARD_CHANNEL.lock();
                                    *global_channel = None;
                                }
                                {
                                    let mut audio_capture = self.audio_capture.write().await;
                                    let _ = audio_capture.stop_buffering().await;
                                }
                                sleep(Duration::from_millis(500)).await;
                                println!("👋 Goodbye!");
                                
                                // Force exit to ensure application terminates after shutdown interactions
                                std::process::exit(0);
                            }
                        };
                        
                        // Handle the result
                        if let Err(e) = event_result {
                            tracing::error!("Error handling pending event #{}: {}", event_count, e);
                            *self.is_processing.write().await = false;
                            let _ = self.terminal_ui.print_warning(&format!("⚠️  Error: {}", e)).await;
                            let _ = self.terminal_ui.print_ready().await;
                        } else {
                            tracing::info!("Pending event #{} processed successfully", event_count);
                        }
                        
                        // Force flush stdout
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        
                        // Add a small delay to ensure everything settles
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    
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

    async fn start_continuous_audio_processing(&self) -> Result<()> {
        let audio_capture = self.audio_capture.clone();
        let plugin_manager = self.plugin_manager.clone();
        let terminal_ui = self.terminal_ui.clone();
        let cancellation_token = self.cancellation_token.clone();
        let system_info = self.system_info.clone();
        let openai_client = self.openai_client.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15)); // Process every 15 seconds for longer context
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            
            // Simple deduplication cache to avoid processing the same content multiple times
            let mut recent_transcripts: std::collections::VecDeque<String> = std::collections::VecDeque::with_capacity(10);
            
            println!("🔄 Starting continuous audio processing every 15 seconds");
            println!("📝 Capturing 30-second chunks for longer, more contextual utterances");
            
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        println!("🛑 Continuous audio processing stopped");
                        break;
                    }
                    _ = interval.tick() => {
                        // Check if any plugins want continuous processing
                        let plugins_list = plugin_manager.list_plugins().await;
                        let has_continuous_plugin = plugins_list.iter().any(|(name, _)| {
                            name.contains("meeting_storage")
                        });
                        
                        if has_continuous_plugin {
                            // Extract longer audio chunks for more contextual utterances
                            let captured_file = {
                                let mut capture = audio_capture.write().await;
                                match capture.extract_recent_buffer(30).await { // 30 second chunks for longer context
                                    Ok(Some(file)) => Some(file),
                                    Ok(None) => None,
                                    Err(e) => {
                                        tracing::debug!("Continuous audio extraction failed: {}", e);
                                        None
                                    }
                                }
                            };
                            
                            if let Some(audio_file) = captured_file {
                                tracing::debug!("Processing continuous audio: {:?}", audio_file);
                                
                                // Fire audio captured event
                                let event = PluginEvent::AudioCaptured { file_path: audio_file.clone() };
                                if let Err(e) = plugin_manager.fire_event(event).await {
                                    tracing::warn!("Failed to fire AudioCaptured event: {}", e);
                                }
                                
                                // Transcribe audio using the same fallback logic as main app
                                let transcript_result = {
                                    // Try local transcription first
                                    match system_info.transcribe_audio(&audio_file).await {
                                        Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                                            Ok(transcript)
                                        }
                                        Ok(Some(_)) | Ok(None) | Err(_) => {
                                            // Try plugin system for transcription
                                            match plugin_manager.transcribe_audio(&audio_file).await {
                                                Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                                                    Ok(transcript)
                                                }
                                                Ok(Some(_)) | Ok(None) | Err(_) => {
                                                    // Fallback to OpenAI if enabled
                                                    if config.llm_provider.fallback_to_openai {
                                                        openai_client.transcribe_audio(&audio_file).await
                                                    } else {
                                                        Err(anyhow::anyhow!("No transcription service available"))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                };
                                
                                match transcript_result {
                                    Ok(transcript) if !transcript.trim().is_empty() => {
                                        let clean_transcript = transcript.trim().to_lowercase();
                                        
                                        // Check for deduplication - avoid processing very similar recent transcripts
                                        let is_duplicate = recent_transcripts.iter().any(|recent| {
                                            let similarity = calculate_text_similarity(&clean_transcript, recent);
                                            similarity > 0.85 // 85% similarity threshold (higher for longer utterances)
                                        });
                                        
                                        if !is_duplicate {
                                            tracing::debug!("Continuous transcription: {}", transcript);
                                            
                                            // Add to recent transcripts cache
                                            recent_transcripts.push_back(clean_transcript);
                                            if recent_transcripts.len() > 10 {
                                                recent_transcripts.pop_front();
                                            }
                                            
                                            // Fire transcription complete event
                                            let event = PluginEvent::TranscriptionComplete { 
                                                text: transcript.clone(), 
                                                confidence: 0.8, // Slightly lower confidence for continuous processing
                                                speaker_id: None  // Speaker detection could be added later
                                            };
                                            if let Err(e) = plugin_manager.fire_event(event).await {
                                                tracing::warn!("Failed to fire TranscriptionComplete event: {}", e);
                                            }
                                        } else {
                                            tracing::debug!("Skipping duplicate transcript: {}", transcript);
                                        }
                                        
                                        // Show status for substantial transcripts from longer audio chunks
                                        if transcript.len() > 20 && transcript.split_whitespace().count() > 3 { 
                                            // Show status for meaningful transcripts (relaxed for longer chunks)
                                            let clean_transcript = transcript.trim();
                                            if !clean_transcript.is_empty() {
                                                tracing::info!("📝 Continuous capture: {}", 
                                                    if clean_transcript.len() > 100 { 
                                                        format!("{}...", &clean_transcript[..100]) 
                                                    } else { 
                                                        clean_transcript.to_string()
                                                    }
                                                );
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        tracing::debug!("Continuous transcription returned empty result");
                                    }
                                    Err(e) => {
                                        tracing::debug!("Continuous transcription failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
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
    
    async fn start_meeting_recording(&self) -> Result<()> {
        if self.config.recording.enabled && self.config.recording.auto_start {
            tracing::info!("Starting automatic meeting recording");
            
            let recorder = self.meeting_recorder.clone();
            let terminal_ui = self.terminal_ui.clone();
            let cancellation_token = self.cancellation_token.clone();
            
            tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::info!("Meeting recording auto-start task cancelled");
                    }
                    _ = async {
                        // Wait a moment for the application to fully initialize
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        
                        match recorder.start_recording(None).await {
                            Ok(recording_id) => {
                                let _ = terminal_ui.print_status(&format!("🎙️  Meeting recording started: {}", recording_id)).await;
                                tracing::info!("Automatic meeting recording started: {}", recording_id);
                            }
                            Err(e) => {
                                let _ = terminal_ui.print_warning(&format!("⚠️  Failed to start automatic recording: {}", e)).await;
                                tracing::error!("Failed to start automatic recording: {}", e);
                            }
                        }
                    } => {}
                }
            });
        }
        
        Ok(())
    }
    
    async fn start_recording_event_handler(&self) -> Result<()> {
        let recording_event_rx = self.recording_event_rx.clone();
        let terminal_ui = self.terminal_ui.clone();
        let cancellation_token = self.cancellation_token.clone();
        
        tokio::spawn(async move {
            loop {
                let event = {
                    let mut rx = recording_event_rx.write().await;
                    tokio::select! {
                        _ = cancellation_token.cancelled() => {
                            tracing::info!("Recording event handler cancelled");
                            break;
                        }
                        event = rx.recv() => {
                            match event {
                                Some(event) => event,
                                None => {
                                    tracing::info!("Recording event receiver closed");
                                    break;
                                }
                            }
                        }
                    }
                };
                
                match event {
                    RecordingEvent::Started(info) => {
                        let _ = terminal_ui.print_status(&format!("🎙️  Recording started: {} ({})", 
                            info.id, info.file_path)).await;
                        tracing::info!("Recording started: {} at {}", info.id, info.file_path);
                    }
                    RecordingEvent::Stopped(info) => {
                        let _ = terminal_ui.print_status(&format!("🛑 Recording stopped: {} ({:.1}s, {:.1}MB)", 
                            info.id, info.duration_seconds, info.file_size_mb())).await;
                        tracing::info!("Recording stopped: {} - Duration: {:.1}s, Size: {:.1}MB", 
                            info.id, info.duration_seconds, info.file_size_mb());
                    }
                    RecordingEvent::Paused(info) => {
                        let _ = terminal_ui.print_status(&format!("⏸️  Recording paused: {}", info.id)).await;
                        tracing::info!("Recording paused: {}", info.id);
                    }
                    RecordingEvent::Resumed(info) => {
                        let _ = terminal_ui.print_status(&format!("▶️  Recording resumed: {}", info.id)).await;
                        tracing::info!("Recording resumed: {}", info.id);
                    }
                    RecordingEvent::Error(error) => {
                        let _ = terminal_ui.print_warning(&format!("❌ Recording error: {}", error)).await;
                        tracing::error!("Recording error: {}", error);
                    }
                    RecordingEvent::StatusUpdate(info) => {
                        // Optional: Show periodic status updates
                        if info.duration_seconds > 0 && info.duration_seconds % 300 == 0 { // Every 5 minutes
                            let _ = terminal_ui.print_status(&format!("📊 Recording status: {} ({:.1}s, {:.1}MB)", 
                                info.id, info.duration_seconds, info.file_size_mb())).await;
                        }
                    }
                }
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
            
            // Fire audio captured event
            let event = PluginEvent::AudioCaptured { file_path: audio_file.clone() };
            if let Err(e) = self.plugin_manager.fire_event(event).await {
                tracing::warn!("Failed to fire AudioCaptured event: {}", e);
            }
            
            let transcript = self.transcribe_audio_with_fallback(&audio_file).await
                .context("Failed to transcribe audio")?;
            
            tracing::info!("handle_audio_capture_internal: Transcription completed");
            
            if !transcript.trim().is_empty() {
                tracing::info!("handle_audio_capture_internal: Displaying transcript");
                self.terminal_ui.print_transcript(&transcript).await?;
                
                // Fire transcription complete event to store the utterance
                let event = PluginEvent::TranscriptionComplete { 
                    text: transcript.clone(), 
                    confidence: 0.9, // Default confidence, could be improved with actual confidence from transcription
                    speaker_id: None  // Speaker detection not implemented yet
                };
                if let Err(e) = self.plugin_manager.fire_event(event).await {
                    tracing::warn!("Failed to fire TranscriptionComplete event: {}", e);
                }
                
                tracing::info!("handle_audio_capture_internal: Generating AI response");
                let context = self.build_conversation_context().await;
                let prompt = if !context.is_empty() {
                    format!("{}\n\nContext: {}", transcript, context)
                } else {
                    transcript.to_string()
                };
                let response = self.generate_streaming_ai_response(&prompt, None).await?;
                
                tracing::info!("handle_audio_capture_internal: Streaming AI response");
                let system_status = self.system_status.read().await;
                self.terminal_ui.stream_response(&response, &system_status).await?;
                
                // Fire prompt stream complete event
                let event = PluginEvent::PromptStreamComplete { response: response.clone() };
                if let Err(e) = self.plugin_manager.fire_event(event).await {
                    tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
                }
                
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
            
            let system_status = self.system_status.read().await;
            self.terminal_ui.stream_response(&response, &system_status).await?;
            
            // Fire prompt stream complete event
            let event = PluginEvent::PromptStreamComplete { response: response.clone() };
            if let Err(e) = self.plugin_manager.fire_event(event).await {
                tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
            }
            
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
                
                let transcript = self.transcribe_audio_with_fallback(&audio_file).await
                    .context("Failed to transcribe audio")?;
                
                let analysis = self.clipboard_handler.read().await.analyze_content_type(&content);
                
                if !transcript.trim().is_empty() {
                    self.terminal_ui.print_transcript(&transcript).await?;
                    self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
                    
                    // Fire transcription complete event to store the utterance
                    let event = PluginEvent::TranscriptionComplete { 
                        text: transcript.clone(), 
                        confidence: 0.9, 
                        speaker_id: None  
                    };
                    if let Err(e) = self.plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire TranscriptionComplete event: {}", e);
                    }
                    
                    let _code_id = self.store_code_in_memory(&content, &analysis).await?;
                    
                    let code_context = self.build_code_context().await;
                    let prompt = format!(
                        "Audio context: {}\n\nCode to analyze:\n```{}\n{}\n```\n\n{}",
                        transcript, analysis.language, content, code_context
                    );
                    let response = self.generate_streaming_ai_response(&prompt, Some("You are an expert code analyst. Analyze the provided code in the context of the audio discussion.")).await?;
                    
                    let system_status = self.system_status.read().await;
                    self.terminal_ui.stream_response(&response, &system_status).await?;
                    
                    // Fire prompt stream complete event
                    let event = PluginEvent::PromptStreamComplete { response: response.clone() };
                    if let Err(e) = self.plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
                    }
                    
                    // Update session history
                    self.update_session_history(&transcript, &response, QuestionType::Combined).await?;
                    
                    // Update conversation context
                    self.update_conversation_context(&transcript, &response).await?;
                } else {
                    self.terminal_ui.print_warning("⚠️  No transcript generated - proceeding with code analysis only").await?;
                    
                    // Fallback to code analysis only
                    self.terminal_ui.print_clipboard_preview(&content, &analysis).await?;
                    
                    let _code_id = self.store_code_in_memory(&content, &analysis).await?;
                    
                    let response = self.generate_streaming_ai_response(&format!("Code to analyze:\n```{}\n{}\n```\n\n{}", analysis.language, content, self.build_code_context().await), Some("You are an expert code analyst. Provide detailed analysis of the code.")).await?;
                    
                    let system_status = self.system_status.read().await;
                    self.terminal_ui.stream_response(&response, &system_status).await?;
                    
                    // Fire prompt stream complete event
                    let event = PluginEvent::PromptStreamComplete { response: response.clone() };
                    if let Err(e) = self.plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
                    }
                    
                    // Update session history
                    self.update_session_history(&content, &response, QuestionType::Code).await?;
                }
            }
            (Some(audio_file), None) => {
                // Only audio available, no clipboard content
                self.terminal_ui.print_status("🔗 Processing audio only (no clipboard content)...").await?;
                
                let transcript = self.transcribe_audio_with_fallback(&audio_file).await
                    .context("Failed to transcribe audio")?;
                
                if !transcript.trim().is_empty() {
                    self.terminal_ui.print_transcript(&transcript).await?;
                    
                    // Fire transcription complete event to store the utterance
                    let event = PluginEvent::TranscriptionComplete { 
                        text: transcript.clone(), 
                        confidence: 0.9, 
                        speaker_id: None  
                    };
                    if let Err(e) = self.plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire TranscriptionComplete event: {}", e);
                    }
                    
                    let context = self.build_conversation_context().await;
                    let prompt = if !context.is_empty() {
                        format!("{}\n\nContext: {}", transcript, context)
                    } else {
                        transcript.to_string()
                    };
                    let response = self.generate_streaming_ai_response(&prompt, None).await?;
                    
                    let system_status = self.system_status.read().await;
                    self.terminal_ui.stream_response(&response, &system_status).await?;
                    
                    // Fire prompt stream complete event
                    let event = PluginEvent::PromptStreamComplete { response: response.clone() };
                    if let Err(e) = self.plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
                    }
                    
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
                
                let code_context = self.build_code_context().await;
                let prompt = format!(
                    "Code to analyze:\n```{}\n{}\n```\n\n{}",
                    analysis.language, content, code_context
                );
                let response = self.generate_streaming_ai_response(&prompt, Some("You are an expert code analyst. Provide detailed analysis of the code.")).await?;
                
                let system_status = self.system_status.read().await;
                self.terminal_ui.stream_response(&response, &system_status).await?;
                
                // Fire prompt stream complete event
                let event = PluginEvent::PromptStreamComplete { response: response.clone() };
                if let Err(e) = self.plugin_manager.fire_event(event).await {
                    tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
                }
                
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
            let transcript = self.transcribe_audio_with_fallback(&audio_file).await
                .unwrap_or_else(|_| "Analyze what you see in the screenshot".to_string());
            
            let audio_context = if !transcript.trim().is_empty() {
                self.terminal_ui.print_transcript(&transcript).await?;
                
                // Fire transcription complete event to store the utterance
                let event = PluginEvent::TranscriptionComplete { 
                    text: transcript.clone(), 
                    confidence: 0.9, 
                    speaker_id: None  
                };
                if let Err(e) = self.plugin_manager.fire_event(event).await {
                    tracing::warn!("Failed to fire TranscriptionComplete event: {}", e);
                }
                
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
            
            let system_status = self.system_status.read().await;
            self.terminal_ui.stream_response(&response, &system_status).await?;
            
            // Fire prompt stream complete event
            let event = PluginEvent::PromptStreamComplete { response: response.clone() };
            if let Err(e) = self.plugin_manager.fire_event(event).await {
                tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
            }
            
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
            
            let system_status = self.system_status.read().await;
            self.terminal_ui.stream_response(&response, &system_status).await?;
            
            // Fire prompt stream complete event
            let event = PluginEvent::PromptStreamComplete { response: response.clone() };
            if let Err(e) = self.plugin_manager.fire_event(event).await {
                tracing::warn!("Failed to fire PromptStreamComplete event: {}", e);
            }
            
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
        let system_status = self.system_status.read().await;
        
        self.terminal_ui.print_session_history(&history, &summary, &code_memory, &system_status).await?;
        
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
        
        history.insert(0, entry.clone());
        
        // Keep only last 5 entries
        if history.len() > 5 {
            history.truncate(5);
        }
        
        // Fire plugin event for session history update
        let event = PluginEvent::SessionHistoryUpdated { entry };
        if let Err(e) = self.plugin_manager.fire_event(event).await {
            tracing::warn!("Failed to fire SessionHistoryUpdated event: {}", e);
        }
        
        Ok(())
    }
    
    async fn update_conversation_context(&self, question: &str, response: &str) -> Result<()> {
        let updated_context = {
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
            
            context.clone()
        }; // Drop the write lock here
        
        // Fire plugin event for conversation context update
        let event = PluginEvent::ConversationContextUpdated { context: updated_context };
        if let Err(e) = self.plugin_manager.fire_event(event).await {
            tracing::warn!("Failed to fire ConversationContextUpdated event: {}", e);
        }
        
        // Update summary after dropping the lock to avoid deadlock
        self.update_conversation_summary().await?;
        
        Ok(())
    }
    
    /// Transcribe audio using local, plugin, and OpenAI fallback systems
    async fn transcribe_audio_with_fallback(&self, audio_file: &PathBuf) -> Result<String> {
        // Try local transcription first
        match self.system_info.transcribe_audio(audio_file).await {
            Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                tracing::info!("Local transcription successful");
                // Clean up the audio file after successful transcription
                let _ = fs::remove_file(audio_file);
                return Ok(transcript);
            }
            Ok(Some(_)) | Ok(None) | Err(_) => {
                tracing::info!("Local transcription failed, trying plugin system");
            }
        }
        
        // Try plugin system for transcription
        match self.plugin_manager.transcribe_audio(audio_file).await {
            Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                tracing::info!("Plugin transcription successful");
                // Clean up the audio file after successful transcription
                let _ = fs::remove_file(audio_file);
                return Ok(transcript);
            }
            Ok(Some(_)) | Ok(None) => {
                tracing::info!("Plugin transcription returned empty or None");
            }
            Err(e) => {
                tracing::warn!("Plugin transcription error: {}", e);
            }
        }
        
        // Fallback to OpenAI if enabled
        if self.config.llm_provider.fallback_to_openai {
            tracing::info!("Plugin transcription failed, using OpenAI API fallback");
            self.terminal_ui.print_status("🔄 Transcribing with OpenAI...").await?;
            match self.openai_client.transcribe_audio(audio_file).await {
                Ok(transcript) => {
                    // Clean up the audio file after successful transcription
                    let _ = fs::remove_file(audio_file);
                    Ok(transcript)
                }
                Err(e) => {
                    // Clean up the audio file even if transcription failed
                    let _ = fs::remove_file(audio_file);
                    Err(anyhow::anyhow!("All transcription methods failed: {}", e))
                }
            }
        } else {
            // Clean up the audio file since no more transcription methods are available
            let _ = fs::remove_file(audio_file);
            Err(anyhow::anyhow!("No transcription service available"))
        }
    }
    
    /// Generate AI response using the active LLM provider (plugin system or OpenAI fallback)
    async fn generate_ai_response(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        // Create provider-specific options based on active provider
        let options = match &self.config.llm_provider.active_provider {
            LLMProvider::OpenAI => LLMOptions {
                max_tokens: Some(self.config.openai.max_tokens),
                temperature: Some(self.config.openai.temperature),
                model: Some(self.config.openai.model.clone()),
                system_prompt: system_prompt.map(|s| s.to_string()),
                streaming: false,
            },
            LLMProvider::Ollama => {
                // Get Ollama-specific config or use defaults
                let ollama_config = match self.config.llm_provider.provider_configs.get("ollama") {
                    Some(config) => {
                        match serde_json::from_value::<crate::plugins::ollama_provider::OllamaConfig>(config.clone()) {
                            Ok(parsed_config) => parsed_config,
                            Err(e) => {
                                tracing::warn!("Failed to parse Ollama config: {}, using defaults", e);
                                crate::plugins::ollama_provider::OllamaConfig::default()
                            }
                        }
                    }
                    None => {
                        tracing::warn!("No Ollama config found in provider_configs, using defaults");
                        crate::plugins::ollama_provider::OllamaConfig::default()
                    }
                };
                
                LLMOptions {
                    max_tokens: Some(self.config.openai.max_tokens), // Use OpenAI config for general settings
                    temperature: Some(self.config.openai.temperature),
                    model: Some(ollama_config.default_model), // Use Ollama model name
                    system_prompt: system_prompt.map(|s| s.to_string()),
                    streaming: false,
                }
            },
            LLMProvider::Custom(_name) => {
                // For custom providers, use neutral defaults without specific model
                LLMOptions {
                    max_tokens: Some(self.config.openai.max_tokens),
                    temperature: Some(self.config.openai.temperature),
                    model: None, // Let the custom provider choose the model
                    system_prompt: system_prompt.map(|s| s.to_string()),
                    streaming: false,
                }
            },
        };

        // Try plugin system first
        match self.plugin_manager.generate_completion(prompt, &options).await {
            Ok(Some(response)) => {
                return Ok(response);
            }
            Ok(None) => {
                tracing::warn!("No active LLM provider found in plugin system");
            }
            Err(e) => {
                tracing::error!("Plugin system LLM generation failed: {}", e);
            }
        }

        // Determine if we should fallback to OpenAI
        let should_fallback = self.config.llm_provider.fallback_to_openai 
            && !self.config.openai.api_key.is_empty()
            && self.config.openai.api_key != "your_openai_api_key_here";

        if should_fallback {
            tracing::info!("Using OpenAI fallback for LLM generation");
            return self.openai_client.generate_meeting_support(prompt, "").await;
        }

        // If no fallback, provide helpful error message
        match &self.config.llm_provider.active_provider {
            LLMProvider::Ollama => {
                Err(anyhow::anyhow!(
                    "Ollama LLM provider failed and OpenAI fallback is disabled.\n\
                    Please ensure:\n\
                    1. Ollama is running: ollama serve\n\
                    2. You have a model installed: ollama pull llama2:7b\n\
                    3. Or enable OpenAI fallback by setting LLM_FALLBACK_TO_OPENAI=true with a valid API key"
                ))
            }
            LLMProvider::Custom(name) => {
                Err(anyhow::anyhow!(
                    "Custom LLM provider '{}' failed and OpenAI fallback is disabled.\n\
                    Please check your custom provider configuration or enable OpenAI fallback.", 
                    name
                ))
            }
            LLMProvider::OpenAI => {
                Err(anyhow::anyhow!("OpenAI LLM provider failed. Please check your API key."))
            }
        }
    }

    /// Generate streaming AI response using the active LLM provider
    async fn generate_streaming_ai_response(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        // Create provider-specific options based on active provider
        let options = match &self.config.llm_provider.active_provider {
            LLMProvider::OpenAI => LLMOptions {
                max_tokens: Some(self.config.openai.max_tokens),
                temperature: Some(self.config.openai.temperature),
                model: Some(self.config.openai.model.clone()),
                system_prompt: system_prompt.map(|s| s.to_string()),
                streaming: true,
            },
            LLMProvider::Ollama => {
                // Get Ollama-specific config or use defaults
                let ollama_config = match self.config.llm_provider.provider_configs.get("ollama") {
                    Some(config) => {
                        match serde_json::from_value::<crate::plugins::ollama_provider::OllamaConfig>(config.clone()) {
                            Ok(parsed_config) => parsed_config,
                            Err(e) => {
                                tracing::warn!("Failed to parse Ollama config for streaming: {}, using defaults", e);
                                crate::plugins::ollama_provider::OllamaConfig::default()
                            }
                        }
                    }
                    None => {
                        tracing::warn!("No Ollama config found in provider_configs for streaming, using defaults");
                        crate::plugins::ollama_provider::OllamaConfig::default()
                    }
                };
                
                LLMOptions {
                    max_tokens: Some(self.config.openai.max_tokens), // Use OpenAI config for general settings
                    temperature: Some(self.config.openai.temperature),
                    model: Some(ollama_config.default_model), // Use Ollama model name
                    system_prompt: system_prompt.map(|s| s.to_string()),
                    streaming: true,
                }
            },
            LLMProvider::Custom(_name) => {
                // For custom providers, use neutral defaults without specific model
                LLMOptions {
                    max_tokens: Some(self.config.openai.max_tokens),
                    temperature: Some(self.config.openai.temperature),
                    model: None, // Let the custom provider choose the model
                    system_prompt: system_prompt.map(|s| s.to_string()),
                    streaming: true,
                }
            },
        };

        // Try plugin system first
        match self.plugin_manager.generate_streaming_completion(prompt, &options).await {
            Ok(Some(mut stream)) => {
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
            Ok(None) => {
                tracing::warn!("No active LLM provider found for streaming");
            }
            Err(e) => {
                tracing::error!("Plugin system streaming LLM generation failed: {}", e);
            }
        }

        // Determine if we should fallback to OpenAI
        let should_fallback = self.config.llm_provider.fallback_to_openai 
            && !self.config.openai.api_key.is_empty()
            && self.config.openai.api_key != "your_openai_api_key_here";

        if should_fallback {
            tracing::info!("Using OpenAI fallback for streaming LLM generation");
            return self.openai_client.generate_meeting_support(prompt, "").await;
        }

        // If no fallback, provide helpful error message
        match &self.config.llm_provider.active_provider {
            LLMProvider::Ollama => {
                Err(anyhow::anyhow!(
                    "Ollama LLM provider failed and OpenAI fallback is disabled.\n\
                    Please ensure:\n\
                    1. Ollama is running: ollama serve\n\
                    2. You have a model installed: ollama pull llama2:7b\n\
                    3. Or enable OpenAI fallback by setting LLM_FALLBACK_TO_OPENAI=true with a valid API key"
                ))
            }
            LLMProvider::Custom(name) => {
                Err(anyhow::anyhow!(
                    "Custom LLM provider '{}' failed and OpenAI fallback is disabled.\n\
                    Please check your custom provider configuration or enable OpenAI fallback.", 
                    name
                ))
            }
            LLMProvider::OpenAI => {
                Err(anyhow::anyhow!("OpenAI LLM provider failed. Please check your API key."))
            }
        }
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

    /// Helper function to prompt user for yes/no input
    async fn ask_yes_no(&self, question: &str) -> Result<bool> {
        loop {
            print!("{} (y/n): ", question);
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => {
                    println!("{}", "Please enter 'y' or 'n'".yellow());
                }
            }
        }
    }



    /// Generate transcript using the advanced diarization plugin
    async fn generate_transcript(&self) -> Result<()> {
        // Try to get audio data from multiple sources
        let audio_file = {
            let mut audio_capture = self.audio_capture.write().await;
            
            // First try to extract recent buffer
            match audio_capture.extract_recent_buffer(60).await {
                Ok(Some(file)) => {
                    println!("{}", "📡 Using recent audio buffer for transcript...".blue());
                    Some(file)
                }
                Ok(None) => {
                    // Try with a shorter duration
                    println!("{}", "🔄 Trying shorter audio duration...".yellow());
                    match audio_capture.extract_recent_buffer(30).await {
                        Ok(Some(file)) => Some(file),
                        Ok(None) => {
                            println!("{}", "⚠️  No audio data available for transcript generation".yellow());
                            return Ok(());
                        }
                        Err(e) => {
                            println!("{}", format!("⚠️  Error extracting audio: {}", e).yellow());
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    println!("{}", format!("⚠️  Error extracting audio from buffer: {}", e).yellow());
                    
                    // Try to use the last recorded meeting file if available
                    if self.meeting_recorder.is_recording().await {
                        println!("{}", "🎙️  Attempting to use meeting recording for transcript...".blue());
                        // Note: This would require access to the current recording file
                        // For now, we'll just skip and show the error
                        return Ok(());
                    } else {
                        return Ok(());
                    }
                }
            }
        };

        if let Some(audio_file) = audio_file {
            // Check if the audio file exists and has content
            match std::fs::metadata(&audio_file) {
                Ok(metadata) => {
                    if metadata.len() == 0 {
                        println!("{}", "⚠️  Audio file is empty, cannot generate transcript".yellow());
                        return Ok(());
                    }
                    println!("{}", format!("📂 Audio file: {} ({:.1}KB)", 
                        audio_file.file_name().unwrap_or_default().to_string_lossy(),
                        metadata.len() as f64 / 1024.0).blue());
                }
                Err(e) => {
                    println!("{}", format!("⚠️  Cannot access audio file: {}", e).yellow());
                    return Ok(());
                }
            }
            
            println!("{}", "📝 Generating transcript from meeting audio...".cyan());
            println!("{}", "   This may take a moment depending on audio length...".bright_black());
            
            // Fire the audio captured event to trigger the diarization plugin
            let event = PluginEvent::AudioCaptured { 
                file_path: audio_file.clone() 
            };
            
            match self.plugin_manager.fire_event(event).await {
                Ok(results) => {
                    let mut transcript_found = false;
                    
                    // Check if any plugin returned a transcript
                    for result in results {
                        match result {
                            PluginHookResult::Replace(data) => {
                                if let Some(segments) = data.get("segments") {
                                    if let Some(segments_array) = segments.as_array() {
                                        if !segments_array.is_empty() {
                                            transcript_found = true;
                                            
                                            // Display the transcript in a formatted way
                                            println!();
                                            println!("{}", "📄 Meeting Transcript:".green().bold());
                                            println!("{}", "=".repeat(50).bright_black());
                                            
                                            for segment in segments_array {
                                                if let (Some(speaker), Some(text)) = (
                                                    segment.get("speaker_id").and_then(|s| s.as_str()),
                                                    segment.get("text").and_then(|s| s.as_str())
                                                ) {
                                                    if !text.trim().is_empty() {
                                                        println!("{}: {}", speaker.cyan().bold(), text.white());
                                                    }
                                                }
                                            }
                                            
                                            println!("{}", "=".repeat(50).bright_black());
                                            
                                            // Show summary stats
                                            if let Some(total_speakers) = data.get("total_speakers").and_then(|v| v.as_u64()) {
                                                println!("{} {}", "👥 Total speakers:".blue(), total_speakers);
                                            }
                                            if let Some(total_segments) = data.get("total_segments").and_then(|v| v.as_u64()) {
                                                println!("{} {}", "💬 Total segments:".blue(), total_segments);
                                            }
                                            if let Some(total_duration) = data.get("total_duration").and_then(|v| v.as_f64()) {
                                                println!("{} {:.1}s", "⏱️  Total duration:".blue(), total_duration);
                                            }
                                            
                                            println!();
                                            println!("{}", "✅ Transcript generated successfully!".green());
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }
                    
                    // If no transcript was generated, show a helpful message
                    if !transcript_found {
                        println!("{}", "⚠️  No transcript data was generated by the diarization plugin".yellow());
                        println!("{}", "   This could be due to:".bright_black());
                        println!("{}", "   • No speech detected in the audio".bright_black());
                        println!("{}", "   • Audio quality too poor for transcription".bright_black());
                        println!("{}", "   • Plugin configuration issues".bright_black());
                        println!("{}", "   • Missing dependencies (Python, PyAnnote, etc.)".bright_black());
                    }
                }
                Err(e) => {
                    println!("{}", format!("⚠️  Error generating transcript: {}", e).yellow());
                    println!("{}", "   Check that the advanced diarization plugin is properly configured".bright_black());
                }
            }
        }
        
        Ok(())
    }

    /// Check if any transcript generation capability is available
    async fn is_transcript_generation_available(&self) -> bool {
        let plugins = self.plugin_manager.list_plugins().await;
        
        // Check for any transcription-capable plugins
        let has_stt = plugins.contains_key("stt_post_processor");
        let has_advanced_diarization = plugins.contains_key("advanced_diarization");
        
        // Debug: Show what transcript capabilities are available
        if has_stt {
            println!("🔍 STT Post Processor plugin is available");
        }
        if has_advanced_diarization {
            println!("🔍 Advanced Diarization plugin is available");
        }
        
        has_stt || has_advanced_diarization
    }
}

/// Calculate similarity between two text strings using Jaccard similarity
fn calculate_text_similarity(text1: &str, text2: &str) -> f32 {
    let words1: std::collections::HashSet<&str> = text1.split_whitespace().collect();
    let words2: std::collections::HashSet<&str> = text2.split_whitespace().collect();
    
    if words1.is_empty() && words2.is_empty() {
        return 1.0;
    }
    
    if words1.is_empty() || words2.is_empty() {
        return 0.0;
    }
    
    let intersection: std::collections::HashSet<_> = words1.intersection(&words2).collect();
    let union: std::collections::HashSet<_> = words1.union(&words2).collect();
    
    intersection.len() as f32 / union.len() as f32
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
        
        Some(Commands::Record { command }) => {
            // Handle recording commands
            handle_record_command(command).await?;
        }
        
        Some(Commands::Transcript { command }) => {
            // Handle transcript commands
            handle_transcript_command(command).await?;
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
    let mut plugin_manager = PluginManager::new(config.clone(), temp_dir)?;
    let registry = PluginRegistry::new()?;
    
    // Register all built-in plugins (same as main application)
    MeetingAssistant::register_builtin_plugins(&mut plugin_manager, &config).await?;
    
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

async fn handle_record_command(command: RecordCommand) -> Result<()> {
    // Initialize components for recording commands
    let config = Config::load().await?;
    let temp_dir = dirs::home_dir()
        .context("Failed to get home directory")?
        .join(".meeting-assistant")
        .join("temp");
    std::fs::create_dir_all(&temp_dir)?;
    
    let (recorder, mut _event_rx) = MeetingRecorder::new(&config)?;
    
    match command {
        RecordCommand::Start { title } => {
            println!("🎙️  Starting meeting recording...");
            if let Some(title) = &title {
                println!("📝 Title: {}", title);
            }
            
            match recorder.start_recording(title).await {
                Ok(recording_id) => {
                    println!("✅ Recording started successfully!");
                    println!("📋 Recording ID: {}", recording_id);
                    
                    // Show recording info
                    if let Some(info) = recorder.get_current_recording().await {
                        println!("📁 Output file: {}", info.file_path);
                        println!("🔊 Format: {} ({}Hz)", info.format, info.quality.sample_rate());
                        println!("📊 Press Ctrl+C to stop recording");
                    }
                    
                    // Wait for stop signal
                    tokio::signal::ctrl_c().await?;
                    println!("\n🛑 Stopping recording...");
                    
                    match recorder.stop_recording().await {
                        Ok(Some(final_info)) => {
                            println!("✅ Recording stopped successfully!");
                            println!("⏱️  Duration: {:.1} seconds", final_info.duration_seconds);
                            println!("📊 File size: {:.1} MB", final_info.file_size_mb());
                            println!("📁 Saved to: {}", final_info.file_path);
                        }
                        Ok(None) => {
                            println!("ℹ️  No active recording found");
                        }
                        Err(e) => {
                            eprintln!("❌ Error stopping recording: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to start recording: {}", e);
                }
            }
        }
        
        RecordCommand::Stop => {
            println!("🛑 Stopping current recording...");
            match recorder.stop_recording().await {
                Ok(Some(info)) => {
                    println!("✅ Recording stopped successfully!");
                    println!("📋 Recording ID: {}", info.id);
                    println!("⏱️  Duration: {:.1} seconds", info.duration_seconds);
                    println!("📊 File size: {:.1} MB", info.file_size_mb());
                    println!("📁 Saved to: {}", info.file_path);
                }
                Ok(None) => {
                    println!("ℹ️  No active recording found");
                }
                Err(e) => {
                    eprintln!("❌ Error stopping recording: {}", e);
                }
            }
        }
        
        RecordCommand::Pause => {
            println!("⏸️  Pausing current recording...");
            match recorder.pause_recording().await {
                Ok(()) => {
                    println!("✅ Recording paused successfully!");
                }
                Err(e) => {
                    eprintln!("❌ Error pausing recording: {}", e);
                }
            }
        }
        
        RecordCommand::Resume => {
            println!("▶️  Resuming recording...");
            match recorder.resume_recording().await {
                Ok(()) => {
                    println!("✅ Recording resumed successfully!");
                }
                Err(e) => {
                    eprintln!("❌ Error resuming recording: {}", e);
                }
            }
        }
        
        RecordCommand::Status => {
            println!("📊 Recording Status");
            println!("==================");
            
            if let Some(info) = recorder.get_current_recording().await {
                println!("📋 Recording ID: {}", info.id);
                println!("📁 Output file: {}", info.file_path);
                println!("🔊 Format: {} ({}Hz)", info.format, info.quality.sample_rate());
                println!("📊 Status: {}", info.status);
                println!("⏱️  Duration: {:.1} seconds", info.duration_seconds);
                println!("📊 File size: {:.1} MB", info.file_size_mb());
                println!("📅 Started: {}", info.started_at.format("%Y-%m-%d %H:%M:%S"));
                
                if let Some(ended_at) = info.ended_at {
                    println!("📅 Ended: {}", ended_at.format("%Y-%m-%d %H:%M:%S"));
                }
                
                if !info.metadata.is_empty() {
                    println!("📝 Metadata:");
                    for (key, value) in &info.metadata {
                        println!("  • {}: {}", key, value);
                    }
                }
            } else {
                println!("ℹ️  No active recording");
            }
        }
        
        RecordCommand::Test => {
            println!("🔧 Testing FFmpeg setup and audio device availability...");
            
            match recorder.test_ffmpeg_setup().await {
                Ok(report) => {
                    println!("{}", report);
                }
                Err(e) => {
                    eprintln!("❌ FFmpeg test failed: {}", e);
                    eprintln!("💡 This might explain why your recordings are 0 bytes.");
                    eprintln!("💡 Try checking:");
                    eprintln!("   • Audio device permissions");
                    eprintln!("   • Audio device index configuration");
                    eprintln!("   • FFmpeg installation and version");
                }
            }
        }
        
        RecordCommand::List => {
            println!("📋 Meeting Recordings");
            println!("====================");
            
            match recorder.list_recordings().await {
                Ok(recordings) => {
                    if recordings.is_empty() {
                        println!("No recordings found.");
                    } else {
                        for info in recordings {
                            println!("📋 {} ({})", info.id, info.status);
                            println!("   📁 {}", info.file_path);
                            println!("   ⏱️  {:.1}s • 📊 {:.1}MB • 📅 {}", 
                                info.duration_seconds, 
                                info.file_size_mb(),
                                info.started_at.format("%Y-%m-%d %H:%M:%S"));
                            
                            if let Some(title) = info.metadata.get("title") {
                                println!("   📝 {}", title);
                            }
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error listing recordings: {}", e);
                }
            }
        }
        
        RecordCommand::Delete { id } => {
            println!("🗑️  Deleting recording: {}", id);
            match recorder.delete_recording(&id).await {
                Ok(()) => {
                    println!("✅ Recording deleted successfully!");
                }
                Err(e) => {
                    eprintln!("❌ Error deleting recording: {}", e);
                }
            }
        }
        
        RecordCommand::Info { id } => {
            println!("📊 Recording Information");
            println!("=======================");
            
            match recorder.list_recordings().await {
                Ok(recordings) => {
                    if let Some(info) = recordings.iter().find(|r| r.id == id) {
                        println!("📋 Recording ID: {}", info.id);
                        println!("📁 File path: {}", info.file_path);
                        println!("🔊 Format: {} ({}Hz)", info.format, info.quality.sample_rate());
                        println!("📊 Status: {}", info.status);
                        println!("⏱️  Duration: {:.1} seconds", info.duration_seconds);
                        println!("📊 File size: {:.1} MB", info.file_size_mb());
                        println!("📅 Started: {}", info.started_at.format("%Y-%m-%d %H:%M:%S"));
                        
                        if let Some(ended_at) = info.ended_at {
                            println!("📅 Ended: {}", ended_at.format("%Y-%m-%d %H:%M:%S"));
                        }
                        
                        println!("🎤 Audio config:");
                        println!("  • Sample rate: {} Hz", info.sample_rate);
                        println!("  • Channels: {}", info.channels);
                        println!("  • Has transcript: {}", if info.has_transcript { "Yes" } else { "No" });
                        println!("  • Has diarization: {}", if info.has_diarization { "Yes" } else { "No" });
                        
                        if !info.metadata.is_empty() {
                            println!("📝 Metadata:");
                            for (key, value) in &info.metadata {
                                println!("  • {}: {}", key, value);
                            }
                        }
                    } else {
                        println!("❌ Recording not found: {}", id);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error getting recording info: {}", e);
                }
            }
        }
    }
    
    Ok(())
}

async fn handle_transcript_command(command: TranscriptCommand) -> Result<()> {
    // Initialize plugin manager for transcript commands
    let config = Config::load().await?;
    let temp_dir = dirs::home_dir()
        .context("Failed to get home directory")?
        .join(".meeting-assistant")
        .join("temp");
    std::fs::create_dir_all(&temp_dir)?;
    let mut plugin_manager = PluginManager::new(config.clone(), temp_dir.clone())?;
    
    // Initialize the required services for transcription
    let openai_client = Arc::new(OpenAIClient::new(&config).await?);
    let system_info = Arc::new(SystemInfo::new().await?);
    
    // Register the STT post-processor plugin
    let stt_plugin = STTPostProcessorPlugin::new();
    plugin_manager.register_plugin("stt_post_processor".to_string(), Box::new(stt_plugin)).await?;
    
    let plugin_manager = Arc::new(plugin_manager);
    
    // Set up transcription services for the STT plugin
    {
        let mut plugins = plugin_manager.get_plugins().write().await;
        if let Some(stt_plugin) = plugins.get_mut("stt_post_processor") {
            if let Some(stt_plugin) = stt_plugin.as_any_mut().downcast_mut::<STTPostProcessorPlugin>() {
                stt_plugin.set_transcription_services(
                    system_info.clone(),
                    openai_client.clone(),
                    plugin_manager.clone()
                );
            }
        }
    }
    
    // Initialize plugins
    plugin_manager.initialize_plugins().await?;
    
    match command {
        TranscriptCommand::List => {
            println!("📋 Available Audio Files for Transcription");
            println!("==========================================");
            
            // Fire custom event to get audio files
            let event = PluginEvent::Custom {
                event_type: "list_audio_files".to_string(),
                data: serde_json::Value::Null,
            };
            
            let results = plugin_manager.fire_event(event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                if let Ok(files) = serde_json::from_value::<Vec<PathBuf>>(result) {
                    if files.is_empty() {
                        println!("No audio files found.");
                    } else {
                        for (i, file) in files.iter().enumerate() {
                            println!("  {}. {:?}", i + 1, file);
                        }
                    }
                } else {
                    println!("Error parsing audio files list.");
                }
            } else {
                println!("No audio files found.");
            }
        }
        
        TranscriptCommand::Generate { file } => {
            println!("🎙️  Generating transcript for: {:?}", file);
            
            if !file.exists() {
                eprintln!("❌ Audio file not found: {:?}", file);
                return Ok(());
            }
            
            // Fire custom event to process the file
            let event = PluginEvent::Custom {
                event_type: "process_file".to_string(),
                data: json!({
                    "file_path": file.to_string_lossy()
                }),
            };
            
            let results = plugin_manager.fire_event(event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                if let Some(error) = result.get("error") {
                    eprintln!("❌ Error processing file: {}", error);
                } else {
                    println!("✅ Successfully generated transcript!");
                    println!("📋 Transcript ID: {}", result.get("transcript_id").unwrap_or(&serde_json::Value::Null));
                    println!("👥 Speakers: {}", result.get("speakers").unwrap_or(&serde_json::Value::Null));
                    println!("📝 Segments: {}", result.get("segments").unwrap_or(&serde_json::Value::Null));
                    println!("🎯 Confidence: {:.2}", result.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0));
                    
                    if let Some(full_text) = result.get("full_text").and_then(|t| t.as_str()) {
                        println!("\n📄 Full Transcript:");
                        println!("{}", "-".repeat(50));
                        println!("{}", full_text);
                    }
                }
            } else {
                println!("❌ Failed to process audio file");
            }
        }

        TranscriptCommand::Diarize { file, model: _model, max_speakers: _max_speakers, min_speakers: _min_speakers, format } => {
            println!("🎯 Starting advanced speaker diarization for: {:?}", file);
            println!("🔧 Model: {}, Format: {}", _model, format);
            
            if !file.exists() {
                eprintln!("❌ Audio file not found: {:?}", file);
                return Ok(());
            }
            
            // Create a new plugin manager specifically for diarization
            let mut diarization_plugin_manager = PluginManager::new(config.clone(), temp_dir.clone())?;
            
            // Register the Advanced Diarization plugin
            let advanced_diarization_plugin = AdvancedDiarizationPlugin::new();
            diarization_plugin_manager.register_plugin("advanced_diarization".to_string(), Box::new(advanced_diarization_plugin)).await?;
            
            // Initialize the diarization plugin
            diarization_plugin_manager.initialize_plugins().await?;
            
            // Fire AudioCaptured event to trigger diarization
            let event = PluginEvent::AudioCaptured {
                file_path: file.clone(),
            };
            
            let results = diarization_plugin_manager.fire_event(event).await?;
            
            // Process results
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                println!("🎯 Diarization Results:");
                println!("=====================");
                
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    // Format as human-readable text
                    if let Some(segments) = result.get("segments") {
                        if let Some(segments_array) = segments.as_array() {
                            for (i, segment) in segments_array.iter().enumerate() {
                                if let (Some(start), Some(end), Some(text), Some(speaker)) = (
                                    segment.get("start"),
                                    segment.get("end"),
                                    segment.get("text"),
                                    segment.get("speaker")
                                ) {
                                    println!("{}. [{:.1}s - {:.1}s] {}: {}", 
                                        i + 1,
                                        start.as_f64().unwrap_or(0.0),
                                        end.as_f64().unwrap_or(0.0),
                                        speaker.as_str().unwrap_or("Unknown"),
                                        text.as_str().unwrap_or("")
                                    );
                                }
                            }
                        }
                    }
                    
                    if let Some(speakers) = result.get("speakers") {
                        if let Some(speakers_array) = speakers.as_array() {
                            println!("\n📊 Speakers detected: {}", speakers_array.len());
                            for (i, speaker) in speakers_array.iter().enumerate() {
                                if let Some(speaker_str) = speaker.as_str() {
                                    println!("  {}. {}", i + 1, speaker_str);
                                }
                            }
                        }
                    }
                }
            } else {
                println!("No diarization results available.");
            }
        }

        TranscriptCommand::DiarizeLatest { model: _model, max_speakers: _max_speakers, min_speakers: _min_speakers, format } => {
            println!("🎯 Finding latest audio file for diarization...");
            
            // Find the latest audio file
            let (recorder, _) = MeetingRecorder::new(&config)?;
            let recordings = recorder.list_recordings().await?;
            
            if recordings.is_empty() {
                eprintln!("❌ No audio files found");
                eprintln!("💡 Record some audio first with: meeting-assistant record start");
                return Ok(());
            }
            
            // Get the latest recording (recordings are sorted by start time, most recent first)
            let latest_recording = &recordings[0];
            let latest_file = PathBuf::from(&latest_recording.file_path);
            
            println!("📁 Latest audio file: {:?}", latest_file);
            println!("📅 Recorded: {}", latest_recording.started_at.format("%Y-%m-%d %H:%M:%S"));
            println!("⏱️  Duration: {:.1}s", latest_recording.duration_seconds);
            println!("🔧 Model: {}, Format: {}", _model, format);
            
            if !latest_file.exists() {
                eprintln!("❌ Audio file not found: {:?}", latest_file);
                return Ok(());
            }
            
            // Create a new plugin manager specifically for diarization
            let mut diarization_plugin_manager = PluginManager::new(config.clone(), temp_dir.clone())?;
            
            // Register the Advanced Diarization plugin
            let advanced_diarization_plugin = AdvancedDiarizationPlugin::new();
            diarization_plugin_manager.register_plugin("advanced_diarization".to_string(), Box::new(advanced_diarization_plugin)).await?;
            
            // Initialize the diarization plugin
            diarization_plugin_manager.initialize_plugins().await?;
            
            println!("🎤 Processing with Whisper + PyAnnote diarization...");
            
            // Fire AudioCaptured event (which the WhisperPyAnnote plugin listens for)
            let event = PluginEvent::AudioCaptured { file_path: latest_file.clone() };
            
            let results = diarization_plugin_manager.fire_event(event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                println!("✅ Diarization completed successfully!");
                
                // Display results based on format (same logic as regular Diarize command)
                match format.as_str() {
                    "json" => {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    }
                    "text" => {
                        if let Some(segments) = result.get("segments").and_then(|s| s.as_array()) {
                            println!("\n📄 Transcription:");
                            println!("{}", "=".repeat(60));
                            for segment in segments {
                                if let (Some(speaker_id), Some(text)) = (
                                    segment.get("speaker_id").and_then(|s| s.as_str()),
                                    segment.get("text").and_then(|t| t.as_str())
                                ) {
                                    println!("{}: {}", speaker_id, text);
                                }
                            }
                        }
                    }
                    "detailed" | _ => {
                        // Detailed format (default)
                        println!("\n📊 Diarization Results:");
                        println!("{}", "=".repeat(60));
                        
                        if let Some(total_speakers) = result.get("total_speakers") {
                            println!("👥 Total speakers detected: {}", total_speakers);
                        }
                        if let Some(total_segments) = result.get("total_segments") {
                            println!("📝 Total segments: {}", total_segments);
                        }
                        if let Some(total_duration) = result.get("total_duration") {
                            println!("⏱️  Total duration: {:.2} seconds", total_duration);
                        }
                        if let Some(avg_confidence) = result.get("average_confidence") {
                            println!("🎯 Average confidence: {:.1}%", avg_confidence.as_f64().unwrap_or(0.0) * 100.0);
                        }
                        
                        if let Some(speakers) = result.get("speakers").and_then(|s| s.as_array()) {
                            println!("\n👥 Speaker Profiles:");
                            for (i, speaker) in speakers.iter().enumerate() {
                                if let (Some(id), Some(duration)) = (
                                    speaker.get("id").and_then(|s| s.as_str()),
                                    speaker.get("total_duration").and_then(|d| d.as_f64())
                                ) {
                                    println!("  {}. {} - {:.1}s", i + 1, id, duration);
                                }
                            }
                        }
                        
                        if let Some(segments) = result.get("segments").and_then(|s| s.as_array()) {
                            println!("\n📄 Transcription:");
                            println!("{}", "=".repeat(60));
                            for segment in segments {
                                if let (Some(start), Some(end), Some(speaker_id), Some(text)) = (
                                    segment.get("start_time").and_then(|s| s.as_f64()),
                                    segment.get("end_time").and_then(|e| e.as_f64()),
                                    segment.get("speaker_id").and_then(|s| s.as_str()),
                                    segment.get("text").and_then(|t| t.as_str())
                                ) {
                                    println!("[{:5.1}s - {:5.1}s] {}: {}", start, end, speaker_id, text);
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("❌ No response from diarization processing");
                eprintln!("💡 Make sure you have:");
                eprintln!("   • Python dependencies installed: pip install openai-whisper torch");
                eprintln!("   • For full speaker separation: pip install pyannote.audio");
                eprintln!("   • HuggingFace token in .env: HUGGINGFACE_HUB_TOKEN=your_token");
            }
        }
        
        TranscriptCommand::Reprocess => {
            println!("🔄 Reprocessing all audio files...");
            
            // Fire custom event to reprocess all files
            let event = PluginEvent::Custom {
                event_type: "reprocess_all".to_string(),
                data: serde_json::Value::Null,
            };
            
            let results = plugin_manager.fire_event(event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                if let Some(error) = result.get("error") {
                    eprintln!("❌ Error reprocessing files: {}", error);
                } else if let Ok(transcripts) = serde_json::from_value::<Vec<serde_json::Value>>(result) {
                    println!("✅ Successfully reprocessed {} files!", transcripts.len());
                    for transcript in transcripts {
                        if let Some(id) = transcript.get("id") {
                            println!("  📋 {}", id);
                        }
                    }
                } else {
                    println!("❌ Error parsing reprocessing results");
                }
            } else {
                println!("❌ Failed to reprocess files");
            }
        }
        
        TranscriptCommand::Show { id } => {
            println!("📄 Showing transcript: {}", id);
            
            // Fire custom event to get specific transcript
            let event = PluginEvent::Custom {
                event_type: "get_transcript".to_string(),
                data: json!({
                    "transcript_id": id
                }),
            };
            
            let results = plugin_manager.fire_event(event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                if result.is_null() {
                    println!("❌ Transcript not found: {}", id);
                } else {
                    println!("✅ Found transcript:");
                    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "Error formatting transcript".to_string()));
                }
            } else {
                println!("❌ Failed to retrieve transcript");
            }
        }
        
        TranscriptCommand::Status => {
            println!("📊 Processing Status");
            println!("===================");
            
            // Get all audio files first
            let list_event = PluginEvent::Custom {
                event_type: "list_audio_files".to_string(),
                data: serde_json::Value::Null,
            };
            
            let results = plugin_manager.fire_event(list_event).await?;
            
            if let Some(result) = results.into_iter().find_map(|r| {
                if let PluginHookResult::Replace(data) = r {
                    Some(data)
                } else {
                    None
                }
            }) {
                if let Ok(files) = serde_json::from_value::<Vec<PathBuf>>(result) {
                    if files.is_empty() {
                        println!("No audio files found.");
                    } else {
                        for file in files {
                            // Get status for each file
                            let status_event = PluginEvent::Custom {
                                event_type: "get_processing_status".to_string(),
                                data: json!({
                                    "file_path": file.to_string_lossy()
                                }),
                            };
                            
                            if let Ok(status_results) = plugin_manager.fire_event(status_event).await {
                                if let Some(status_result) = status_results.into_iter().find_map(|r| {
                                    if let PluginHookResult::Replace(data) = r {
                                        Some(data)
                                    } else {
                                        None
                                    }
                                }) {
                                    if !status_result.is_null() {
                                        let status_str = status_result.get("status")
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("Unknown");
                                        let status_emoji = match status_str {
                                            "Completed" => "✅",
                                            "Processing" => "⏳",
                                            "Failed" => "❌",
                                            "Pending" => "⏰",
                                            _ => "❓",
                                        };
                                        
                                        println!("  {} {:?} - {}", status_emoji, file, status_str);
                                        
                                        if let Some(error) = status_result.get("error").and_then(|e| e.as_str()) {
                                            println!("    Error: {}", error);
                                        }
                                    } else {
                                        println!("  ❓ {:?} - Not processed", file);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    println!("Error parsing audio files list.");
                }
            }
        }
        
        TranscriptCommand::Interactive => {
            println!("🤖 Interactive Transcript Analysis");
            println!("===================================");
            
            // Create and run the interactive plugin directly
            let mut transcript_plugin = crate::plugins::transcript_interactive::TranscriptInteractivePlugin::new();
            let terminal_ui = Arc::new(TerminalUI::new());
            transcript_plugin.set_services(terminal_ui, openai_client);
            
            // Run the interactive session directly
            match transcript_plugin.run_interactive().await {
                Ok(()) => {
                    println!("✅ Interactive session completed successfully!");
                }
                Err(e) => {
                    println!("❌ Interactive session failed: {}", e);
                }
            }
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
/*
 * Meeting Assistant CLI - Continuous Meeting Architecture
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

use anyhow::{Result, Context};
use chrono::Duration as ChronoDuration;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::continuous_audio::AudioPipeline;
use crate::continuous_types::*;


#[derive(Parser)]
#[command(name = "meeting-assistant")]
#[command(about = "Continuous AI-powered meeting assistant with real-time transcription and analysis")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start continuous meeting recording
    Start {
        /// Meeting title
        #[arg(long)]
        title: Option<String>,
        
        /// Disable auto-start recording
        #[arg(long)]
        no_auto_record: bool,
    },
    
    /// Stop continuous meeting recording
    Stop {
        /// Force stop even if processing backlog
        #[arg(long)]
        force: bool,
    },
    
    /// Pause/resume recording
    Pause,
    Resume,
    
    /// Show system status
    Status,
    
    /// Speaker management
    Speakers {
        #[command(subcommand)]
        action: SpeakerAction,
    },
    
    /// Search through recordings
    Search {
        /// Search query
        query: String,
        
        /// Search mode: text, semantic, or auto
        #[arg(long, default_value = "auto")]
        mode: String,
        
        /// Maximum results
        #[arg(long, default_value = "10")]
        limit: usize,
        
        /// Filter by speaker
        #[arg(long)]
        speaker: Option<String>,
        
        /// Filter by time range
        #[arg(long)]
        since: Option<String>,
        
        /// Minimum confidence threshold
        #[arg(long, default_value = "0.7")]
        confidence: f32,
    },
    
    /// Analytics and insights
    Analytics {
        #[command(subcommand)]
        report: AnalyticsReport,
    },
    
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    
    /// Database management
    Database {
        #[command(subcommand)]
        action: DatabaseAction,
    },
}

#[derive(Subcommand)]
enum SpeakerAction {
    /// List all speakers
    List,
    
    /// Identify a speaker
    Identify {
        speaker_id: String,
        name: String,
    },
    
    /// Merge two speaker profiles
    Merge {
        from_id: String,
        to_id: String,
    },
    
    /// Show speaker statistics
    Stats {
        speaker_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum AnalyticsReport {
    /// Meeting length statistics
    MeetingLength {
        #[arg(long)]
        since: Option<String>,
    },
    
    /// Speaker time analysis
    SpeakerTime {
        #[arg(long)]
        meeting_id: Option<String>,
    },
    
    /// Topic analysis
    Topics {
        #[arg(long)]
        since: Option<String>,
        
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    
    /// Overall statistics
    Summary,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    
    /// Set configuration value
    Set {
        key: String,
        value: String,
    },
    
    /// Reset to defaults
    Reset,
    
    /// Validate configuration
    Validate,
}

#[derive(Subcommand)]
pub enum DatabaseAction {
    /// Show database statistics
    Stats,
    
    /// Truncate all database tables (WARNING: Irreversible!)
    Truncate {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    
    /// Backup database
    Backup {
        /// Backup file path (optional)
        #[arg(long)]
        path: Option<String>,
    },
    
    /// Optimize database (VACUUM)
    Optimize,
}

/// Core continuous meeting assistant
pub struct ContinuousMeetingAssistant {
    // Processing pipelines
    audio_pipeline: Arc<AudioPipeline>,
    transcription_pipeline: Arc<TranscriptionPipeline>,
    diarization_pipeline: Arc<DiarizationPipeline>,
    vectorization_pipeline: Arc<VectorizationPipeline>,
    storage_pipeline: Arc<StoragePipeline>,
    
    // State management
    current_meeting: Arc<RwLock<Option<MeetingSession>>>,
    speaker_registry: Arc<RwLock<SpeakerRegistry>>,
    processing_queue: Arc<ProcessingQueue>,
    
    // Configuration and control
    config: ContinuousMeetingConfig,
    cancellation_token: CancellationToken,
    
    // Status tracking
    system_status: Arc<RwLock<SystemStatus>>,
}

impl ContinuousMeetingAssistant {
    pub async fn new() -> Result<Self> {
        let config = ContinuousMeetingConfig::default();
        
        // Create temporary directory
        let temp_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".meeting-assistant")
            .join("temp");
        std::fs::create_dir_all(&temp_dir)?;
        
        // Initialize processing pipelines
        let (audio_pipeline, audio_receiver) = AudioPipeline::new(config.clone(), temp_dir.clone());
        let (transcription_pipeline, transcript_receiver) = TranscriptionPipeline::new(audio_receiver);
        let (diarization_pipeline, diarized_receiver) = DiarizationPipeline::new(transcript_receiver);
        let (vectorization_pipeline, vectorized_receiver) = VectorizationPipeline::new(diarized_receiver);
        let storage_pipeline = StoragePipeline::new(vectorized_receiver);
        
        // Initialize state
        let speaker_registry = Arc::new(RwLock::new(SpeakerRegistry::new()));
        let processing_queue = Arc::new(ProcessingQueue::new(config.max_processing_queue_size));
        let cancellation_token = CancellationToken::new();
        
        // Initialize system status
        let system_status = Arc::new(RwLock::new(SystemStatus {
            recording_status: MeetingStatus::Starting,
            pipeline_health: PipelineHealth {
                audio_capture: HealthStatus::Healthy,
                transcription: HealthStatus::Healthy,
                diarization: HealthStatus::Healthy,
                vectorization: HealthStatus::Healthy,
                storage: HealthStatus::Healthy,
            },
            resource_usage: ResourceUsage {
                cpu_percent: 0.0,
                memory_mb: 0,
                disk_usage_mb: 0,
                network_requests_per_minute: 0,
            },
            queue_status: QueueStatus {
                audio_queue_size: 0,
                transcription_queue_size: 0,
                diarization_queue_size: 0,
                vectorization_queue_size: 0,
                total_backlog: 0,
            },
            error_count: ErrorCounts {
                audio_errors: 0,
                transcription_errors: 0,
                diarization_errors: 0,
                vectorization_errors: 0,
                storage_errors: 0,
                total_errors: 0,
            },
        }));
        
        Ok(Self {
            audio_pipeline: Arc::new(audio_pipeline),
            transcription_pipeline: Arc::new(transcription_pipeline),
            diarization_pipeline: Arc::new(diarization_pipeline),
            vectorization_pipeline: Arc::new(vectorization_pipeline),
            storage_pipeline: Arc::new(storage_pipeline),
            
            current_meeting: Arc::new(RwLock::new(None)),
            speaker_registry,
            processing_queue,
            
            config,
            cancellation_token,
            
            system_status,
        })
    }
    
    pub async fn start_meeting(&self, title: Option<String>) -> Result<Uuid> {
        println!("🎯 Starting continuous meeting recording...");
        
        // Create new meeting session
        let meeting_id = Uuid::new_v4();
        let meeting = MeetingSession {
            id: meeting_id,
            started_at: chrono::Utc::now(),
            title,
            participants: Vec::new(),
            status: MeetingStatus::Starting,
            statistics: MeetingStatistics {
                total_duration: ChronoDuration::zero(),
                speaking_time_by_speaker: std::collections::HashMap::new(),
                total_words: 0,
                total_segments: 0,
                average_confidence: 0.0,
                speaker_count: 0,
            },
        };
        
        *self.current_meeting.write().await = Some(meeting);
        
        // Start all pipelines
        self.start_pipelines().await?;
        
        // Update status
        {
            let mut status = self.system_status.write().await;
            status.recording_status = MeetingStatus::Recording;
        }
        
        println!("✅ Meeting started: {}", meeting_id);
        println!("🎙️  Continuous recording and analysis active");
        
        Ok(meeting_id)
    }
    
    pub async fn stop_meeting(&self, force: bool) -> Result<()> {
        println!("🛑 Stopping meeting recording...");
        
        // Update status
        {
            let mut status = self.system_status.write().await;
            status.recording_status = MeetingStatus::Stopping;
        }
        
        // Stop pipelines
        self.stop_pipelines().await?;
        
        // Finalize meeting
        if let Some(meeting) = self.current_meeting.write().await.take() {
            let duration = chrono::Utc::now() - meeting.started_at;
            println!("📊 Meeting completed: {} (Duration: {})", meeting.id, format_duration(&duration));
        }
        
        // Update status
        {
            let mut status = self.system_status.write().await;
            status.recording_status = MeetingStatus::Completed;
        }
        
        println!("✅ Meeting recording stopped");
        Ok(())
    }
    
    pub async fn pause_recording(&self) -> Result<()> {
        println!("⏸️  Pausing recording...");
        
        self.audio_pipeline.pause().await?;
        
        {
            let mut status = self.system_status.write().await;
            status.recording_status = MeetingStatus::Paused;
        }
        
        println!("⏸️  Recording paused");
        Ok(())
    }
    
    pub async fn resume_recording(&self) -> Result<()> {
        println!("▶️  Resuming recording...");
        
        self.audio_pipeline.resume().await?;
        
        {
            let mut status = self.system_status.write().await;
            status.recording_status = MeetingStatus::Recording;
        }
        
        println!("▶️  Recording resumed");
        Ok(())
    }
    
    pub async fn get_status(&self) -> SystemStatus {
        // Update queue status
        let (audio, transcription, diarization, vectorization) = self.processing_queue.queue_sizes().await;
        let total_backlog = audio + transcription + diarization + vectorization;
        
        let audio_status = self.audio_pipeline.get_status().await;
        
        let mut status = self.system_status.read().await.clone();
        status.queue_status = QueueStatus {
            audio_queue_size: audio,
            transcription_queue_size: transcription,
            diarization_queue_size: diarization,
            vectorization_queue_size: vectorization,
            total_backlog,
        };
        
        status
    }
    
    async fn start_pipelines(&self) -> Result<()> {
        println!("🔧 Starting processing pipelines...");
        
        // Start audio pipeline
        self.audio_pipeline.start(self.cancellation_token.clone()).await?;
        
        // Start transcription pipeline
        self.transcription_pipeline.start(self.cancellation_token.clone()).await?;
        
        // Start diarization pipeline
        self.diarization_pipeline.start(self.cancellation_token.clone()).await?;
        
        // Start vectorization pipeline
        self.vectorization_pipeline.start(self.cancellation_token.clone()).await?;
        
        // Start storage pipeline
        self.storage_pipeline.start(self.cancellation_token.clone()).await?;
        
        println!("✅ All pipelines started");
        Ok(())
    }
    
    async fn stop_pipelines(&self) -> Result<()> {
        println!("🔧 Stopping processing pipelines...");
        
        // Signal cancellation
        self.cancellation_token.cancel();
        
        // Stop pipelines in reverse order
        self.storage_pipeline.stop().await?;
        self.vectorization_pipeline.stop().await?;
        self.diarization_pipeline.stop().await?;
        self.transcription_pipeline.stop().await?;
        self.audio_pipeline.stop().await?;
        
        println!("✅ All pipelines stopped");
        Ok(())
    }
}

// TODO: Implement placeholder pipeline structs
pub struct TranscriptionPipeline {}
pub struct DiarizationPipeline {}
pub struct VectorizationPipeline {}
pub struct StoragePipeline {}

impl TranscriptionPipeline {
    pub fn new(_receiver: AudioChunkReceiver) -> (Self, TranscriptReceiver) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self {}, receiver)
    }
    
    pub async fn start(&self, _token: CancellationToken) -> Result<()> {
        println!("🎤 Transcription pipeline started");
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        println!("🎤 Transcription pipeline stopped");
        Ok(())
    }
}

impl DiarizationPipeline {
    pub fn new(_receiver: TranscriptReceiver) -> (Self, DiarizedReceiver) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self {}, receiver)
    }
    
    pub async fn start(&self, _token: CancellationToken) -> Result<()> {
        println!("👥 Diarization pipeline started");
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        println!("👥 Diarization pipeline stopped");
        Ok(())
    }
}

impl VectorizationPipeline {
    pub fn new(_receiver: DiarizedReceiver) -> (Self, VectorizedReceiver) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self {}, receiver)
    }
    
    pub async fn start(&self, _token: CancellationToken) -> Result<()> {
        println!("🔮 Vectorization pipeline started");
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        println!("🔮 Vectorization pipeline stopped");
        Ok(())
    }
}

impl StoragePipeline {
    pub fn new(_receiver: VectorizedReceiver) -> Self {
        Self {}
    }
    
    pub async fn start(&self, _token: CancellationToken) -> Result<()> {
        println!("💾 Storage pipeline started");
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        println!("💾 Storage pipeline stopped");
        Ok(())
    }
}

fn format_duration(duration: &chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Handle database management commands
pub async fn handle_database_command(action: DatabaseAction) -> Result<()> {
    match action {
        DatabaseAction::Stats => {
            println!("📊 Database Statistics");
            println!("======================");
            
            // Check if advanced database exists
            let db_path = dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".meeting-assistant")
                .join("advanced_meetings.db");
            
            if !db_path.exists() {
                println!("❌ Advanced meeting database not found at: {}", db_path.display());
                println!("💡 Run a meeting session to create the database first.");
                return Ok(());
            }
            
            // Use sqlite3 command to get basic stats
            use std::process::Command;
            
            println!("📁 Database Path: {}", db_path.display());
            
            // Get file size
            let metadata = std::fs::metadata(&db_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            println!("💾 File Size: {:.2} MB", size_mb);
            
            // Get table counts using sqlite3
            let tables = ["meetings", "utterances", "speaker_profiles", "audio_segments"];
            
            for table in &tables {
                match Command::new("sqlite3")
                    .arg(&db_path)
                    .arg(&format!("SELECT COUNT(*) FROM {};", table))
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let output_str = String::from_utf8_lossy(&output.stdout);
                            let count = output_str.trim();
                            println!("📋 {}: {} records", table, count);
                        } else {
                            println!("⚠️  {}: Error reading table", table);
                        }
                    }
                    Err(_) => {
                        println!("⚠️  {}: sqlite3 command not available", table);
                        break;
                    }
                }
            }
            
            println!("\n💡 Use 'meeting-assistant database truncate' to clear all data");
        }
        
        DatabaseAction::Truncate { force } => {
            println!("🗑️  Database Truncation");
            println!("======================");
            
            let db_path = dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".meeting-assistant")
                .join("advanced_meetings.db");
            
            if !db_path.exists() {
                println!("❌ Advanced meeting database not found at: {}", db_path.display());
                println!("💡 Nothing to truncate.");
                return Ok(());
            }
            
            // Safety confirmation
            if !force {
                println!("⚠️  WARNING: This will permanently delete ALL meeting data!");
                println!("📊 Database: {}", db_path.display());
                println!("🗑️  This operation cannot be undone.");
                println!();
                print!("Type 'CONFIRM' to proceed with truncation: ");
                io::stdout().flush()?;
                
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                
                if input.trim() != "CONFIRM" {
                    println!("❌ Truncation cancelled.");
                    return Ok(());
                }
            }
            
            // Create backup first
            let backup_path = format!("{}.backup.{}", 
                db_path.display(), 
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            
            if let Err(e) = std::fs::copy(&db_path, &backup_path) {
                println!("⚠️  Warning: Failed to create backup: {}", e);
                if !force {
                    print!("Continue without backup? (y/N): ");
                    io::stdout().flush()?;
                    
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    
                    if !input.trim().to_lowercase().starts_with('y') {
                        println!("❌ Truncation cancelled.");
                        return Ok(());
                    }
                }
            } else {
                println!("💾 Backup created: {}", backup_path);
            }
            
            // Perform truncation using sqlite3
            use std::process::Command;
            
            println!("🗑️  Truncating database tables...");
            
            // First, delete all data in a transaction
            let delete_sql = r#"
BEGIN TRANSACTION;
DELETE FROM utterances;
DELETE FROM speaker_profiles;
DELETE FROM audio_segments;
DELETE FROM meetings;
COMMIT;
"#;
            
            match Command::new("sqlite3")
                .arg(&db_path)
                .arg(delete_sql)
                .output()
            {
                Ok(output) => {
                    if !output.status.success() {
                        let error = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Database deletion failed: {}", error));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to execute sqlite3 delete command: {}", e));
                }
            }
            
            // Then run VACUUM separately (outside of transaction)
            match Command::new("sqlite3")
                .arg(&db_path)
                .arg("VACUUM;")
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        println!("✅ Database truncated successfully!");
                        println!("🧹 VACUUM completed - disk space reclaimed");
                        
                        // Show final stats
                        let metadata = std::fs::metadata(&db_path)?;
                        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                        println!("💾 Database size after truncation: {:.2} MB", size_mb);
                    } else {
                        let error = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Database VACUUM failed: {}", error));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to execute sqlite3 VACUUM command: {}", e));
                }
            }
        }
        
        DatabaseAction::Backup { path } => {
            println!("💾 Database Backup");
            println!("==================");
            
            let db_path = dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".meeting-assistant")
                .join("advanced_meetings.db");
            
            if !db_path.exists() {
                println!("❌ Advanced meeting database not found at: {}", db_path.display());
                return Ok(());
            }
            
            let backup_path = if let Some(custom_path) = path {
                PathBuf::from(custom_path)
            } else {
                db_path.parent()
                    .unwrap()
                    .join(format!("advanced_meetings.backup.{}.db", 
                        chrono::Utc::now().format("%Y%m%d_%H%M%S")))
            };
            
            println!("📁 Source: {}", db_path.display());
            println!("📁 Backup: {}", backup_path.display());
            
            std::fs::copy(&db_path, &backup_path)?;
            
            let metadata = std::fs::metadata(&backup_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            
            println!("✅ Backup completed successfully!");
            println!("💾 Backup size: {:.2} MB", size_mb);
        }
        
        DatabaseAction::Optimize => {
            println!("🧹 Database Optimization");
            println!("========================");
            
            let db_path = dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".meeting-assistant")
                .join("advanced_meetings.db");
            
            if !db_path.exists() {
                println!("❌ Advanced meeting database not found at: {}", db_path.display());
                return Ok(());
            }
            
            let before_metadata = std::fs::metadata(&db_path)?;
            let before_size = before_metadata.len() as f64 / (1024.0 * 1024.0);
            
            println!("📊 Database size before optimization: {:.2} MB", before_size);
            println!("🧹 Running VACUUM...");
            
            use std::process::Command;
            
            match Command::new("sqlite3")
                .arg(&db_path)
                .arg("VACUUM;")
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        let after_metadata = std::fs::metadata(&db_path)?;
                        let after_size = after_metadata.len() as f64 / (1024.0 * 1024.0);
                        let savings = before_size - after_size;
                        
                        println!("✅ Database optimization completed!");
                        println!("📊 Database size after optimization: {:.2} MB", after_size);
                        if savings > 0.0 {
                            println!("💾 Space reclaimed: {:.2} MB", savings);
                        }
                    } else {
                        let error = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Database optimization failed: {}", error));
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to execute sqlite3 command: {}", e));
                }
            }
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Start { title, no_auto_record }) => {
            let assistant = ContinuousMeetingAssistant::new().await?;
            let meeting_id = assistant.start_meeting(title).await?;
            
            // Keep running until interrupted
            println!("Press Ctrl+C to stop recording...");
            tokio::signal::ctrl_c().await?;
            
            assistant.stop_meeting(false).await?;
        }
        
        Some(Commands::Stop { force }) => {
            println!("Stop command not yet implemented");
            // TODO: Connect to running instance and stop
        }
        
        Some(Commands::Status) => {
            println!("Status command not yet implemented");
            // TODO: Show current system status
        }
        
        Some(Commands::Search { query, mode, limit, speaker, since, confidence }) => {
            println!("Search: '{}' (mode: {}, limit: {})", query, mode, limit);
            // TODO: Implement search functionality
        }
        
        Some(Commands::Database { action }) => {
            handle_database_command(action).await?;
        }
        
        _ => {
            println!("🎯 Continuous Meeting Assistant");
            println!("Run 'meeting-assistant start' to begin recording");
            println!("Run 'meeting-assistant --help' for all commands");
        }
    }
    
    Ok(())
} 
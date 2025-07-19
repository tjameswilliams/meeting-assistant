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

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::fmt;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum AppEvent {
    AudioCapture,
    ClipboardAnalysis,
    CombinedMode,
    ScreenshotMode,
    Cancel,
    ShowHistory,
    ClearContext,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestionType {
    Audio,
    Code,
    Combined,
    Screenshot,
    PortfolioHistory,
    TechnicalKnowledge,
    Behavioral,
    General,
}

impl fmt::Display for QuestionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuestionType::Audio => write!(f, "audio"),
            QuestionType::Code => write!(f, "code_analysis"),
            QuestionType::Combined => write!(f, "combined"),
            QuestionType::Screenshot => write!(f, "screenshot"),
            QuestionType::PortfolioHistory => write!(f, "portfolio_history"),
            QuestionType::TechnicalKnowledge => write!(f, "technical_knowledge"),
            QuestionType::Behavioral => write!(f, "behavioral"),
            QuestionType::General => write!(f, "general"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEntry {
    pub timestamp: DateTime<Utc>,
    pub input: String,
    pub response: String,
    pub question_type: QuestionType,
    pub confidence: f32,
    pub key_topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub timestamp: DateTime<Utc>,
    pub question: String,
    pub question_type: String,
    pub key_topics: Vec<String>,
    pub response: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodeEntry {
    pub id: usize,
    pub timestamp: DateTime<Utc>,
    pub code: String,
    pub language: String,
    pub analysis_type: String,
    pub description: String,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentAnalysis {
    pub content_type: String,
    pub language: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionAnalysis {
    pub question_type: String,
    pub strategy: String,
    pub confidence: f32,
    pub key_topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhisperBackend {
    WhisperCpp,
    WhisperBrew,
    FasterWhisper,
    StandardWhisper,
    OpenAIAPI,
}

impl fmt::Display for WhisperBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhisperBackend::WhisperCpp => write!(f, "whisper.cpp"),
            WhisperBackend::WhisperBrew => write!(f, "brew"),
            WhisperBackend::FasterWhisper => write!(f, "faster-whisper"),
            WhisperBackend::StandardWhisper => write!(f, "python"),
            WhisperBackend::OpenAIAPI => write!(f, "openai-api"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemStatus {
    pub audio_ready: bool,
    pub whisper_ready: bool,
    pub whisper_backend: Option<WhisperBackend>,
    pub openai_ready: bool,
    pub plugins_ready: bool,
    pub recording_active: bool,
    pub error_message: Option<String>,
}

impl SystemStatus {
    pub fn new() -> Self {
        Self {
            audio_ready: false,
            whisper_ready: false,
            whisper_backend: None,
            openai_ready: false,
            plugins_ready: false,
            recording_active: false,
            error_message: None,
        }
    }
    
    pub fn is_all_systems_ready(&self) -> bool {
        self.audio_ready && self.whisper_ready && self.openai_ready && self.plugins_ready && self.error_message.is_none()
    }
    
    pub fn get_status_summary(&self) -> String {
        if let Some(error) = &self.error_message {
            format!("❌ ERROR: {}", error)
        } else if self.is_all_systems_ready() {
            if self.recording_active {
                "🟢 ALL SYSTEMS GO - Recording Active".to_string()
            } else {
                "🟢 ALL SYSTEMS GO - Ready".to_string()
            }
        } else {
            let mut parts = Vec::new();
            
            if !self.audio_ready {
                parts.push("Audio");
            }
            if !self.whisper_ready {
                parts.push("Whisper");
            }
            if !self.openai_ready {
                parts.push("OpenAI");
            }
            if !self.plugins_ready {
                parts.push("Plugins");
            }
            
            if parts.is_empty() {
                "🟡 INITIALIZING...".to_string()
            } else {
                format!("🟡 WAITING: {}", parts.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct HotkeyInfo {
    pub key: &'static str,
    pub description: &'static str,
    pub emoji: &'static str,
}

impl HotkeyInfo {
    pub fn get_all_hotkeys() -> Vec<HotkeyInfo> {
        vec![
            HotkeyInfo { key: "A", description: "Audio capture + AI analysis", emoji: "🎙️" },
            HotkeyInfo { key: "S", description: "Clipboard analysis", emoji: "📋" },
            HotkeyInfo { key: "Q", description: "Combined audio + clipboard", emoji: "🔗" },
            HotkeyInfo { key: "W", description: "Screenshot + audio analysis", emoji: "📸" },
            HotkeyInfo { key: "R", description: "Cancel current request", emoji: "🛑" },
            HotkeyInfo { key: "H", description: "Show session history", emoji: "📚" },
            HotkeyInfo { key: "C", description: "Clear conversation context", emoji: "🔄" },
            HotkeyInfo { key: "Ctrl+C", description: "Exit application", emoji: "🚪" },
        ]
    }
    
    pub fn format_hotkeys() -> String {
        let hotkeys = Self::get_all_hotkeys();
        let mut result = String::new();
        
        for (i, hotkey) in hotkeys.iter().enumerate() {
            if i > 0 {
                result.push_str(" • ");
            }
            result.push_str(&format!("{} {}", hotkey.emoji, hotkey.key));
        }
        
        result
    }
}

#[derive(Debug, Clone)]
pub struct KeyState {
    pub last_press: std::time::Instant,
    pub tap_count: usize,
}

impl Default for KeyState {
    fn default() -> Self {
        Self {
            last_press: std::time::Instant::now(),
            tap_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub device_index: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_duration: u64,
    pub capture_duration: u64,
    /// Enhanced audio quality settings for diarization
    pub enhanced_quality: bool,
    /// Minimum sample rate for diarization (will upgrade if lower)
    pub min_diarization_sample_rate: u32,
    /// Bit depth for audio capture (16, 24, or 32)
    pub bit_depth: u16,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_index: ":7".to_string(), // Default for macOS "Tim's Input"
            sample_rate: 44100, // Upgraded default for better diarization
            channels: 1,
            buffer_duration: 8,
            capture_duration: 15,
            enhanced_quality: true, // Enable enhanced quality by default
            min_diarization_sample_rate: 44100, // Minimum for good diarization
            bit_depth: 24, // 24-bit for better dynamic range
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 1800,
            temperature: 0.5,
        }
    }
}

/// Configuration for meeting recording functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingRecordingConfig {
    pub enabled: bool,
    pub output_dir: String,
    pub format: AudioFormat,
    pub quality: AudioQuality,
    pub auto_start: bool,
    pub auto_stop_on_exit: bool,
    pub max_duration_hours: u32,
    pub compression_enabled: bool,
    pub backup_enabled: bool,
    pub post_processing_enabled: bool,
}

impl Default for MeetingRecordingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            output_dir: "~/.meeting-assistant/recordings".to_string(),
            format: AudioFormat::WAV,
            quality: AudioQuality::High,
            auto_start: true,
            auto_stop_on_exit: true,
            max_duration_hours: 8,
            compression_enabled: false,
            backup_enabled: true,
            post_processing_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioFormat {
    WAV,
    MP3,
    FLAC,
    OGG,
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioFormat::WAV => write!(f, "wav"),
            AudioFormat::MP3 => write!(f, "mp3"),
            AudioFormat::FLAC => write!(f, "flac"),
            AudioFormat::OGG => write!(f, "ogg"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioQuality {
    /// Low quality - 16kHz, 16-bit (basic transcription)
    Low,
    /// Medium quality - 22kHz, 16-bit (good transcription)
    Medium,
    /// High quality - 44.1kHz, 24-bit (excellent diarization)
    High,
    /// Ultra quality - 48kHz, 24-bit (professional diarization)
    Ultra,
    /// Broadcast quality - 48kHz, 32-bit float (studio grade)
    Broadcast,
}

impl AudioQuality {
    pub fn sample_rate(&self) -> u32 {
        match self {
            AudioQuality::Low => 16000,
            AudioQuality::Medium => 22050,
            AudioQuality::High => 44100,
            AudioQuality::Ultra => 48000,
            AudioQuality::Broadcast => 48000,
        }
    }

    pub fn bit_depth(&self) -> u16 {
        match self {
            AudioQuality::Low | AudioQuality::Medium => 16,
            AudioQuality::High | AudioQuality::Ultra => 24,
            AudioQuality::Broadcast => 32,
        }
    }

    /// Returns the FFmpeg codec string for this quality level
    pub fn ffmpeg_codec(&self) -> &'static str {
        match self {
            AudioQuality::Low | AudioQuality::Medium => "pcm_s16le",
            AudioQuality::High | AudioQuality::Ultra => "pcm_s24le",
            AudioQuality::Broadcast => "pcm_f32le",
        }
    }

    /// Returns the sample format string for FFmpeg
    pub fn sample_format(&self) -> &'static str {
        match self {
            AudioQuality::Low | AudioQuality::Medium => "s16",
            AudioQuality::High | AudioQuality::Ultra => "s24",
            AudioQuality::Broadcast => "flt",
        }
    }

    /// Returns whether this quality level is suitable for diarization
    pub fn suitable_for_diarization(&self) -> bool {
        matches!(self, AudioQuality::High | AudioQuality::Ultra | AudioQuality::Broadcast)
    }

    /// Returns the recommended quality for diarization
    pub fn for_diarization() -> Self {
        AudioQuality::High
    }

    /// Returns estimated file size multiplier compared to Low quality
    pub fn size_multiplier(&self) -> f32 {
        match self {
            AudioQuality::Low => 1.0,
            AudioQuality::Medium => 1.4,
            AudioQuality::High => 5.5,
            AudioQuality::Ultra => 6.0,
            AudioQuality::Broadcast => 12.0,
        }
    }
}

/// Status of meeting recording
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecordingStatus {
    Idle,
    Starting,
    Recording,
    Paused,
    Stopping,
    Stopped,
    Error(String),
}

impl fmt::Display for RecordingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordingStatus::Idle => write!(f, "idle"),
            RecordingStatus::Starting => write!(f, "starting"),
            RecordingStatus::Recording => write!(f, "recording"),
            RecordingStatus::Paused => write!(f, "paused"),
            RecordingStatus::Stopping => write!(f, "stopping"),
            RecordingStatus::Stopped => write!(f, "stopped"),
            RecordingStatus::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

/// Information about a meeting recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingRecordingInfo {
    pub id: String,
    pub file_path: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_seconds: u64,
    pub file_size_bytes: u64,
    pub status: RecordingStatus,
    pub format: AudioFormat,
    pub quality: AudioQuality,
    pub sample_rate: u32,
    pub channels: u16,
    pub has_transcript: bool,
    pub has_diarization: bool,
    pub metadata: HashMap<String, String>,
}

impl MeetingRecordingInfo {
    pub fn new(id: String, file_path: String, config: &MeetingRecordingConfig, audio_config: &AudioConfig) -> Self {
        Self {
            id,
            file_path,
            started_at: chrono::Utc::now(),
            ended_at: None,
            duration_seconds: 0,
            file_size_bytes: 0,
            status: RecordingStatus::Starting,
            format: config.format.clone(),
            quality: config.quality.clone(),
            sample_rate: audio_config.sample_rate,
            channels: audio_config.channels,
            has_transcript: false,
            has_diarization: false,
            metadata: HashMap::new(),
        }
    }
    
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.duration_seconds)
    }
    
    pub fn is_active(&self) -> bool {
        matches!(self.status, RecordingStatus::Recording | RecordingStatus::Starting)
    }
    
    pub fn file_size_mb(&self) -> f64 {
        self.file_size_bytes as f64 / 1024.0 / 1024.0
    }
}

/// Post-processing options for recordings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingOptions {
    pub transcription_enabled: bool,
    pub diarization_enabled: bool,
    pub noise_reduction_enabled: bool,
    pub normalize_audio: bool,
    pub generate_summary: bool,
    pub extract_key_moments: bool,
    pub confidence_threshold: f32,
}

impl Default for PostProcessingOptions {
    fn default() -> Self {
        Self {
            transcription_enabled: true,
            diarization_enabled: true,
            noise_reduction_enabled: false,
            normalize_audio: false,
            generate_summary: true,
            extract_key_moments: true,
            confidence_threshold: 0.7,
        }
    }
}

#[derive(Debug)]
pub struct StreamingResponse {
    pub content: String,
    pub is_complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Technology {
    pub name: String,
    pub category: String,
}

// Common technology categories and their items
pub const FRONTEND_TECHNOLOGIES: &[&str] = &[
    "react", "vue", "vue.js", "angular", "svelte", "ember", "backbone", "jquery",
    "bootstrap", "tailwind", "material-ui", "ant design", "next.js", "nuxt",
    "gatsby", "astro", "vite", "webpack", "parcel"
];

pub const BACKEND_TECHNOLOGIES: &[&str] = &[
    "node.js", "nodejs", "express", "koa", "fastify", "nest.js", "django",
    "flask", "fastapi", "spring", "spring boot", "laravel", "symfony",
    "ruby on rails", "rails", "phoenix", "gin", "echo", ".net", "asp.net", "core"
];

pub const PROGRAMMING_LANGUAGES: &[&str] = &[
    "javascript", "typescript", "python", "java", "c++", "c#", "go", "rust",
    "php", "ruby", "swift", "kotlin", "scala", "clojure", "elixir", "haskell",
    "dart", "r", "matlab", "perl", "lua"
];

pub const DATABASES: &[&str] = &[
    "postgresql", "postgres", "mysql", "mongodb", "redis", "cassandra",
    "dynamodb", "elasticsearch", "sqlite", "mariadb", "couchdb", "neo4j",
    "influxdb", "graphql", "prisma", "sequelize", "mongoose"
];

pub const CLOUD_TECHNOLOGIES: &[&str] = &[
    "aws", "azure", "google cloud", "gcp", "docker", "kubernetes", "terraform",
    "ansible", "jenkins", "github actions", "gitlab ci", "cloudformation",
    "helm", "istio", "consul", "vault", "nginx", "apache", "load balancer",
    "cdn", "cloudfront", "s3", "ec2", "lambda"
];

pub const DEVOPS_TECHNOLOGIES: &[&str] = &[
    "ci/cd", "continuous integration", "continuous deployment", "microservices",
    "api gateway", "service mesh", "monitoring", "logging", "grafana",
    "prometheus", "elk stack", "datadog", "new relic", "sentry", "git",
    "github", "gitlab", "bitbucket"
];

pub const MOBILE_TECHNOLOGIES: &[&str] = &[
    "react native", "flutter", "ionic", "cordova", "phonegap", "xamarin",
    "native script", "ios", "android", "swift ui", "jetpack compose"
];

pub const TESTING_TECHNOLOGIES: &[&str] = &[
    "jest", "mocha", "cypress", "selenium", "playwright", "vitest",
    "unit testing", "integration testing", "e2e testing", "tdd", "bdd",
    "test driven development", "behavior driven development"
];

pub const DATA_TECHNOLOGIES: &[&str] = &[
    "machine learning", "artificial intelligence", "data science", "pandas",
    "numpy", "scikit-learn", "tensorflow", "pytorch", "jupyter", "apache spark",
    "hadoop", "kafka", "rabbitmq", "etl", "data pipeline", "big data", "analytics"
];

pub const ARCHITECTURE_PATTERNS: &[&str] = &[
    "microservices", "monolith", "serverless", "event driven", "domain driven design",
    "ddd", "clean architecture", "hexagonal", "cqrs", "event sourcing",
    "saga pattern", "circuit breaker", "api first", "rest api", "graphql api",
    "websockets", "grpc"
];

// Common abbreviations and their full forms
pub const TECHNOLOGY_ABBREVIATIONS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("k8s", "kubernetes"),
    ("tf", "terraform"),
    ("pg", "postgresql"),
    ("mongo", "mongodb"),
    ("es", "elasticsearch"),
    ("ml", "machine learning"),
    ("ai", "artificial intelligence"),
    ("api", "rest api"),
    ("db", "database"),
    ("ui", "user interface"),
    ("ux", "user experience"),
];

pub fn get_all_technologies() -> Vec<Technology> {
    let mut technologies = Vec::new();
    
    for &tech in FRONTEND_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "frontend".to_string(),
        });
    }
    
    for &tech in BACKEND_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "backend".to_string(),
        });
    }
    
    for &tech in PROGRAMMING_LANGUAGES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "language".to_string(),
        });
    }
    
    for &tech in DATABASES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "database".to_string(),
        });
    }
    
    for &tech in CLOUD_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "cloud".to_string(),
        });
    }
    
    for &tech in DEVOPS_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "devops".to_string(),
        });
    }
    
    for &tech in MOBILE_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "mobile".to_string(),
        });
    }
    
    for &tech in TESTING_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "testing".to_string(),
        });
    }
    
    for &tech in DATA_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "data".to_string(),
        });
    }
    
    for &tech in ARCHITECTURE_PATTERNS {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "architecture".to_string(),
        });
    }
    
    technologies
} 
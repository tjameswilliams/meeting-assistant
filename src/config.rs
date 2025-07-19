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

use anyhow::{Result, Context};
use std::env;
use std::path::PathBuf;
use std::collections::HashMap;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use crate::types::{AudioConfig, OpenAIConfig, MeetingRecordingConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LLMProvider {
    OpenAI,
    Ollama,
    Custom(String),
}

impl Default for LLMProvider {
    fn default() -> Self {
        LLMProvider::OpenAI
    }
}

impl std::fmt::Display for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMProvider::OpenAI => write!(f, "openai"),
            LLMProvider::Ollama => write!(f, "ollama"),
            LLMProvider::Custom(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProviderConfig {
    pub active_provider: LLMProvider,
    pub fallback_to_openai: bool,
    pub provider_configs: HashMap<String, serde_json::Value>,
}

impl Default for LLMProviderConfig {
    fn default() -> Self {
        Self {
            active_provider: LLMProvider::OpenAI,
            fallback_to_openai: true,
            provider_configs: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub openai: OpenAIConfig,
    pub llm_provider: LLMProviderConfig,
    pub recording: MeetingRecordingConfig,
    pub temp_dir: PathBuf,
    pub double_tap_window_ms: u64,
    pub debounce_ms: u64,
    pub max_recording_time: u64,
}

impl Config {
    pub async fn load() -> Result<Self> {
        // Load environment variables from .env file if it exists
        dotenv::dotenv().ok();
        
        // OpenAI configuration
        let openai_api_key = env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not found")?;
        
        let openai_model = env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());
        
        let openai_max_tokens = env::var("OPENAI_MAX_TOKENS")
            .unwrap_or_else(|_| "1800".to_string())
            .parse::<u32>()
            .unwrap_or(1800);
        
        let openai_temperature = env::var("OPENAI_TEMPERATURE")
            .unwrap_or_else(|_| "0.5".to_string())
            .parse::<f32>()
            .unwrap_or(0.5);
        
        let openai = OpenAIConfig {
            api_key: openai_api_key,
            model: openai_model,
            max_tokens: openai_max_tokens,
            temperature: openai_temperature,
        };
        
        // Audio configuration
        let audio_device = env::var("AUDIO_DEVICE")
            .unwrap_or_else(|_| ":0".to_string()); // Default to "Tim's Input" device
        
        let audio_sample_rate = env::var("AUDIO_SAMPLE_RATE")
            .unwrap_or_else(|_| "44100".to_string()) // Upgraded default for diarization
            .parse::<u32>()
            .unwrap_or(44100);
        
        let audio_channels = env::var("AUDIO_CHANNELS")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u16>()
            .unwrap_or(1);
        
        let buffer_duration = env::var("BUFFER_DURATION")
            .unwrap_or_else(|_| "8".to_string())
            .parse::<u64>()
            .unwrap_or(8);
        
        let capture_duration = env::var("CAPTURE_DURATION")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<u64>()
            .unwrap_or(15);

        // Enhanced audio quality settings
        let enhanced_quality = env::var("AUDIO_ENHANCED_QUALITY")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        let min_diarization_sample_rate = env::var("AUDIO_MIN_DIARIZATION_SAMPLE_RATE")
            .unwrap_or_else(|_| "44100".to_string())
            .parse::<u32>()
            .unwrap_or(44100);

        let bit_depth = env::var("AUDIO_BIT_DEPTH")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<u16>()
            .unwrap_or(24);

        // Validate bit depth
        let bit_depth = match bit_depth {
            16 | 24 | 32 => bit_depth,
            _ => {
                tracing::warn!("Invalid bit depth {}, using 24-bit", bit_depth);
                24
            }
        };

        // Ensure sample rate meets diarization requirements if enhanced quality is enabled
        let effective_sample_rate = if enhanced_quality && audio_sample_rate < min_diarization_sample_rate {
            tracing::info!("Upgrading sample rate from {}Hz to {}Hz for enhanced diarization quality", 
                          audio_sample_rate, min_diarization_sample_rate);
            min_diarization_sample_rate
        } else {
            audio_sample_rate
        };
        
        let audio = AudioConfig {
            device_index: audio_device,
            sample_rate: effective_sample_rate,
            channels: audio_channels,
            buffer_duration,
            capture_duration,
            enhanced_quality,
            min_diarization_sample_rate,
            bit_depth,
        };
        
        // Timing configuration
        let double_tap_window_ms = env::var("DOUBLE_TAP_WINDOW_MS")
            .unwrap_or_else(|_| "500".to_string())
            .parse::<u64>()
            .unwrap_or(500);
        
        let debounce_ms = env::var("DEBOUNCE_MS")
            .unwrap_or_else(|_| "50".to_string())
            .parse::<u64>()
            .unwrap_or(50);
        
        let max_recording_time = env::var("MAX_RECORDING_TIME")
            .unwrap_or_else(|_| "30000".to_string())
            .parse::<u64>()
            .unwrap_or(30000);
        
        // Temporary directory
        let temp_dir = env::var("TEMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".meeting-assistant")
                    .join("temp")
            });
        
        // Ensure temp directory exists
        std::fs::create_dir_all(&temp_dir)
            .context("Failed to create temporary directory")?;
        
        // LLM Provider configuration
        let active_provider_str = env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "openai".to_string())
            .to_lowercase();
        
        let active_provider = match active_provider_str.as_str() {
            "openai" => LLMProvider::OpenAI,
            "ollama" => LLMProvider::Ollama,
            custom => LLMProvider::Custom(custom.to_string()),
        };
        
        let fallback_to_openai = env::var("LLM_FALLBACK_TO_OPENAI")
            .unwrap_or_else(|_| {
                // Smart default: if user explicitly set LLM_PROVIDER to non-OpenAI, 
                // default fallback to false unless explicitly enabled
                match active_provider_str.as_str() {
                    "openai" => "true".to_string(),
                    _ => "false".to_string(),
                }
            })
            .parse::<bool>()
            .unwrap_or(false);
        
        // Ollama configuration
        let mut provider_configs = HashMap::new();
        
        let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama2:7b".to_string());
        
        let ollama_config = serde_json::json!({
            "base_url": env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string()),
            "default_model": ollama_model,
            "timeout_seconds": env::var("OLLAMA_TIMEOUT").unwrap_or_else(|_| "30".to_string()).parse::<u64>().unwrap_or(30),
            "max_retries": env::var("OLLAMA_MAX_RETRIES").unwrap_or_else(|_| "3".to_string()).parse::<u32>().unwrap_or(3),
            "health_check_interval": 60,
            "enabled": true,
            "auto_pull_models": env::var("OLLAMA_AUTO_PULL").unwrap_or_else(|_| "false".to_string()).parse::<bool>().unwrap_or(false),
            "preferred_models": [
                "llama2:7b",
                "codellama:7b", 
                "mistral:7b",
                "neural-chat:7b"
            ]
        });
        provider_configs.insert("ollama".to_string(), ollama_config);
        
        let llm_provider = LLMProviderConfig {
            active_provider,
            fallback_to_openai,
            provider_configs,
        };
        
        // Meeting recording configuration
        let recording_enabled = env::var("MEETING_RECORDING_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let recording_output_dir = env::var("MEETING_RECORDING_OUTPUT_DIR")
            .unwrap_or_else(|_| {
                home_dir()
                    .unwrap_or_else(|| PathBuf::from("/tmp"))
                    .join(".meeting-assistant")
                    .join("recordings")
                    .to_string_lossy()
                    .to_string()
            });
        
        let recording_format = env::var("MEETING_RECORDING_FORMAT")
            .unwrap_or_else(|_| "wav".to_string())
            .to_lowercase();
        
        let recording_quality = env::var("MEETING_RECORDING_QUALITY")
            .unwrap_or_else(|_| "high".to_string())
            .to_lowercase();
        
        let recording_auto_start = env::var("MEETING_RECORDING_AUTO_START")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let recording_max_duration = env::var("MEETING_RECORDING_MAX_DURATION_HOURS")
            .unwrap_or_else(|_| "8".to_string())
            .parse::<u32>()
            .unwrap_or(8);
        
        let recording_post_processing = env::var("MEETING_RECORDING_POST_PROCESSING")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);
        
        let recording = MeetingRecordingConfig {
            enabled: recording_enabled,
            output_dir: recording_output_dir,
            format: match recording_format.as_str() {
                "mp3" => crate::types::AudioFormat::MP3,
                "flac" => crate::types::AudioFormat::FLAC,
                "ogg" => crate::types::AudioFormat::OGG,
                _ => crate::types::AudioFormat::WAV,
            },
            quality: match recording_quality.as_str() {
                "low" => crate::types::AudioQuality::Low,
                "medium" => crate::types::AudioQuality::Medium,
                "ultra" => crate::types::AudioQuality::Ultra,
                _ => crate::types::AudioQuality::High,
            },
            auto_start: recording_auto_start,
            auto_stop_on_exit: true,
            max_duration_hours: recording_max_duration,
            compression_enabled: false,
            backup_enabled: true,
            post_processing_enabled: recording_post_processing,
        };
        
        Ok(Config {
            audio,
            openai,
            llm_provider,
            recording,
            temp_dir,
            double_tap_window_ms,
            debounce_ms,
            max_recording_time,
        })
    }
    
    pub fn get_temp_file(&self, prefix: &str, extension: &str) -> PathBuf {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let filename = format!("{}_{}.{}", prefix, timestamp, extension);
        self.temp_dir.join(filename)
    }
} 
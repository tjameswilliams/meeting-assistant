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
use dirs::home_dir;
use crate::types::{AudioConfig, OpenAIConfig};

#[derive(Debug, Clone)]
pub struct Config {
    pub audio: AudioConfig,
    pub openai: OpenAIConfig,
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
            .unwrap_or_else(|_| "16000".to_string())
            .parse::<u32>()
            .unwrap_or(16000);
        
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
        
        let audio = AudioConfig {
            device_index: audio_device,
            sample_rate: audio_sample_rate,
            channels: audio_channels,
            buffer_duration,
            capture_duration,
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
                    .join(".interview-assistant")
                    .join("temp")
            });
        
        // Ensure temp directory exists
        std::fs::create_dir_all(&temp_dir)
            .context("Failed to create temporary directory")?;
        
        Ok(Config {
            audio,
            openai,
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
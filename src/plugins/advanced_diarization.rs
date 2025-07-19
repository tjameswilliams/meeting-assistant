/*
 * Meeting Assistant CLI - Advanced Speaker Diarization Plugin
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
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;


use crate::plugin_system::*;

/// Diarization provider options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiarizationProvider {
    /// Whisper + PyAnnote (original implementation)
    WhisperPyAnnote,
    /// ElevenLabs Scribe v1 (high quality)
    ElevenLabs,
}

impl Default for DiarizationProvider {
    fn default() -> Self {
        Self::ElevenLabs
    }
}

impl std::fmt::Display for DiarizationProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WhisperPyAnnote => write!(f, "whisper_pyannote"),
            Self::ElevenLabs => write!(f, "elevenlabs"),
        }
    }
}

/// Configuration for Advanced Diarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedDiarizationConfig {
    /// Enable/disable the diarization plugin
    pub enabled: bool,
    /// Choose diarization provider
    pub provider: DiarizationProvider,
    /// ElevenLabs API key (required for ElevenLabs provider)
    pub elevenlabs_api_key: Option<String>,
    /// Whisper model size: tiny, base, small, medium, large (for WhisperPyAnnote)
    pub whisper_model_size: String,
    /// Language hint for processing (e.g., "en", "auto")
    pub language: String,
    /// PyAnnote model path or HuggingFace model ID (for WhisperPyAnnote)
    pub pyannote_model_path: String,
    /// Minimum segment duration in seconds
    pub min_segment_duration: f32,
    /// Maximum number of speakers (0 = auto-detect)
    pub max_speakers: usize,
    /// Speaker similarity threshold for clustering
    pub speaker_threshold: f32,
    /// Audio sample rate for processing
    pub sample_rate: u32,
    /// Enable audio event detection (laughter, applause, etc.)
    pub detect_audio_events: bool,
}

impl Default for AdvancedDiarizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: DiarizationProvider::default(),
            elevenlabs_api_key: None,
            whisper_model_size: "base".to_string(),
            language: "auto".to_string(),
            pyannote_model_path: "pyannote/speaker-diarization-3.1".to_string(),
            min_segment_duration: 0.5,
            max_speakers: 10,
            speaker_threshold: 0.7,
            sample_rate: 16000,
            detect_audio_events: true,
        }
    }
}

/// Speaker information with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerInfo {
    pub id: String,
    pub name: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub total_segments: usize,
    pub total_duration: f32,
    pub confidence: f32,
    pub embedding: Vec<f32>,
}

/// Diarized segment with speaker and transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizedSegment {
    pub start_time: f32,
    pub end_time: f32,
    pub speaker_id: String,
    pub text: String,
    pub confidence: f32,
    pub language: Option<String>,
    pub segment_type: SegmentType,
}

/// Type of segment (word, spacing, or audio event)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SegmentType {
    Word,
    Spacing,
    AudioEvent,
}

impl Default for SegmentType {
    fn default() -> Self {
        Self::Word
    }
}

/// ElevenLabs API response structures
#[derive(Debug, Deserialize)]
struct ElevenLabsResponse {
    language_code: String,
    language_probability: f32,
    text: String,
    words: Vec<ElevenLabsWord>,
}

#[derive(Debug, Deserialize)]
struct ElevenLabsWord {
    text: String,
    start: f32,
    end: f32,
    #[serde(rename = "type")]
    word_type: String,
    speaker_id: String,
}

/// Transcription result from Whisper (for WhisperPyAnnote provider)
#[derive(Debug, Clone)]
struct WhisperSegment {
    pub start: f32,
    pub end: f32,
    pub text: String,
    pub confidence: Option<f32>,
}

/// Speaker diarization result from PyAnnote (for WhisperPyAnnote provider)
#[derive(Debug, Clone)]
struct PyAnnoteSegment {
    pub start: f32,
    pub end: f32,
    pub speaker_id: String,
    pub confidence: f32,
}

/// Advanced speaker diarization plugin
pub struct AdvancedDiarizationPlugin {
    config: AdvancedDiarizationConfig,
    speaker_profiles: Arc<RwLock<HashMap<String, SpeakerInfo>>>,
    enabled: bool,
    #[cfg(feature = "whisper-pyannote")]
    whisper_context: Option<whisper_rs::WhisperContext>,
    #[cfg(feature = "whisper-pyannote")]
    pyannote_session: Option<ort::Session>,
    http_client: reqwest::Client,
}

impl AdvancedDiarizationPlugin {
    pub fn new() -> Self {
        Self {
            config: AdvancedDiarizationConfig::default(),
            speaker_profiles: Arc::new(RwLock::new(HashMap::new())),
            enabled: true,
            #[cfg(feature = "whisper-pyannote")]
            whisper_context: None,
            #[cfg(feature = "whisper-pyannote")]
            pyannote_session: None,
            http_client: reqwest::Client::new(),
        }
    }

    /// Initialize models based on the selected provider
    async fn initialize_models(&mut self) -> Result<()> {
        match self.config.provider {
            DiarizationProvider::ElevenLabs => {
                // Validate ElevenLabs API key
                if self.config.elevenlabs_api_key.is_none() {
                    // Try to get from environment
                    self.config.elevenlabs_api_key = std::env::var("ELEVENLABS_API_KEY").ok()
                        .or_else(|| std::env::var("XI_API_KEY").ok());
                }
                
                if self.config.elevenlabs_api_key.is_none() {
                    return Err(anyhow::anyhow!(
                        "ElevenLabs API key not found. Please set ELEVENLABS_API_KEY environment variable or configure it in plugin settings."
                    ));
                }
                
                // Test API connection
                self.test_elevenlabs_connection().await?;
                tracing::info!("ElevenLabs API connection verified");
            }
            DiarizationProvider::WhisperPyAnnote => {
                // Initialize Whisper and PyAnnote models (existing code)
                self.initialize_whisper_pyannote().await?;
            }
        }
        Ok(())
    }

    /// Test ElevenLabs API connection
    async fn test_elevenlabs_connection(&self) -> Result<()> {
        let api_key = self.config.elevenlabs_api_key.as_ref()
            .context("ElevenLabs API key is required")?;
        
        let response = self.http_client
            .get("https://api.elevenlabs.io/v1/user")
            .header("xi-api-key", api_key)
            .send()
            .await
            .context("Failed to connect to ElevenLabs API")?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "ElevenLabs API authentication failed: {}",
                response.status()
            ));
        }
        
        Ok(())
    }

    /// Initialize Whisper and PyAnnote models (existing implementation)
    async fn initialize_whisper_pyannote(&mut self) -> Result<()> {
        #[cfg(feature = "whisper-pyannote")]
        {
            // Initialize Whisper
            tracing::info!("Loading Whisper model: {}", self.config.whisper_model_size);
            let whisper_params = whisper_rs::WhisperContextParameters::default();
            let model_path = self.download_whisper_model().await?;
            self.whisper_context = Some(
                whisper_rs::WhisperContext::new_with_params(&model_path, whisper_params)
                    .context("Failed to initialize Whisper context")?
            );

            // Initialize PyAnnote via ONNX
            tracing::info!("Loading PyAnnote model: {}", self.config.pyannote_model_path);
            let pyannote_model_path = self.download_pyannote_model().await?;
            self.pyannote_session = Some(
                ort::Session::builder()?
                    .with_optimization_level(ort::GraphOptimizationLevel::All)?
                    .commit_from_file(&pyannote_model_path)
                    .context("Failed to load PyAnnote ONNX model")?
            );
        }

        #[cfg(not(feature = "whisper-pyannote"))]
        {
            return Err(anyhow::anyhow!("whisper-pyannote feature not enabled"));
        }

        Ok(())
    }

    /// Process audio file with ElevenLabs Speech-to-Text
    async fn process_audio_with_elevenlabs(&self, audio_file: &Path) -> Result<Vec<DiarizedSegment>> {
        tracing::info!("🎯 Processing audio with ElevenLabs Scribe v1: {:?}", audio_file);
        
        let api_key = self.config.elevenlabs_api_key.as_ref()
            .context("ElevenLabs API key is required")?;
        
        // Read audio file
        let audio_bytes = tokio::fs::read(audio_file).await
            .context("Failed to read audio file")?;
        
        // Prepare multipart form with model parameter
        let mut form = reqwest::multipart::Form::new()
            .part("audio", reqwest::multipart::Part::bytes(audio_bytes)
                .file_name(audio_file.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("audio.wav")
                    .to_string())
                .mime_str("audio/wav")?)
            .text("model", "scribe_v1"); // Explicitly specify scribe_v1 model for diarization
        
        // Add optional parameters as form fields
        if self.config.max_speakers > 0 {
            form = form.text("max_speakers", self.config.max_speakers.to_string());
        }
        
        if self.config.language != "auto" {
            form = form.text("language", self.config.language.clone());
        }
        
        if self.config.detect_audio_events {
            form = form.text("audio_events", "true");
        }
        
        // Use the base URL without query parameters since we're using form fields
        let url = "https://api.elevenlabs.io/v1/speech-to-text".to_string();
        
        // Make API request
        let response = self.http_client
            .post(&url)
            .header("xi-api-key", api_key)
            .multipart(form)
            .send()
            .await
            .context("Failed to send request to ElevenLabs API")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!(
                "ElevenLabs API request failed ({}): {}",
                status,
                error_text
            ));
        }
        
        // Parse response
        let elevenlabs_response: ElevenLabsResponse = response.json().await
            .context("Failed to parse ElevenLabs response")?;
        
        // Convert to DiarizedSegment format
        let mut segments = Vec::new();
        for word in elevenlabs_response.words {
            let segment_type = match word.word_type.as_str() {
                "word" => SegmentType::Word,
                "spacing" => SegmentType::Spacing,
                "audio_event" => SegmentType::AudioEvent,
                _ => SegmentType::Word, // default
            };
            
            // Only include non-spacing segments in main results, or include all if requested
            if segment_type != SegmentType::Spacing || word.text.trim().is_empty() {
                segments.push(DiarizedSegment {
                    start_time: word.start,
                    end_time: word.end,
                    speaker_id: word.speaker_id,
                    text: word.text,
                    confidence: 0.9, // ElevenLabs doesn't provide per-word confidence, use default
                    language: Some(elevenlabs_response.language_code.clone()),
                    segment_type,
                });
            }
        }
        
        // Group consecutive words from same speaker into sentences
        let grouped_segments = self.group_segments_by_speaker(&segments);
        
        tracing::info!("🎯 ElevenLabs processing completed: {} segments", grouped_segments.len());
        Ok(grouped_segments)
    }

    /// Group consecutive segments from the same speaker into sentences
    fn group_segments_by_speaker(&self, segments: &[DiarizedSegment]) -> Vec<DiarizedSegment> {
        if segments.is_empty() {
            return Vec::new();
        }
        
        let mut grouped = Vec::new();
        let mut current_group: Vec<&DiarizedSegment> = Vec::new();
        let mut current_speaker = &segments[0].speaker_id;
        
        for segment in segments.iter().filter(|s| s.segment_type == SegmentType::Word) {
            if segment.speaker_id == *current_speaker {
                current_group.push(segment);
            } else {
                // Finish current group
                if !current_group.is_empty() {
                    grouped.push(self.merge_segments(current_group));
                }
                
                // Start new group
                current_group = vec![segment];
                current_speaker = &segment.speaker_id;
            }
        }
        
        // Don't forget the last group
        if !current_group.is_empty() {
            grouped.push(self.merge_segments(current_group));
        }
        
        grouped
    }

    /// Merge multiple segments into a single segment
    fn merge_segments(&self, segments: Vec<&DiarizedSegment>) -> DiarizedSegment {
        if segments.is_empty() {
            panic!("Cannot merge empty segments");
        }
        
        let first = segments[0];
        let last = segments[segments.len() - 1];
        
        let merged_text = segments.iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        
        let avg_confidence = segments.iter()
            .map(|s| s.confidence)
            .sum::<f32>() / segments.len() as f32;
        
        DiarizedSegment {
            start_time: first.start_time,
            end_time: last.end_time,
            speaker_id: first.speaker_id.clone(),
            text: merged_text,
            confidence: avg_confidence,
            language: first.language.clone(),
            segment_type: SegmentType::Word,
        }
    }

    /// Process audio file using the selected provider
    pub async fn process_audio(&self, audio_file: &Path) -> Result<Vec<DiarizedSegment>> {
        match self.config.provider {
            DiarizationProvider::ElevenLabs => {
                self.process_audio_with_elevenlabs(audio_file).await
            }
            DiarizationProvider::WhisperPyAnnote => {
                self.process_audio_with_whisper_pyannote(audio_file).await
            }
        }
    }

    /// Process audio file using Whisper + PyAnnote pipeline (existing implementation)
    pub async fn process_audio_with_whisper_pyannote(&self, audio_file: &Path) -> Result<Vec<DiarizedSegment>> {
        tracing::info!("🎯 Processing audio with Whisper + PyAnnote: {:?}", audio_file);
        
        // For now, we'll use a fallback approach calling Python PyAnnote
        // This will be replaced with pure Rust implementation once ONNX models are available
        let result = self.process_via_python_fallback(audio_file).await?;
        
        tracing::info!("🎯 Whisper + PyAnnote completed: {} segments", result.len());
        Ok(result)
    }

    /// Find the Python helper script
    fn find_python_helper_script(&self) -> Result<PathBuf> {
        // Check common locations for the helper script
        let script_name = "whisper_pyannote_helper.py";
        let possible_paths = vec![
            PathBuf::from("scripts").join(script_name),
            PathBuf::from("../scripts").join(script_name),
            PathBuf::from("./scripts").join(script_name),
            PathBuf::from(script_name),
        ];
        
        for path in possible_paths {
            if path.exists() {
                return Ok(path);
            }
        }
        
        Err(anyhow::anyhow!("Python helper script not found: {}", script_name))
    }

    /// Fallback implementation using Python subprocess (for WhisperPyAnnote)
    async fn process_via_python_fallback(&self, audio_file: &Path) -> Result<Vec<DiarizedSegment>> {
        use std::process::Command;
        
        tracing::info!("Using Python fallback for Whisper + PyAnnote processing");
        
        // Find the Python helper script
        let script_path = self.find_python_helper_script()?;
        
        // Get HuggingFace token from environment
        let hf_token = std::env::var("HUGGINGFACE_HUB_TOKEN")
            .or_else(|_| std::env::var("HF_TOKEN"))
            .ok();
        
        // Build command arguments
        let mut cmd = Command::new("python3");
        cmd.arg(&script_path)
           .arg(audio_file)
           .arg("--whisper-model")
           .arg(&self.config.whisper_model_size)
           .arg("--pyannote-model")
           .arg(&self.config.pyannote_model_path);
        
        if let Some(token) = &hf_token {
            cmd.arg("--hf-token").arg(token);
        }
        
        if self.config.max_speakers > 0 {
            cmd.arg("--max-speakers").arg(self.config.max_speakers.to_string());
        }
        
        // Execute Python script
        let output = cmd.output()
            .context("Failed to execute Python helper script")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Python script failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: serde_json::Value = serde_json::from_str(&stdout)
            .context("Failed to parse Python script output")?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow::anyhow!("Python script error: {}", error));
        }
        
        let segments_json = result["segments"].as_array()
            .context("Invalid segments format")?;
        
        let mut segments = Vec::new();
        for segment_json in segments_json {
            let segment = DiarizedSegment {
                start_time: segment_json["start_time"].as_f64().unwrap_or(0.0) as f32,
                end_time: segment_json["end_time"].as_f64().unwrap_or(0.0) as f32,
                speaker_id: segment_json["speaker_id"].as_str().unwrap_or("Unknown").to_string(),
                text: segment_json["text"].as_str().unwrap_or("").to_string(),
                confidence: segment_json["confidence"].as_f64().unwrap_or(0.0) as f32,
                language: segment_json["language"].as_str().map(|s| s.to_string()),
                segment_type: SegmentType::Word,
            };
            segments.push(segment);
        }
        
        Ok(segments)
    }

    /// Align Whisper transcription with PyAnnote diarization (existing implementation)
    fn align_transcription_with_diarization(
        &self,
        whisper_segments: Vec<WhisperSegment>,
        pyannote_segments: Vec<PyAnnoteSegment>,
    ) -> Vec<DiarizedSegment> {
        let mut result = Vec::new();
        
        for whisper_seg in whisper_segments {
            // Find overlapping speaker segments
            let mut best_speaker = "Unknown".to_string();
            let mut best_overlap = 0.0;
            
            for pyannote_seg in &pyannote_segments {
                let overlap_start = whisper_seg.start.max(pyannote_seg.start);
                let overlap_end = whisper_seg.end.min(pyannote_seg.end);
                
                if overlap_start < overlap_end {
                    let overlap_duration = overlap_end - overlap_start;
                    if overlap_duration > best_overlap {
                        best_overlap = overlap_duration;
                        best_speaker = pyannote_seg.speaker_id.clone();
                    }
                }
            }
            
            result.push(DiarizedSegment {
                start_time: whisper_seg.start,
                end_time: whisper_seg.end,
                speaker_id: best_speaker,
                text: whisper_seg.text,
                confidence: whisper_seg.confidence.unwrap_or(0.8),
                language: None,
                segment_type: SegmentType::Word,
            });
        }
        
        result
    }

    /// Update speaker profiles with new segments
    async fn update_speaker_profiles(&self, segments: &[DiarizedSegment]) -> Result<()> {
        let mut profiles = self.speaker_profiles.write().await;
        
        for segment in segments {
            let speaker_id = &segment.speaker_id;
            
            if let Some(profile) = profiles.get_mut(speaker_id) {
                // Update existing profile
                profile.last_seen = Utc::now();
                profile.total_segments += 1;
                profile.total_duration += segment.end_time - segment.start_time;
                profile.confidence = (profile.confidence + segment.confidence) / 2.0;
            } else {
                // Create new profile
                profiles.insert(speaker_id.clone(), SpeakerInfo {
                    id: speaker_id.clone(),
                    name: Some(speaker_id.clone()),
                    first_seen: Utc::now(),
                    last_seen: Utc::now(),
                    total_segments: 1,
                    total_duration: segment.end_time - segment.start_time,
                    confidence: segment.confidence,
                    embedding: Vec::new(), // TODO: Extract speaker embeddings
                });
            }
        }
        
        Ok(())
    }

    /// Get all speaker profiles
    pub async fn get_all_speakers(&self) -> Vec<SpeakerInfo> {
        self.speaker_profiles.read().await.values().cloned().collect()
    }

    /// Update speaker name
    pub async fn update_speaker_name(&self, speaker_id: &str, name: String) -> Result<()> {
        let mut profiles = self.speaker_profiles.write().await;
        if let Some(profile) = profiles.get_mut(speaker_id) {
            profile.name = Some(name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Speaker not found: {}", speaker_id))
        }
    }

    /// Export diarization results
    pub async fn export_diarization(&self, segments: &[DiarizedSegment]) -> Result<serde_json::Value> {
        let speakers = self.get_all_speakers().await;
        let total_duration = segments.iter().map(|s| s.end_time - s.start_time).sum::<f32>();
        let avg_confidence = if segments.is_empty() { 0.0 } else {
            segments.iter().map(|s| s.confidence).sum::<f32>() / segments.len() as f32
        };
        
        Ok(json!({
            "plugin": "advanced_diarization",
            "provider": self.config.provider.to_string(),
            "speakers": speakers,
            "segments": segments,
            "total_speakers": speakers.len(),
            "total_segments": segments.len(),
            "total_duration": total_duration,
            "average_confidence": avg_confidence,
            "languages": segments.iter()
                .filter_map(|s| s.language.as_ref())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        }))
    }

    /// Download Whisper model if not present (existing implementation)
    async fn download_whisper_model(&self) -> Result<PathBuf> {
        let model_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("meeting-assistant")
            .join("whisper-models");
        
        tokio::fs::create_dir_all(&model_dir).await?;
        
        let model_file = model_dir.join(format!("ggml-{}.bin", self.config.whisper_model_size));
        
        if !model_file.exists() {
            tracing::info!("Downloading Whisper model...");
            let download_url = format!(
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
                self.config.whisper_model_size
            );
            
            let response = reqwest::get(&download_url).await?;
            let bytes = response.bytes().await?;
            tokio::fs::write(&model_file, bytes).await?;
        }
        
        Ok(model_file)
    }

    /// Download PyAnnote ONNX model if not present (existing implementation)
    async fn download_pyannote_model(&self) -> Result<PathBuf> {
        let model_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("meeting-assistant")
            .join("pyannote-models");
        
        tokio::fs::create_dir_all(&model_dir).await?;
        
        let model_file = model_dir.join("speaker-diarization.onnx");
        
        if !model_file.exists() {
            tracing::info!("Downloading PyAnnote model...");
            // Note: This is a placeholder. In practice, you'd need to export PyAnnote models to ONNX
            // or use a different approach like calling Python PyAnnote via subprocess
            return Err(anyhow::anyhow!("PyAnnote ONNX model download not implemented yet. Please use Python PyAnnote via subprocess for now."));
        }
        
        Ok(model_file)
    }

    /// Update provider configuration
    pub async fn set_provider(&mut self, provider: DiarizationProvider) -> Result<()> {
        self.config.provider = provider;
        self.initialize_models().await
    }

    /// Get current provider
    pub fn get_provider(&self) -> &DiarizationProvider {
        &self.config.provider
    }

    /// Set ElevenLabs API key
    pub fn set_elevenlabs_api_key(&mut self, api_key: String) {
        self.config.elevenlabs_api_key = Some(api_key);
    }
}

#[async_trait]
impl Plugin for AdvancedDiarizationPlugin {
    fn name(&self) -> &str {
        "advanced_diarization"
    }
    
    fn version(&self) -> &str {
        "2.0.0"
    }
    
    fn description(&self) -> &str {
        "Advanced speaker diarization with support for multiple providers: ElevenLabs Scribe v1 (high quality) and Whisper+PyAnnote (local processing)"
    }
    
    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom configuration if available
        let plugin_data = context.plugin_data.read().await;
        if let Some(data) = plugin_data.get("advanced_diarization") {
            if let Ok(custom_config) = serde_json::from_value::<AdvancedDiarizationConfig>(data.clone()) {
                self.config = custom_config;
            }
        }

        // Initialize models based on provider
        if let Err(e) = self.initialize_models().await {
            tracing::warn!("Failed to initialize models: {}", e);
            match self.config.provider {
                DiarizationProvider::ElevenLabs => {
                    return Err(anyhow::anyhow!("ElevenLabs initialization failed: {}", e));
                }
                DiarizationProvider::WhisperPyAnnote => {
                    tracing::warn!("Falling back to Python subprocess for WhisperPyAnnote");
                }
            }
        }
        
        println!("🎯 Advanced Diarization Plugin initialized");
        match self.config.provider {
            DiarizationProvider::ElevenLabs => {
                println!("   ✅ ElevenLabs Scribe v1 - State-of-the-art accuracy with up to 32 speakers");
                println!("   ✅ Word-level timestamps and speaker identification");
                println!("   ✅ 99 languages supported with audio event detection");
            }
            DiarizationProvider::WhisperPyAnnote => {
                println!("   ✅ OpenAI Whisper for transcription");
                println!("   ✅ PyAnnote for speaker diarization");
                println!("   ✅ Combined pipeline for speaker-attributed transcripts");
            }
        }
        println!("   ✅ Provider: {}", self.config.provider);
        
        Ok(())
    }
    
    async fn cleanup(&mut self, _context: &PluginContext) -> Result<()> {
        println!("🎯 Advanced Diarization Plugin cleaned up");
        Ok(())
    }
    
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        _context: &PluginContext,
    ) -> Result<PluginHookResult> {
        if !self.enabled {
            return Ok(PluginHookResult::Continue);
        }
        
        match event {
            PluginEvent::AudioCaptured { file_path } => {
                tracing::info!("🎯 Processing audio file for Advanced Diarization: {:?}", file_path);
                
                match self.process_audio(file_path).await {
                    Ok(segments) => {
                        // Update speaker profiles
                        if let Err(e) = self.update_speaker_profiles(&segments).await {
                            tracing::warn!("Failed to update speaker profiles: {}", e);
                        }
                        
                        // Export results
                        let result = self.export_diarization(&segments).await?;
                        Ok(PluginHookResult::Replace(result))
                    }
                    Err(e) => {
                        tracing::error!("Advanced Diarization failed: {}", e);
                        Ok(PluginHookResult::Continue)
                    }
                }
            }
            
            PluginEvent::Custom { event_type, data } => {
                match event_type.as_str() {
                    "get_speakers" => {
                        let speakers = self.get_all_speakers().await;
                        Ok(PluginHookResult::Replace(serde_json::to_value(speakers)?))
                    }
                    
                    "update_speaker_name" => {
                        if let (Some(speaker_id), Some(name)) = (
                            data.get("speaker_id").and_then(|v| v.as_str()),
                            data.get("name").and_then(|v| v.as_str())
                        ) {
                            self.update_speaker_name(speaker_id, name.to_string()).await?;
                            Ok(PluginHookResult::Replace(json!({"status": "success"})))
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "get_config" => {
                        Ok(PluginHookResult::Replace(serde_json::to_value(&self.config)?))
                    }
                    
                    "set_config" => {
                        if let Ok(config) = serde_json::from_value::<AdvancedDiarizationConfig>(data.clone()) {
                            self.config = config;
                            // Re-initialize with new config
                            if let Err(e) = self.initialize_models().await {
                                tracing::warn!("Failed to re-initialize with new config: {}", e);
                            }
                            Ok(PluginHookResult::Replace(json!({"status": "config_updated"})))
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "set_provider" => {
                        if let Some(provider_str) = data.get("provider").and_then(|v| v.as_str()) {
                            let provider = match provider_str {
                                "elevenlabs" => DiarizationProvider::ElevenLabs,
                                "whisper_pyannote" => DiarizationProvider::WhisperPyAnnote,
                                _ => return Ok(PluginHookResult::Continue),
                            };
                            
                            match self.set_provider(provider).await {
                                Ok(_) => Ok(PluginHookResult::Replace(json!({"status": "provider_updated"}))),
                                Err(e) => Ok(PluginHookResult::Replace(json!({"status": "error", "message": e.to_string()}))),
                            }
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "get_provider" => {
                        Ok(PluginHookResult::Replace(json!({"provider": self.get_provider().to_string()})))
                    }
                    
                    _ => Ok(PluginHookResult::Continue),
                }
            }
            
            _ => Ok(PluginHookResult::Continue),
        }
    }
    
    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::AudioCaptured { file_path: PathBuf::new() },
            PluginEvent::Custom { 
                event_type: String::new(), 
                data: serde_json::Value::Null 
            },
        ]
    }
    
    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable/disable the Advanced Diarization plugin"
                },
                "provider": {
                    "type": "string",
                    "enum": ["whisper_pyannote", "elevenlabs"],
                    "default": "elevenlabs",
                    "description": "Choose the diarization provider"
                },
                "elevenlabs_api_key": {
                    "type": "string",
                    "description": "ElevenLabs API key (required for ElevenLabs provider)"
                },
                "whisper_model_size": {
                    "type": "string",
                    "enum": ["tiny", "base", "small", "medium", "large"],
                    "default": "base",
                    "description": "Whisper model size (larger = more accurate but slower)"
                },
                "language": {
                    "type": "string",
                    "default": "auto",
                    "description": "Language hint for processing (e.g., 'en', 'auto')"
                },
                "pyannote_model_path": {
                    "type": "string",
                    "default": "pyannote/speaker-diarization-3.1",
                    "description": "PyAnnote model path or HuggingFace model ID"
                },
                "max_speakers": {
                    "type": "integer",
                    "default": 10,
                    "description": "Maximum number of speakers to detect (0 = auto)"
                },
                "speaker_threshold": {
                    "type": "number",
                    "default": 0.7,
                    "description": "Speaker similarity threshold (0.0-1.0)"
                },
                "detect_audio_events": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable audio event detection (laughter, applause, etc.)"
                }
            }
        }))
    }
    
    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        if let Some(enabled) = config.get("enabled") {
            if !enabled.is_boolean() {
                return Err(anyhow::anyhow!("'enabled' must be a boolean"));
            }
        }
        
        if let Some(provider_str) = config.get("provider") {
            if let Some(provider) = provider_str.as_str() {
                let valid_providers = ["whisper_pyannote", "elevenlabs"];
                if !valid_providers.contains(&provider) {
                    return Err(anyhow::anyhow!("'provider' must be one of: {:?}", valid_providers));
                }
            }
        }

        if let Some(elevenlabs_api_key) = config.get("elevenlabs_api_key") {
            if let Some(api_key) = elevenlabs_api_key.as_str() {
                if api_key.is_empty() {
                    return Err(anyhow::anyhow!("'elevenlabs_api_key' cannot be empty"));
                }
            }
        }

        if let Some(model_size) = config.get("whisper_model_size") {
            if let Some(size_str) = model_size.as_str() {
                let valid_sizes = ["tiny", "base", "small", "medium", "large"];
                if !valid_sizes.contains(&size_str) {
                    return Err(anyhow::anyhow!("'whisper_model_size' must be one of: {:?}", valid_sizes));
                }
            }
        }
        
        if let Some(threshold) = config.get("speaker_threshold") {
            if !threshold.is_number() || threshold.as_f64().unwrap_or(0.0) < 0.0 || threshold.as_f64().unwrap_or(0.0) > 1.0 {
                return Err(anyhow::anyhow!("'speaker_threshold' must be between 0.0 and 1.0"));
            }
        }
        
        if let Some(detect_events) = config.get("detect_audio_events") {
            if !detect_events.is_boolean() {
                return Err(anyhow::anyhow!("'detect_audio_events' must be a boolean"));
            }
        }
        
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Helper function to create the plugin instance
pub fn create_advanced_diarization_plugin() -> Box<dyn Plugin> {
    Box::new(AdvancedDiarizationPlugin::new())
} 
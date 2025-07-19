/*
 * Meeting Assistant CLI - Speech-to-Text Post-Processing Plugin
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
use uuid::Uuid;

use crate::plugin_system::*;
use crate::plugins::rust_native_diarization::{SpectralDiarizationPlugin, EnhancedSpeakerProfile, VoiceCharacteristics};
use crate::ai::OpenAIClient;
use crate::system::SystemInfo;

/// Configuration for the STT post-processor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTConfig {
    /// Enable/disable the plugin
    pub enabled: bool,
    /// Auto-process audio files when captured
    pub auto_process: bool,
    /// Minimum confidence threshold for transcription
    pub min_confidence: f32,
    /// Enable diarization (speaker separation)
    pub diarization_enabled: bool,
    /// Output directory for transcripts
    pub output_dir: Option<PathBuf>,
    /// Maximum file age in hours to process
    pub max_file_age_hours: u64,
    /// Preferred transcription backend
    pub transcription_backend: TranscriptionBackend,
    /// Retry failed transcriptions
    pub retry_failed: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for STTConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_process: true,
            min_confidence: 0.7,
            diarization_enabled: true,
            output_dir: None,
            max_file_age_hours: 24,
            transcription_backend: TranscriptionBackend::OpenAI,
            retry_failed: true,
            max_retries: 3,
        }
    }
}

/// Supported transcription backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionBackend {
    OpenAI,
    WhisperCpp,
    Local,
}

/// Transcript with speaker diarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizedTranscript {
    pub id: Uuid,
    pub audio_file: PathBuf,
    pub created_at: DateTime<Utc>,
    pub total_duration: f32,
    pub speakers: Vec<EnhancedSpeakerProfile>,
    pub segments: Vec<TranscriptSegment>,
    pub full_text: String,
    pub confidence: f32,
    pub backend_used: TranscriptionBackend,
}

/// Individual transcript segment with speaker info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_time: f32,
    pub end_time: f32,
    pub speaker_id: String,
    pub speaker_name: Option<String>,
    pub text: String,
    pub confidence: f32,
}

/// Processing status for audio files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStatus {
    pub file_path: PathBuf,
    pub status: ProcessingState,
    pub last_processed: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub transcript_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingState {
    Pending,
    Processing,
    Completed,
    Failed,
    Skipped,
}

/// Speech-to-Text Post-Processing Plugin
pub struct STTPostProcessorPlugin {
    config: STTConfig,
    diarization_plugin: Arc<RwLock<SpectralDiarizationPlugin>>,
    transcripts: Arc<RwLock<HashMap<Uuid, DiarizedTranscript>>>,
    processing_status: Arc<RwLock<HashMap<PathBuf, ProcessingStatus>>>,
    audio_files: Arc<RwLock<Vec<PathBuf>>>,
    enabled: bool,
    // References to transcription services
    system_info: Option<Arc<SystemInfo>>,
    openai_client: Option<Arc<OpenAIClient>>,
    plugin_manager: Option<Arc<PluginManager>>,
}

impl STTPostProcessorPlugin {
    pub fn new() -> Self {
        Self {
            config: STTConfig::default(),
            diarization_plugin: Arc::new(RwLock::new(SpectralDiarizationPlugin::new())),
            transcripts: Arc::new(RwLock::new(HashMap::new())),
            processing_status: Arc::new(RwLock::new(HashMap::new())),
            audio_files: Arc::new(RwLock::new(Vec::new())),
            enabled: true,
            system_info: None,
            openai_client: None,
            plugin_manager: None,
        }
    }

    /// Set references to transcription services
    pub fn set_transcription_services(
        &mut self,
        system_info: Arc<SystemInfo>,
        openai_client: Arc<OpenAIClient>,
        plugin_manager: Arc<PluginManager>,
    ) {
        self.system_info = Some(system_info);
        self.openai_client = Some(openai_client);
        self.plugin_manager = Some(plugin_manager);
    }

    /// Enable downcasting for this plugin type
    pub fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// Process a single audio file for STT + diarization
    pub async fn process_audio_file(&self, audio_file: &Path) -> Result<DiarizedTranscript> {
        tracing::info!("Processing audio file for STT: {:?}", audio_file);

        // Update processing status
        {
            let mut status = self.processing_status.write().await;
            status.insert(audio_file.to_path_buf(), ProcessingStatus {
                file_path: audio_file.to_path_buf(),
                status: ProcessingState::Processing,
                last_processed: Some(Utc::now()),
                error: None,
                transcript_id: None,
            });
        }

        // Step 1: Transcribe audio
        let transcript_result = self.transcribe_audio(audio_file).await;
        
        let (transcript_text, confidence) = match transcript_result {
            Ok((text, conf)) => (text, conf),
            Err(e) => {
                tracing::error!("Transcription failed for {:?}: {}", audio_file, e);
                
                // Update status to failed
                {
                    let mut status = self.processing_status.write().await;
                    if let Some(entry) = status.get_mut(audio_file) {
                        entry.status = ProcessingState::Failed;
                        entry.error = Some(e.to_string());
                    }
                }
                
                if self.config.retry_failed {
                    // TODO: Implement retry logic
                    tracing::info!("Retry logic not yet implemented");
                }
                
                return Err(e);
            }
        };

        // Step 2: Perform diarization if enabled
        let segments = if self.config.diarization_enabled {
            println!("🎤 Diarization is enabled, starting diarization process...");
            match self.diarize_transcript(&transcript_text, audio_file).await {
                Ok(segments) => {
                    println!("🎤 Diarization completed successfully with {} segments", segments.len());
                    segments
                }
                Err(e) => {
                    println!("🎤 Diarization failed: {}, falling back to single speaker", e);
                    vec![TranscriptSegment {
                        start_time: 0.0,
                        end_time: self.get_audio_duration(audio_file).await.unwrap_or(0.0),
                        speaker_id: "Speaker 1".to_string(),
                        speaker_name: Some("Speaker 1".to_string()),
                        text: transcript_text.clone(),
                        confidence,
                    }]
                }
            }
        } else {
            println!("🎤 Diarization is disabled, using single speaker");
            // Single speaker segment
            vec![TranscriptSegment {
                start_time: 0.0,
                end_time: self.get_audio_duration(audio_file).await.unwrap_or(0.0),
                speaker_id: "Speaker 1".to_string(),
                speaker_name: Some("Speaker 1".to_string()),
                text: transcript_text.clone(),
                confidence,
            }]
        };

        // Step 3: Create diarized transcript
        let transcript_id = Uuid::new_v4();
        let speakers = self.get_speakers_from_segments(&segments).await;
        
        let diarized_transcript = DiarizedTranscript {
            id: transcript_id,
            audio_file: audio_file.to_path_buf(),
            created_at: Utc::now(),
            total_duration: self.get_audio_duration(audio_file).await.unwrap_or(0.0),
            speakers,
            segments,
            full_text: transcript_text,
            confidence,
            backend_used: self.config.transcription_backend.clone(),
        };

        // Step 4: Store transcript
        {
            let mut transcripts = self.transcripts.write().await;
            transcripts.insert(transcript_id, diarized_transcript.clone());
        }

        // Step 5: Save to file if output directory is specified
        if let Some(output_dir) = &self.config.output_dir {
            self.save_transcript_to_file(&diarized_transcript, output_dir).await?;
        }

        // Step 6: Update processing status to completed
        {
            let mut status = self.processing_status.write().await;
            status.insert(audio_file.to_path_buf(), ProcessingStatus {
                file_path: audio_file.to_path_buf(),
                status: ProcessingState::Completed,
                last_processed: Some(Utc::now()),
                error: None,
                transcript_id: Some(transcript_id),
            });
        }

        tracing::info!("Successfully processed audio file: {:?}", audio_file);
        Ok(diarized_transcript)
    }

    /// Transcribe audio using the configured backend with fallback logic
    async fn transcribe_audio(&self, audio_file: &Path) -> Result<(String, f32)> {
        let audio_path = audio_file.to_path_buf();
        
        // Use the same fallback logic as the main application
        match self.config.transcription_backend {
            TranscriptionBackend::Local => {
                // Try local transcription first
                if let Some(system_info) = &self.system_info {
                    tracing::info!("Using local backend for transcription");
                    match system_info.transcribe_audio(&audio_path).await {
                        Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                            tracing::info!("Local transcription successful");
                            return Ok((transcript, 0.8));
                        }
                        Ok(Some(_)) | Ok(None) => {
                            tracing::info!("Local transcription returned empty, trying plugin system");
                        }
                        Err(e) => {
                            tracing::warn!("Local transcription failed: {}", e);
                        }
                    }
                }
                
                // Fallback to plugin system
                if let Some(plugin_manager) = &self.plugin_manager {
                    match plugin_manager.transcribe_audio(&audio_path).await {
                        Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                            tracing::info!("Plugin transcription successful");
                            return Ok((transcript, 0.8));
                        }
                        Ok(Some(_)) | Ok(None) => {
                            tracing::info!("Plugin transcription returned empty");
                        }
                        Err(e) => {
                            tracing::warn!("Plugin transcription failed: {}", e);
                        }
                    }
                }
                
                // Final fallback to OpenAI
                if let Some(openai_client) = &self.openai_client {
                    tracing::info!("Using OpenAI fallback for transcription");
                    match openai_client.transcribe_audio(&audio_path).await {
                        Ok(transcript) => Ok((transcript, 0.9)),
                        Err(e) => Err(anyhow::anyhow!("All transcription methods failed: {}", e))
                    }
                } else {
                    Err(anyhow::anyhow!("No transcription services available"))
                }
            }
            
            TranscriptionBackend::WhisperCpp => {
                // Try local whisper.cpp first
                if let Some(system_info) = &self.system_info {
                    tracing::info!("Using whisper.cpp backend for transcription");
                    match system_info.transcribe_audio(&audio_path).await {
                        Ok(Some(transcript)) if !transcript.trim().is_empty() => {
                            tracing::info!("Whisper.cpp transcription successful");
                            return Ok((transcript, 0.8));
                        }
                        Ok(Some(_)) | Ok(None) => {
                            tracing::info!("Whisper.cpp transcription returned empty");
                        }
                        Err(e) => {
                            tracing::warn!("Whisper.cpp transcription failed: {}", e);
                        }
                    }
                }
                
                // Fallback to OpenAI
                if let Some(openai_client) = &self.openai_client {
                    tracing::info!("Whisper.cpp failed, using OpenAI fallback");
                    match openai_client.transcribe_audio(&audio_path).await {
                        Ok(transcript) => Ok((transcript, 0.9)),
                        Err(e) => Err(anyhow::anyhow!("Whisper.cpp and OpenAI transcription failed: {}", e))
                    }
                } else {
                    Err(anyhow::anyhow!("Whisper.cpp transcription failed and no OpenAI client available"))
                }
            }
            
            TranscriptionBackend::OpenAI => {
                // Use OpenAI directly
                if let Some(openai_client) = &self.openai_client {
                    tracing::info!("Using OpenAI backend for transcription");
                    match openai_client.transcribe_audio(&audio_path).await {
                        Ok(transcript) => Ok((transcript, 0.9)),
                        Err(e) => Err(anyhow::anyhow!("OpenAI transcription failed: {}", e))
                    }
                } else {
                    Err(anyhow::anyhow!("OpenAI client not available"))
                }
            }
        }
    }

    /// Diarize transcript using the embedded diarization plugin
    async fn diarize_transcript(&self, transcript: &str, audio_file: &Path) -> Result<Vec<TranscriptSegment>> {
        tracing::info!("🎤 Starting diarization for audio file: {:?}", audio_file);
        
        // Get audio segments from the diarization plugin
        let segments = {
            let diarization = self.diarization_plugin.read().await;
            diarization.process_audio_file(audio_file).await?
        };

        tracing::info!("🎤 Diarization returned {} segments", segments.len());
        
        // For now, create simple segments based on speaker changes
        // In a full implementation, this would align the transcript with audio segments
        if segments.is_empty() {
            tracing::warn!("🎤 No diarization segments found, falling back to single speaker");
            // No diarization possible, return single segment
            return Ok(vec![TranscriptSegment {
                start_time: 0.0,
                end_time: self.get_audio_duration(audio_file).await.unwrap_or(0.0),
                speaker_id: "Speaker 1".to_string(),
                speaker_name: Some("Speaker 1".to_string()),
                text: transcript.to_string(),
                confidence: 0.8,
            }]);
        }
        
        tracing::info!("🎤 Processing {} diarization segments", segments.len());
        
        // Now we need to map the diarization segments to the transcript text
        // This is a simplified approach - in production, we'd use forced alignment
        let mut transcript_segments = Vec::new();
        
        // Calculate total transcript length for proportional mapping
        let total_duration = segments.iter().map(|s| s.end_time - s.start_time).sum::<f64>();
        let mut current_time = 0.0f64;
        
        for (_i, segment) in segments.iter().enumerate() {
            let segment_duration = segment.end_time - segment.start_time;
            let proportion = segment_duration / total_duration;
            
            // Estimate how much of the transcript belongs to this segment
            let segment_text_length = (transcript.len() as f64 * proportion) as usize;
            let start_idx = (current_time / total_duration * transcript.len() as f64) as usize;
            let end_idx = (start_idx + segment_text_length).min(transcript.len());
            
            let segment_text = if start_idx < transcript.len() {
                transcript[start_idx..end_idx].to_string()
            } else {
                "".to_string()
            };
            
            if !segment_text.trim().is_empty() {
                let speaker_id = segment.speaker_id.clone();
                
                transcript_segments.push(TranscriptSegment {
                    start_time: segment.start_time as f32,
                    end_time: segment.end_time as f32,
                    speaker_id: speaker_id.clone(),
                    speaker_name: Some(speaker_id),
                    text: segment_text.trim().to_string(),
                    confidence: segment.confidence as f32,
                });
            }
            
            current_time += segment_duration;
        }
        
        if transcript_segments.is_empty() {
            tracing::warn!("🎤 No valid segments created from diarization, falling back to single speaker");
            return Ok(vec![TranscriptSegment {
                start_time: 0.0,
                end_time: self.get_audio_duration(audio_file).await.unwrap_or(0.0),
                speaker_id: "Speaker 1".to_string(),
                speaker_name: Some("Speaker 1".to_string()),
                text: transcript.to_string(),
                confidence: 0.8,
            }]);
        }
        
        tracing::info!("🎤 Created {} transcript segments from diarization", transcript_segments.len());
        Ok(transcript_segments)
    }

    /// Get audio duration (placeholder implementation)
    async fn get_audio_duration(&self, _audio_file: &Path) -> Result<f32> {
        // In a real implementation, this would analyze the audio file
        // For now, return a placeholder duration
        Ok(30.0)
    }

    /// Extract unique speakers from segments
    async fn get_speakers_from_segments(&self, segments: &[TranscriptSegment]) -> Vec<EnhancedSpeakerProfile> {
        let mut speakers = HashMap::new();
        
        for segment in segments {
            let speaker_id = segment.speaker_id.clone();
            
            if let Some(speaker) = speakers.get_mut(&speaker_id) {
                // Update existing speaker
                let profile: &mut EnhancedSpeakerProfile = speaker;
                profile.total_segments += 1;
                profile.total_duration += (segment.end_time - segment.start_time) as f64;
                profile.last_seen = Utc::now();
            } else {
                // Create new speaker
                speakers.insert(speaker_id.clone(), EnhancedSpeakerProfile {
                    id: speaker_id.clone(),
                    name: segment.speaker_name.clone(),
                    first_seen: Utc::now(),
                    last_seen: Utc::now(),
                    total_segments: 1,
                    total_duration: (segment.end_time - segment.start_time) as f64,
                    embedding: Vec::new(),
                    confidence: segment.confidence as f64,
                    voice_characteristics: VoiceCharacteristics::default(),
                });
            }
        }
        
        speakers.into_values().collect()
    }

    /// Save transcript to file
    async fn save_transcript_to_file(&self, transcript: &DiarizedTranscript, output_dir: &Path) -> Result<()> {
        tokio::fs::create_dir_all(output_dir).await?;
        
        let filename = format!("transcript_{}.json", transcript.id);
        let filepath = output_dir.join(filename);
        
        let json_content = serde_json::to_string_pretty(transcript)?;
        tokio::fs::write(&filepath, json_content).await?;
        
        tracing::info!("Saved transcript to: {:?}", filepath);
        Ok(())
    }

    /// List all available audio files for processing
    pub async fn list_audio_files(&self) -> Vec<PathBuf> {
        self.audio_files.read().await.clone()
    }

    /// Get processing status for a file
    pub async fn get_processing_status(&self, audio_file: &Path) -> Option<ProcessingStatus> {
        self.processing_status.read().await.get(audio_file).cloned()
    }

    /// Get all transcripts
    pub async fn get_all_transcripts(&self) -> Vec<DiarizedTranscript> {
        self.transcripts.read().await.values().cloned().collect()
    }

    /// Get transcript by ID
    pub async fn get_transcript(&self, transcript_id: &Uuid) -> Option<DiarizedTranscript> {
        self.transcripts.read().await.get(transcript_id).cloned()
    }

    /// Reprocess all audio files
    pub async fn reprocess_all_files(&self) -> Result<Vec<DiarizedTranscript>> {
        let audio_files = self.list_audio_files().await;
        let mut results = Vec::new();
        
        for audio_file in audio_files {
            match self.process_audio_file(&audio_file).await {
                Ok(transcript) => {
                    results.push(transcript);
                    println!("✅ Processed: {:?}", audio_file);
                }
                Err(e) => {
                    println!("❌ Failed to process {:?}: {}", audio_file, e);
                }
            }
        }
        
        Ok(results)
    }

    /// Format transcript for display
    pub fn format_transcript(&self, transcript: &DiarizedTranscript) -> String {
        let mut formatted = format!("📄 Transcript: {}\n", transcript.id);
        formatted.push_str(&format!("📁 Audio File: {:?}\n", transcript.audio_file));
        formatted.push_str(&format!("⏱️  Duration: {:.1}s\n", transcript.total_duration));
        formatted.push_str(&format!("🎯 Confidence: {:.2}\n", transcript.confidence));
        formatted.push_str(&format!("👥 Speakers: {}\n", transcript.speakers.len()));
        formatted.push_str(&format!("📅 Created: {}\n", transcript.created_at.format("%Y-%m-%d %H:%M:%S")));
        formatted.push_str("\n📝 Transcript:\n");
        formatted.push_str(&"-".repeat(50));
        formatted.push('\n');
        
        for segment in &transcript.segments {
            let speaker_name = segment.speaker_name.as_ref().unwrap_or(&segment.speaker_id);
            formatted.push_str(&format!("[{:.1}s - {:.1}s] {}: {}\n", 
                segment.start_time, segment.end_time, speaker_name, segment.text));
        }
        
        formatted
    }
}

#[async_trait]
impl Plugin for STTPostProcessorPlugin {
    fn name(&self) -> &str {
        "stt_post_processor"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Post-processing plugin for Speech-to-Text with speaker diarization"
    }
    
    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom configuration if available
        let plugin_data = context.plugin_data.read().await;
        if let Some(data) = plugin_data.get("stt_post_processor") {
            if let Ok(custom_config) = serde_json::from_value::<STTConfig>(data.clone()) {
                self.config = custom_config;
            }
        }

        // Set up output directory if not specified
        if self.config.output_dir.is_none() {
            let default_output = dirs::home_dir()
                .context("Failed to get home directory")?
                .join(".meeting-assistant")
                .join("transcripts");
            self.config.output_dir = Some(default_output);
        }

        // Initialize the diarization plugin
        {
            let mut diarization = self.diarization_plugin.write().await;
            diarization.initialize(context).await?;
        }

        // Note: The transcription services (system_info, openai_client, plugin_manager) 
        // will be set separately via set_transcription_services() since they're not 
        // available in the PluginContext

        println!("🎙️  STT Post-Processor Plugin initialized");
        println!("   ✅ Speech-to-Text processing enabled");
        println!("   ✅ Speaker diarization: {}", if self.config.diarization_enabled { "enabled" } else { "disabled" });
        println!("   ✅ Auto-processing: {}", if self.config.auto_process { "enabled" } else { "disabled" });
        println!("   ✅ Backend: {:?}", self.config.transcription_backend);
        if let Some(output_dir) = &self.config.output_dir {
            println!("   ✅ Output directory: {:?}", output_dir);
        }
        
        Ok(())
    }
    
    async fn cleanup(&mut self, context: &PluginContext) -> Result<()> {
        // Clean up the diarization plugin
        {
            let mut diarization = self.diarization_plugin.write().await;
            diarization.cleanup(context).await?;
        }

        println!("🎙️  STT Post-Processor Plugin cleaned up");
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
                tracing::info!("🎙️  Audio captured, processing for STT: {:?}", file_path);

                // Add to audio files list
                {
                    let mut audio_files = self.audio_files.write().await;
                    if !audio_files.contains(file_path) {
                        audio_files.push(file_path.clone());
                    }
                }

                // Process immediately if auto-processing is enabled
                if self.config.auto_process {
                    match self.process_audio_file(file_path).await {
                        Ok(transcript) => {
                            tracing::info!("✅ Successfully processed audio file: {:?}", file_path);
                            
                            // Return the transcript as a result
                            let result = json!({
                                "transcript_id": transcript.id,
                                "speakers": transcript.speakers.len(),
                                "segments": transcript.segments.len(),
                                "confidence": transcript.confidence,
                                "full_text": transcript.full_text,
                                "file_path": file_path
                            });
                            
                            Ok(PluginHookResult::Replace(result))
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to process audio file {:?}: {}", file_path, e);
                            Ok(PluginHookResult::Continue)
                        }
                    }
                } else {
                    // Just add to queue for later processing
                    Ok(PluginHookResult::Continue)
                }
            }
            
            PluginEvent::AudioRecordingCompleted { file_path, duration_seconds } => {
                tracing::info!("🎙️  Audio recording completed, processing for STT: {:?} ({}s)", file_path, duration_seconds);

                // Add to audio files list
                {
                    let mut audio_files = self.audio_files.write().await;
                    if !audio_files.contains(file_path) {
                        audio_files.push(file_path.clone());
                    }
                }

                // Process recording completion automatically (this is the primary completion hook)
                match self.process_audio_file(file_path).await {
                    Ok(transcript) => {
                        tracing::info!("✅ Successfully processed completed recording: {:?}", file_path);
                        
                        // Format and display the transcript
                        let formatted_transcript = self.format_transcript(&transcript);
                        println!("\n{}", formatted_transcript);
                        
                        // Return the transcript as a result
                        let result = json!({
                            "transcript_id": transcript.id,
                            "speakers": transcript.speakers.len(),
                            "segments": transcript.segments.len(),
                            "confidence": transcript.confidence,
                            "full_text": transcript.full_text,
                            "file_path": file_path,
                            "duration_seconds": duration_seconds
                        });
                        
                        Ok(PluginHookResult::Replace(result))
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to process completed recording {:?}: {}", file_path, e);
                        Ok(PluginHookResult::Continue)
                    }
                }
            }
            
            PluginEvent::Custom { event_type, data } => {
                match event_type.as_str() {
                    "list_audio_files" => {
                        let files = self.list_audio_files().await;
                        Ok(PluginHookResult::Replace(serde_json::to_value(files)?))
                    }
                    
                    "get_transcripts" => {
                        let transcripts = self.get_all_transcripts().await;
                        Ok(PluginHookResult::Replace(serde_json::to_value(transcripts)?))
                    }
                    
                    "get_transcript" => {
                        if let Some(transcript_id) = data.get("transcript_id").and_then(|v| v.as_str()) {
                            if let Ok(uuid) = Uuid::parse_str(transcript_id) {
                                let transcript = self.get_transcript(&uuid).await;
                                Ok(PluginHookResult::Replace(serde_json::to_value(transcript)?))
                            } else {
                                Ok(PluginHookResult::Continue)
                            }
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "reprocess_all" => {
                        match self.reprocess_all_files().await {
                            Ok(transcripts) => {
                                Ok(PluginHookResult::Replace(serde_json::to_value(transcripts)?))
                            }
                            Err(e) => {
                                let result = json!({
                                    "error": e.to_string()
                                });
                                Ok(PluginHookResult::Replace(result))
                            }
                        }
                    }
                    
                    "process_file" => {
                        if let Some(file_path) = data.get("file_path").and_then(|v| v.as_str()) {
                            let path = PathBuf::from(file_path);
                            match self.process_audio_file(&path).await {
                                Ok(transcript) => {
                                    Ok(PluginHookResult::Replace(serde_json::to_value(transcript)?))
                                }
                                Err(e) => {
                                    let result = json!({
                                        "error": e.to_string()
                                    });
                                    Ok(PluginHookResult::Replace(result))
                                }
                            }
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "get_processing_status" => {
                        if let Some(file_path) = data.get("file_path").and_then(|v| v.as_str()) {
                            let path = PathBuf::from(file_path);
                            let status = self.get_processing_status(&path).await;
                            Ok(PluginHookResult::Replace(serde_json::to_value(status)?))
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "get_config" => {
                        Ok(PluginHookResult::Replace(serde_json::to_value(&self.config)?))
                    }
                    
                    "set_config" => {
                        if let Ok(config) = serde_json::from_value::<STTConfig>(data.clone()) {
                            self.config = config;
                            Ok(PluginHookResult::Replace(json!({"status": "config_updated"})))
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
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
            PluginEvent::AudioRecordingCompleted { file_path: PathBuf::new(), duration_seconds: 0.0 },
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
                    "description": "Enable/disable the STT post-processor"
                },
                "auto_process": {
                    "type": "boolean",
                    "default": true,
                    "description": "Automatically process audio files when captured"
                },
                "min_confidence": {
                    "type": "number",
                    "default": 0.7,
                    "description": "Minimum confidence threshold for transcription"
                },
                "diarization_enabled": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable speaker diarization"
                },
                "max_file_age_hours": {
                    "type": "integer",
                    "default": 24,
                    "description": "Maximum file age in hours to process"
                },
                "retry_failed": {
                    "type": "boolean",
                    "default": true,
                    "description": "Retry failed transcriptions"
                },
                "max_retries": {
                    "type": "integer",
                    "default": 3,
                    "description": "Maximum retry attempts"
                }
            }
        }))
    }
    
    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        // Validate configuration
        if let Some(enabled) = config.get("enabled") {
            if !enabled.is_boolean() {
                return Err(anyhow::anyhow!("'enabled' must be a boolean"));
            }
        }
        
        if let Some(auto_process) = config.get("auto_process") {
            if !auto_process.is_boolean() {
                return Err(anyhow::anyhow!("'auto_process' must be a boolean"));
            }
        }
        
        if let Some(confidence) = config.get("min_confidence") {
            if !confidence.is_number() || confidence.as_f64().unwrap_or(0.0) < 0.0 || confidence.as_f64().unwrap_or(0.0) > 1.0 {
                return Err(anyhow::anyhow!("'min_confidence' must be a number between 0.0 and 1.0"));
            }
        }
        
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
} 
/*
 * Meeting Assistant CLI - Continuous Meeting Types
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

use anyhow::Result;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Core data flowing through the processing pipeline
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub id: Uuid,
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: DateTime<Utc>,
    pub duration: ChronoDuration,
    pub sequence_number: u64,
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub id: Uuid,
    pub audio_chunk_id: Uuid,
    pub text: String,
    pub confidence: f32,
    pub start_time: DateTime<Utc>,
    pub duration: ChronoDuration,
    pub language: String,
    pub word_timestamps: Option<Vec<WordTimestamp>>,
}

#[derive(Debug, Clone)]
pub struct WordTimestamp {
    pub word: String,
    pub start: f32,
    pub end: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct DiarizedSegment {
    pub id: Uuid,
    pub transcript_id: Uuid,
    pub text: String,
    pub speaker_id: String,
    pub speaker_confidence: f32,
    pub start_time: DateTime<Utc>,
    pub duration: ChronoDuration,
    pub is_speaker_change: bool,
    pub voice_features: Option<VoiceFeatures>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceFeatures {
    pub pitch_mean: f32,
    pub pitch_std: f32,
    pub energy_mean: f32,
    pub spectral_centroid: f32,
    pub mfcc: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct VectorizedSegment {
    pub id: Uuid,
    pub diarized_id: Uuid,
    pub text: String,
    pub speaker_id: String,
    pub embedding: Vec<f32>,
    pub start_time: DateTime<Utc>,
    pub duration: ChronoDuration,
    pub metadata: SegmentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentMetadata {
    pub word_count: u32,
    pub sentiment: Option<String>,
    pub key_phrases: Vec<String>,
    pub confidence_scores: ConfidenceScores,
    pub topics: Vec<String>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScores {
    pub transcription: f32,
    pub speaker_identification: f32,
    pub embedding_quality: f32,
    pub overall: f32,
}

/// Speaker management and identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerProfile {
    pub id: String,
    pub name: Option<String>,
    pub voice_features: VoiceFeatures,
    pub first_appearance: DateTime<Utc>,
    pub last_appearance: DateTime<Utc>,
    pub total_speaking_time: ChronoDuration,
    pub utterance_count: u64,
    pub confidence_history: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SpeakerRegistry {
    pub profiles: HashMap<String, SpeakerProfile>,
    pub unknown_speakers: VecDeque<String>,
    pub speaker_aliases: HashMap<String, String>, // alias -> canonical_id
}

impl SpeakerRegistry {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            unknown_speakers: VecDeque::new(),
            speaker_aliases: HashMap::new(),
        }
    }

    pub fn add_speaker(&mut self, profile: SpeakerProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }

    pub fn identify_speaker(&mut self, speaker_id: &str, name: &str) {
        if let Some(profile) = self.profiles.get_mut(speaker_id) {
            profile.name = Some(name.to_string());
        }
    }

    pub fn merge_speakers(&mut self, from_id: &str, to_id: &str) -> Result<()> {
        if let Some(from_profile) = self.profiles.remove(from_id) {
            if let Some(to_profile) = self.profiles.get_mut(to_id) {
                // Merge statistics
                to_profile.total_speaking_time += from_profile.total_speaking_time;
                to_profile.utterance_count += from_profile.utterance_count;
                to_profile.confidence_history.extend(from_profile.confidence_history);
                
                // Update aliases
                self.speaker_aliases.insert(from_id.to_string(), to_id.to_string());
            }
        }
        Ok(())
    }
}

/// Meeting session management
#[derive(Debug, Clone)]
pub struct MeetingSession {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
    pub title: Option<String>,
    pub participants: Vec<String>,
    pub status: MeetingStatus,
    pub statistics: MeetingStatistics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MeetingStatus {
    Starting,
    Recording,
    Paused,
    Stopping,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingStatistics {
    pub total_duration: ChronoDuration,
    pub speaking_time_by_speaker: HashMap<String, ChronoDuration>,
    pub total_words: u64,
    pub total_segments: u64,
    pub average_confidence: f32,
    pub speaker_count: u32,
}

/// Configuration for the continuous meeting system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousMeetingConfig {
    // Audio settings
    pub audio_chunk_duration: f32,
    pub audio_overlap: f32,
    pub sample_rate: u32,
    pub channels: u16,
    
    // Processing settings
    pub transcription_confidence_threshold: f32,
    pub speaker_change_threshold: f32,
    pub embedding_batch_size: usize,
    pub database_batch_size: usize,
    
    // Performance settings
    pub max_processing_queue_size: usize,
    pub transcription_timeout: f32,
    pub embedding_timeout: f32,
    
    // Storage settings
    pub database_path: String,
    pub audio_retention_hours: u64,
    pub backup_interval_minutes: u64,
    
    // Privacy settings
    pub auto_start_recording: bool,
    pub save_raw_audio: bool,
    pub speaker_anonymization: bool,
}

impl Default for ContinuousMeetingConfig {
    fn default() -> Self {
        Self {
            // Audio settings
            audio_chunk_duration: 3.0,
            audio_overlap: 0.5,
            sample_rate: 16000,
            channels: 1,
            
            // Processing settings
            transcription_confidence_threshold: 0.7,
            speaker_change_threshold: 0.8,
            embedding_batch_size: 10,
            database_batch_size: 50,
            
            // Performance settings
            max_processing_queue_size: 100,
            transcription_timeout: 30.0,
            embedding_timeout: 10.0,
            
            // Storage settings
            database_path: "~/.meeting-assistant/continuous.db".to_string(),
            audio_retention_hours: 24,
            backup_interval_minutes: 60,
            
            // Privacy settings
            auto_start_recording: false,
            save_raw_audio: false,
            speaker_anonymization: false,
        }
    }
}

/// Processing queue for managing workflow
#[derive(Debug)]
pub struct ProcessingQueue {
    pub audio_chunks: Arc<RwLock<VecDeque<AudioChunk>>>,
    pub transcription_queue: Arc<RwLock<VecDeque<TranscriptSegment>>>,
    pub diarization_queue: Arc<RwLock<VecDeque<DiarizedSegment>>>,
    pub vectorization_queue: Arc<RwLock<VecDeque<VectorizedSegment>>>,
    pub max_size: usize,
}

impl ProcessingQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            audio_chunks: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            transcription_queue: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            diarization_queue: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            vectorization_queue: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    pub async fn queue_sizes(&self) -> (usize, usize, usize, usize) {
        let audio = self.audio_chunks.read().await.len();
        let transcription = self.transcription_queue.read().await.len();
        let diarization = self.diarization_queue.read().await.len();
        let vectorization = self.vectorization_queue.read().await.len();
        (audio, transcription, diarization, vectorization)
    }

    pub async fn is_backlogged(&self) -> bool {
        let (audio, transcription, diarization, vectorization) = self.queue_sizes().await;
        let total = audio + transcription + diarization + vectorization;
        total > self.max_size / 2
    }
}

/// Channel types for inter-component communication
pub type AudioChunkSender = mpsc::UnboundedSender<AudioChunk>;
pub type AudioChunkReceiver = mpsc::UnboundedReceiver<AudioChunk>;

pub type TranscriptSender = mpsc::UnboundedSender<TranscriptSegment>;
pub type TranscriptReceiver = mpsc::UnboundedReceiver<TranscriptSegment>;

pub type DiarizedSender = mpsc::UnboundedSender<DiarizedSegment>;
pub type DiarizedReceiver = mpsc::UnboundedReceiver<DiarizedSegment>;

pub type VectorizedSender = mpsc::UnboundedSender<VectorizedSegment>;
pub type VectorizedReceiver = mpsc::UnboundedReceiver<VectorizedSegment>;

/// Error types for the continuous system
#[derive(Debug, thiserror::Error)]
pub enum ContinuousError {
    #[error("Audio capture error: {0}")]
    AudioCapture(String),
    
    #[error("Transcription error: {0}")]
    Transcription(String),
    
    #[error("Speaker diarization error: {0}")]
    Diarization(String),
    
    #[error("Vectorization error: {0}")]
    Vectorization(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Pipeline overload: {0}")]
    PipelineOverload(String),
    
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),
}

/// Status and monitoring types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub recording_status: MeetingStatus,
    pub pipeline_health: PipelineHealth,
    pub resource_usage: ResourceUsage,
    pub queue_status: QueueStatus,
    pub error_count: ErrorCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineHealth {
    pub audio_capture: HealthStatus,
    pub transcription: HealthStatus,
    pub diarization: HealthStatus,
    pub vectorization: HealthStatus,
    pub storage: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning { message: String },
    Error { message: String },
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub disk_usage_mb: u64,
    pub network_requests_per_minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub audio_queue_size: usize,
    pub transcription_queue_size: usize,
    pub diarization_queue_size: usize,
    pub vectorization_queue_size: usize,
    pub total_backlog: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCounts {
    pub audio_errors: u32,
    pub transcription_errors: u32,
    pub diarization_errors: u32,
    pub vectorization_errors: u32,
    pub storage_errors: u32,
    pub total_errors: u32,
} 
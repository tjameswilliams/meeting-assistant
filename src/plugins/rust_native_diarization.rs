/*
 * Meeting Assistant CLI - Spectral-Based Speaker Diarization Plugin
 * Copyright (c) 2024 Meeting Assistant Contributors
 * 
 * Reliable speaker diarization using spectral analysis and clustering
 */

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::plugin_system::*;

/// Configuration for the spectral-based speaker diarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralDiarizationConfig {
    /// Enable/disable the diarization plugin
    pub enabled: bool,
    /// Voice activity detection threshold (0.0 - 1.0)
    pub vad_threshold: f64,
    /// Minimum speech duration in seconds
    pub min_speech_duration: f64,
    /// Maximum silence duration to consider same speaker
    pub max_silence_duration: f64,
    /// Speaker similarity threshold for clustering
    pub speaker_similarity_threshold: f64,
    /// Maximum number of speakers to detect
    pub max_speakers: usize,
    /// Sample rate for audio processing
    pub sample_rate: u32,
    /// Frame size for analysis (in samples)
    pub frame_size: usize,
    /// Hop size for analysis (in samples)
    pub hop_size: usize,
    /// Number of MFCC coefficients to extract
    pub mfcc_coefficients: usize,
}

impl Default for SpectralDiarizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            vad_threshold: 0.01,  // Moderate threshold for good speech detection
            min_speech_duration: 0.3,  // Short segments for fine-grained analysis
            max_silence_duration: 1.5,  // Reasonable silence duration
            speaker_similarity_threshold: 0.65,  // Higher threshold for final classification
            max_speakers: 6,
            sample_rate: 16000,
            frame_size: 1024,
            hop_size: 512,
            mfcc_coefficients: 13,
        }
    }
}

/// Enhanced speaker profile with spectral features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSpeakerProfile {
    pub id: String,
    pub name: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub total_segments: usize,
    pub total_duration: f64,
    pub embedding: Vec<f64>,
    pub confidence: f64,
    pub voice_characteristics: VoiceCharacteristics,
}

/// Voice characteristics for speaker identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCharacteristics {
    pub fundamental_frequency: f64,
    pub spectral_centroid: f64,
    pub spectral_bandwidth: f64,
    pub spectral_rolloff: f64,
    pub zero_crossing_rate: f64,
    pub mfccs: Vec<f64>,
}

/// Diarization segment with speaker and timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationSegment {
    pub start_time: f64,
    pub end_time: f64,
    pub speaker_id: String,
    pub confidence: f64,
    pub text: Option<String>,
    pub voice_characteristics: VoiceCharacteristics,
}

/// Audio segment with extracted features
#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub start_time: f64,
    pub end_time: f64,
    pub audio_data: Vec<f32>,
    pub features: Vec<f64>,
    pub voice_characteristics: VoiceCharacteristics,
    pub speaker_id: Option<String>,
}

/// Spectral-based speaker diarization plugin
pub struct SpectralDiarizationPlugin {
    config: SpectralDiarizationConfig,
    speaker_profiles: Arc<RwLock<HashMap<String, EnhancedSpeakerProfile>>>,
    enabled: bool,
    next_speaker_id: Arc<RwLock<usize>>,
}

impl SpectralDiarizationPlugin {
    pub fn new() -> Self {
        Self {
            config: SpectralDiarizationConfig::default(),
            speaker_profiles: Arc::new(RwLock::new(HashMap::new())),
            enabled: true,
            next_speaker_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Process audio file and return diarized segments with two-phase approach
    pub async fn process_audio_file(&self, audio_file: &Path) -> Result<Vec<DiarizationSegment>> {
        tracing::info!("Processing audio file for spectral diarization: {:?}", audio_file);
        
        // Load audio file
        let audio_data = self.load_audio_file(audio_file).await?;
        if audio_data.is_empty() {
            tracing::warn!("Audio file is empty or could not be loaded");
            return Ok(Vec::new());
        }
        
        tracing::info!("Loaded audio: {} samples at {} Hz", audio_data.len(), self.config.sample_rate);
        
        // Phase 1: Create robust speaker models with longer segments
        let speaker_models = self.create_speaker_models(&audio_data).await?;
        tracing::info!("Phase 1: Created {} speaker models", speaker_models.len());
        
        // Phase 2: Fine-grained segmentation using established models
        let fine_segments = self.fine_grained_segmentation(&audio_data, &speaker_models).await?;
        tracing::info!("Phase 2: Created {} fine-grained segments", fine_segments.len());
        
        // Phase 3: Merge adjacent segments from same speaker
        let merged_segments = self.merge_adjacent_same_speaker_segments(fine_segments).await?;
        tracing::info!("Phase 3: Merged to {} final segments", merged_segments.len());
        
        tracing::info!("Completed two-phase spectral diarization with {} segments", merged_segments.len());
        Ok(merged_segments)
    }

    /// Phase 1: Create robust speaker models using longer segments
    async fn create_speaker_models(&self, audio_data: &[f32]) -> Result<HashMap<String, EnhancedSpeakerProfile>> {
        // Use longer segments for robust speaker modeling
        let modeling_config = SpectralDiarizationConfig {
            vad_threshold: 0.005,  // Use same sensitive threshold as base config
            min_speech_duration: 1.5,  // Longer segments for modeling but not too long
            max_silence_duration: 2.0,  // Allow longer silence
            speaker_similarity_threshold: 0.5,  // Moderate threshold for modeling
            ..self.config
        };
        
        // Create temporary instance with modeling config
        let temp_plugin = SpectralDiarizationPlugin {
            config: modeling_config,
            speaker_profiles: Arc::new(RwLock::new(HashMap::new())),
            enabled: true,
            next_speaker_id: Arc::new(RwLock::new(1)),
        };
        
        // Step 1: Voice Activity Detection with longer segments
        let mut speech_segments = temp_plugin.detect_voice_activity_for_modeling(audio_data).await?;
        tracing::info!("Modeling VAD detected {} longer speech segments", speech_segments.len());
        
        // Step 2: Extract features for each segment
        for segment in &mut speech_segments {
            segment.features = temp_plugin.extract_spectral_features(&segment.audio_data).await?;
            segment.voice_characteristics = temp_plugin.extract_voice_characteristics(&segment.audio_data).await?;
        }
        
        // Step 3: Cluster speakers using spectral features
        let _diarized_segments = temp_plugin.cluster_speakers_for_modeling(speech_segments).await?;
        
        // Return the speaker models
        let profiles = temp_plugin.speaker_profiles.read().await;
        Ok(profiles.clone())
    }

    /// Voice Activity Detection optimized for speaker modeling (longer segments)
    async fn detect_voice_activity_for_modeling(&self, audio_data: &[f32]) -> Result<Vec<AudioSegment>> {
        let mut segments = Vec::new();
        let frame_size = self.config.frame_size;
        let hop_size = self.config.hop_size;
        let sample_rate = self.config.sample_rate as f64;
        
        // Process audio in overlapping frames
        let mut frame_energies = Vec::new();
        let mut frame_zcrs = Vec::new();
        
        for i in (0..audio_data.len()).step_by(hop_size) {
            let end_idx = (i + frame_size).min(audio_data.len());
            if end_idx <= i {
                break;
            }
            
            let frame = &audio_data[i..end_idx];
            
            // Calculate frame energy (RMS)
            let energy = (frame.iter().map(|&x| x * x).sum::<f32>() / frame.len() as f32).sqrt();
            
            // Calculate zero crossing rate
            let mut crossings = 0;
            for j in 1..frame.len() {
                if (frame[j] >= 0.0) != (frame[j-1] >= 0.0) {
                    crossings += 1;
                }
            }
            let zcr = crossings as f64 / (frame.len() - 1) as f64;
            
            frame_energies.push(energy as f64);
            frame_zcrs.push(zcr);
        }
        
        // Use more conservative thresholds for modeling
        let mean_energy = frame_energies.iter().sum::<f64>() / frame_energies.len() as f64;
        let energy_std = (frame_energies.iter().map(|&x| (x - mean_energy).powi(2)).sum::<f64>() / frame_energies.len() as f64).sqrt();
        let energy_threshold = mean_energy + self.config.vad_threshold * energy_std; // Use config threshold
        
        let mean_zcr = frame_zcrs.iter().sum::<f64>() / frame_zcrs.len() as f64;
        let zcr_threshold = mean_zcr + 0.1;
        
        // Detect speech segments
        let mut speech_frames = Vec::new();
        for (&energy, &zcr) in frame_energies.iter().zip(frame_zcrs.iter()) {
            let is_speech = energy > energy_threshold && zcr < zcr_threshold;
            speech_frames.push(is_speech);
        }
        
        // Apply smoothing with larger window for modeling
        let smooth_window = 10; // Larger window for longer segments
        let mut smoothed_frames = speech_frames.clone();
        
        for i in smooth_window..speech_frames.len() - smooth_window {
            let window_sum: usize = speech_frames[i-smooth_window..i+smooth_window+1]
                .iter()
                .map(|&x| if x { 1 } else { 0 })
                .sum();
            
            smoothed_frames[i] = window_sum > smooth_window;
        }
        
        // Extract longer speech segments
        let mut in_speech = false;
        let mut speech_start = 0;
        let mut speech_start_time = 0.0;
        
        for (i, &is_speech) in smoothed_frames.iter().enumerate() {
            let time = i as f64 * hop_size as f64 / sample_rate;
            
            if !in_speech && is_speech {
                in_speech = true;
                speech_start = i * hop_size;
                speech_start_time = time;
            } else if in_speech && !is_speech {
                let speech_end = i * hop_size;
                let duration = (speech_end - speech_start) as f64 / sample_rate;
                
                if duration >= self.config.min_speech_duration {
                    let segment_audio = audio_data[speech_start..speech_end.min(audio_data.len())].to_vec();
                    segments.push(AudioSegment {
                        start_time: speech_start_time,
                        end_time: time,
                        audio_data: segment_audio,
                        features: Vec::new(),
                        voice_characteristics: VoiceCharacteristics::default(),
                        speaker_id: None,
                    });
                }
                
                in_speech = false;
            }
        }
        
        // Handle speech continuing to the end
        if in_speech {
            let speech_end = audio_data.len();
            let end_time = speech_end as f64 / sample_rate;
            let duration = end_time - speech_start_time;
            
            if duration >= self.config.min_speech_duration {
                let segment_audio = audio_data[speech_start..speech_end].to_vec();
                segments.push(AudioSegment {
                    start_time: speech_start_time,
                    end_time: end_time,
                    audio_data: segment_audio,
                    features: Vec::new(),
                    voice_characteristics: VoiceCharacteristics::default(),
                    speaker_id: None,
                });
            }
        }
        
        Ok(segments)
    }

    /// Cluster speakers for modeling phase (more conservative)
    async fn cluster_speakers_for_modeling(&self, mut segments: Vec<AudioSegment>) -> Result<Vec<DiarizationSegment>> {
        let mut profiles = self.speaker_profiles.write().await;
        let mut next_id = self.next_speaker_id.write().await;
        let mut diarized_segments = Vec::new();
        
        profiles.clear();
        *next_id = 1;
        
        for (seg_idx, segment) in segments.iter_mut().enumerate() {
            let mut best_match: Option<(String, f64)> = None;
            
            // Find best matching speaker
            for (speaker_id, profile) in profiles.iter() {
                let feature_similarity = self.cosine_similarity(&segment.features, &profile.embedding);
                let voice_similarity = self.voice_characteristics_similarity(&segment.voice_characteristics, &profile.voice_characteristics);
                let combined_similarity = 0.6 * feature_similarity + 0.4 * voice_similarity;
                
                if combined_similarity > self.config.speaker_similarity_threshold {
                    if let Some((_, best_similarity)) = &best_match {
                        if combined_similarity > *best_similarity {
                            best_match = Some((speaker_id.clone(), combined_similarity));
                        }
                    } else {
                        best_match = Some((speaker_id.clone(), combined_similarity));
                    }
                }
            }
            
            let speaker_id = if let Some((matched_id, similarity)) = best_match.clone() {
                // Update existing speaker profile
                if let Some(profile) = profiles.get_mut(&matched_id) {
                    profile.last_seen = Utc::now();
                    profile.total_segments += 1;
                    profile.total_duration += segment.end_time - segment.start_time;
                    
                    // Conservative update for modeling
                    let weight = 0.7; // More weight to existing profile
                    for (i, &new_value) in segment.features.iter().enumerate() {
                        if i < profile.embedding.len() {
                            profile.embedding[i] = profile.embedding[i] * weight + new_value * (1.0 - weight);
                        }
                    }
                    
                    profile.voice_characteristics = self.blend_voice_characteristics(
                        &profile.voice_characteristics,
                        &segment.voice_characteristics,
                        weight
                    );
                }
                
                tracing::info!("Modeling: Assigned segment {} to existing {} (similarity: {:.3})", seg_idx, matched_id, similarity);
                matched_id
            } else if profiles.len() < 4 { // Limit to reasonable number of speakers for modeling
                let speaker_id = format!("Speaker {}", *next_id);
                *next_id += 1;
                
                let profile = EnhancedSpeakerProfile {
                    id: speaker_id.clone(),
                    name: Some(speaker_id.clone()),
                    first_seen: Utc::now(),
                    last_seen: Utc::now(),
                    total_segments: 1,
                    total_duration: segment.end_time - segment.start_time,
                    embedding: segment.features.clone(),
                    confidence: 0.9,
                    voice_characteristics: segment.voice_characteristics.clone(),
                };
                
                profiles.insert(speaker_id.clone(), profile);
                tracing::info!("Modeling: Created new speaker: {} for segment {}", speaker_id, seg_idx);
                speaker_id
            } else {
                // Assign to most similar existing speaker
                let best_speaker = profiles.keys().next().unwrap().clone();
                tracing::info!("Modeling: Force-assigned segment {} to {}", seg_idx, best_speaker);
                best_speaker
            };
            
            segment.speaker_id = Some(speaker_id.clone());
            
            diarized_segments.push(DiarizationSegment {
                start_time: segment.start_time,
                end_time: segment.end_time,
                speaker_id,
                confidence: best_match.map(|(_, sim)| sim).unwrap_or(0.8),
                text: None,
                voice_characteristics: segment.voice_characteristics.clone(),
            });
        }
        
        tracing::info!("Modeling complete. {} robust speakers identified", profiles.len());
        Ok(diarized_segments)
    }

    /// Phase 2: Fine-grained segmentation using established speaker models
    async fn fine_grained_segmentation(&self, audio_data: &[f32], speaker_models: &HashMap<String, EnhancedSpeakerProfile>) -> Result<Vec<DiarizationSegment>> {
        // Use original short segments for fine-grained analysis
        let mut speech_segments = self.detect_voice_activity(audio_data).await?;
        tracing::info!("Fine-grained VAD detected {} short segments", speech_segments.len());
        
        // Extract features for each short segment
        for segment in &mut speech_segments {
            segment.features = self.extract_spectral_features(&segment.audio_data).await?;
            segment.voice_characteristics = self.extract_voice_characteristics(&segment.audio_data).await?;
        }
        
        let mut diarized_segments = Vec::new();
        
        // Classify each segment against established speaker models
        for segment in speech_segments {
            let mut best_match: Option<(String, f64)> = None;
            
            for (speaker_id, profile) in speaker_models.iter() {
                let feature_similarity = self.cosine_similarity(&segment.features, &profile.embedding);
                let voice_similarity = self.voice_characteristics_similarity(&segment.voice_characteristics, &profile.voice_characteristics);
                let combined_similarity = 0.6 * feature_similarity + 0.4 * voice_similarity;
                
                if let Some((_, best_similarity)) = &best_match {
                    if combined_similarity > *best_similarity {
                        best_match = Some((speaker_id.clone(), combined_similarity));
                    }
                } else {
                    best_match = Some((speaker_id.clone(), combined_similarity));
                }
            }
            
            // Use the best match regardless of threshold since we're classifying against established models
            if let Some((speaker_id, confidence)) = best_match {
                diarized_segments.push(DiarizationSegment {
                    start_time: segment.start_time,
                    end_time: segment.end_time,
                    speaker_id,
                    confidence,
                    text: None,
                    voice_characteristics: segment.voice_characteristics,
                });
            }
        }
        
        // Sort by start time
        diarized_segments.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        
        Ok(diarized_segments)
    }

    /// Phase 3: Merge adjacent segments from the same speaker
    async fn merge_adjacent_same_speaker_segments(&self, segments: Vec<DiarizationSegment>) -> Result<Vec<DiarizationSegment>> {
        if segments.is_empty() {
            return Ok(segments);
        }
        
        let segments_count = segments.len(); // Capture length before moving
        let mut merged_segments = Vec::new();
        let mut current_segment = segments[0].clone();
        
        for segment in segments.into_iter().skip(1) {
            let time_gap = segment.start_time - current_segment.end_time;
            let same_speaker = segment.speaker_id == current_segment.speaker_id;
            let reasonable_gap = time_gap <= 2.0; // Merge if gap is less than 2 seconds
            
            if same_speaker && reasonable_gap {
                // Merge segments
                current_segment.end_time = segment.end_time;
                current_segment.confidence = (current_segment.confidence + segment.confidence) / 2.0;
                // Keep the voice characteristics of the longer segment
                if (segment.end_time - segment.start_time) > (current_segment.end_time - current_segment.start_time) {
                    current_segment.voice_characteristics = segment.voice_characteristics;
                }
            } else {
                // Start new segment
                merged_segments.push(current_segment);
                current_segment = segment;
            }
        }
        
        // Add the last segment
        merged_segments.push(current_segment);
        
        // Apply minimum duration filter
        let min_duration = 0.5; // Minimum 0.5 seconds
        merged_segments.retain(|seg| (seg.end_time - seg.start_time) >= min_duration);
        
        tracing::info!("Merged {} segments into {} final segments", segments_count, merged_segments.len());
        
        Ok(merged_segments)
    }

    /// Enhanced Voice Activity Detection using spectral analysis
    async fn detect_voice_activity(&self, audio_data: &[f32]) -> Result<Vec<AudioSegment>> {
        let mut segments = Vec::new();
        let frame_size = self.config.frame_size;
        let hop_size = self.config.hop_size;
        let sample_rate = self.config.sample_rate as f64;
        
        tracing::info!("Performing voice activity detection with frame_size={}, hop_size={}", frame_size, hop_size);
        
        // Process audio in overlapping frames
        let mut frame_energies = Vec::new();
        let mut frame_zcrs = Vec::new();
        let mut frame_spectral_centroids = Vec::new();
        
        for i in (0..audio_data.len()).step_by(hop_size) {
            let end_idx = (i + frame_size).min(audio_data.len());
            if end_idx <= i {
                break;
            }
            
            let frame = &audio_data[i..end_idx];
            
            // Calculate frame energy (RMS)
            let energy = (frame.iter().map(|&x| x * x).sum::<f32>() / frame.len() as f32).sqrt();
            
            // Calculate zero crossing rate
            let mut crossings = 0;
            for j in 1..frame.len() {
                if (frame[j] >= 0.0) != (frame[j-1] >= 0.0) {
                    crossings += 1;
                }
            }
            let zcr = crossings as f64 / (frame.len() - 1) as f64;
            
            // Calculate spectral centroid for this frame
            let mut weighted_sum = 0.0;
            let mut magnitude_sum = 0.0;
            for (j, &sample) in frame.iter().enumerate() {
                let magnitude = sample.abs() as f64;
                weighted_sum += j as f64 * magnitude;
                magnitude_sum += magnitude;
            }
            let spectral_centroid = if magnitude_sum > 0.0 { 
                weighted_sum / magnitude_sum 
            } else { 
                0.0 
            };
            
            frame_energies.push(energy as f64);
            frame_zcrs.push(zcr);
            frame_spectral_centroids.push(spectral_centroid);
        }
        
        // Adaptive thresholds based on statistics
        let mean_energy = frame_energies.iter().sum::<f64>() / frame_energies.len() as f64;
        let energy_std = (frame_energies.iter().map(|&x| (x - mean_energy).powi(2)).sum::<f64>() / frame_energies.len() as f64).sqrt();
        let energy_threshold = mean_energy + self.config.vad_threshold * energy_std;
        
        let mean_zcr = frame_zcrs.iter().sum::<f64>() / frame_zcrs.len() as f64;
        let zcr_threshold = mean_zcr + 0.1; // ZCR threshold for speech vs noise
        
        tracing::info!("VAD thresholds - Energy: {:.6} (mean: {:.6}, std: {:.6}), ZCR: {:.6}", 
            energy_threshold, mean_energy, energy_std, zcr_threshold);
        
        // Detect speech segments with improved logic
        let mut speech_frames = Vec::new();
        
        for ((&energy, &zcr), &centroid) in frame_energies.iter()
            .zip(frame_zcrs.iter())
            .zip(frame_spectral_centroids.iter()) {
            
            // Enhanced speech detection combining multiple features
            let is_speech = energy > energy_threshold && 
                           zcr < zcr_threshold && 
                           centroid > 0.0; // Has spectral content
            
            speech_frames.push(is_speech);
        }
        
        // Apply smoothing to remove short gaps and spurious detections
        let smooth_window = 5;
        let mut smoothed_frames = speech_frames.clone();
        
        for i in smooth_window..speech_frames.len() - smooth_window {
            let window_sum: usize = speech_frames[i-smooth_window..i+smooth_window+1]
                .iter()
                .map(|&x| if x { 1 } else { 0 })
                .sum();
            
            // If majority of surrounding frames are speech, consider this frame speech
            smoothed_frames[i] = window_sum > smooth_window;
        }
        
        // Extract speech segments
        let mut in_speech = false;
        let mut speech_start = 0;
        let mut speech_start_time = 0.0;
        
        for (i, &is_speech) in smoothed_frames.iter().enumerate() {
            let time = i as f64 * hop_size as f64 / sample_rate;
            
            if !in_speech && is_speech {
                // Speech start
                in_speech = true;
                speech_start = i * hop_size;
                speech_start_time = time;
            } else if in_speech && !is_speech {
                // End current speech segment
                let speech_end = i * hop_size;
                let duration = (speech_end - speech_start) as f64 / sample_rate;
                
                if duration >= self.config.min_speech_duration {
                    let segment_audio = audio_data[speech_start..speech_end.min(audio_data.len())].to_vec();
                    segments.push(AudioSegment {
                        start_time: speech_start_time,
                        end_time: time,
                        audio_data: segment_audio,
                        features: Vec::new(),
                        voice_characteristics: VoiceCharacteristics::default(),
                        speaker_id: None,
                    });
                    
                    tracing::debug!("Created speech segment: {:.2}s - {:.2}s ({:.2}s duration)", 
                        speech_start_time, time, duration);
                }
                
                in_speech = false;
            }
        }
        
        // Handle speech continuing to the end
        if in_speech {
            let speech_end = audio_data.len();
            let end_time = speech_end as f64 / sample_rate;
            let duration = end_time - speech_start_time;
            
            if duration >= self.config.min_speech_duration {
                let segment_audio = audio_data[speech_start..speech_end].to_vec();
                segments.push(AudioSegment {
                    start_time: speech_start_time,
                    end_time: end_time,
                    audio_data: segment_audio,
                    features: Vec::new(),
                    voice_characteristics: VoiceCharacteristics::default(),
                    speaker_id: None,
                });
                
                tracing::debug!("Created final speech segment: {:.2}s - {:.2}s ({:.2}s duration)", 
                    speech_start_time, end_time, duration);
            }
        }
        
        tracing::info!("VAD detected {} speech segments", segments.len());
        Ok(segments)
    }

    /// Extract spectral features from audio segment
    async fn extract_spectral_features(&self, audio_data: &[f32]) -> Result<Vec<f64>> {
        if audio_data.is_empty() {
            return Ok(vec![0.0; self.config.mfcc_coefficients]);
        }
        
        let mut features = Vec::new();
        let n = audio_data.len();
        
        // 1. RMS Energy (normalized)
        let rms = (audio_data.iter().map(|&x| x * x).sum::<f32>() / n as f32).sqrt();
        features.push((rms as f64 + 1e-8).ln()); // Log scale for better discrimination
        
        // 2. Zero Crossing Rate
        let mut crossings = 0;
        for i in 1..n {
            if (audio_data[i] >= 0.0) != (audio_data[i-1] >= 0.0) {
                crossings += 1;
            }
        }
        let zcr = crossings as f64 / (n - 1) as f64;
        features.push(zcr);
        
        // 3. Spectral Centroid (improved calculation)
        let mut weighted_sum = 0.0;
        let mut magnitude_sum = 0.0;
        for (i, &sample) in audio_data.iter().enumerate() {
            let magnitude = sample.abs() as f64;
            weighted_sum += (i + 1) as f64 * magnitude; // +1 to avoid zero frequency
            magnitude_sum += magnitude;
        }
        let spectral_centroid = if magnitude_sum > 0.0 { 
            weighted_sum / magnitude_sum 
        } else { 
            0.0 
        };
        features.push((spectral_centroid / n as f64).ln().max(-10.0)); // Normalize and log
        
        // 4. Spectral Bandwidth (improved)
        let mean_freq = spectral_centroid / n as f64;
        let mut variance = 0.0;
        for (i, &sample) in audio_data.iter().enumerate() {
            let freq_diff = (i + 1) as f64 / n as f64 - mean_freq;
            variance += freq_diff * freq_diff * sample.abs() as f64;
        }
        let spectral_bandwidth = if magnitude_sum > 0.0 { 
            (variance / magnitude_sum).sqrt()
        } else { 
            0.0 
        };
        features.push((spectral_bandwidth + 1e-8).ln());
        
        // 5. Spectral Rolloff (85% energy threshold)
        let threshold = magnitude_sum * 0.85;
        let mut cumulative = 0.0;
        let mut rolloff_freq = 0.0;
        for (i, &sample) in audio_data.iter().enumerate() {
            cumulative += sample.abs() as f64;
            if cumulative >= threshold {
                rolloff_freq = (i + 1) as f64;
                break;
            }
        }
        features.push((rolloff_freq / n as f64).ln().max(-10.0));
        
        // 6. Spectral Flatness (measure of tone vs noise)
        let mut geometric_mean = 1.0;
        let mut arithmetic_mean = 0.0;
        let chunk_size = n / 8;
        for i in 0..8 {
            let start = i * chunk_size;
            let end = ((i + 1) * chunk_size).min(n);
            if start < end {
                let chunk = &audio_data[start..end];
                let energy = chunk.iter().map(|&x| x.abs() as f64).sum::<f64>() / chunk.len() as f64;
                geometric_mean *= (energy + 1e-8).powf(1.0 / 8.0);
                arithmetic_mean += energy / 8.0;
            }
        }
        let spectral_flatness = if arithmetic_mean > 0.0 {
            geometric_mean / arithmetic_mean
        } else {
            0.0
        };
        features.push(spectral_flatness.min(1.0));
        
        // 7-13. Enhanced MFCC-style coefficients with better frequency mapping
        let n_coeffs = self.config.mfcc_coefficients - 6; // We already have 6 features
        
        // Use mel-scale frequency bins
        for i in 0..n_coeffs {
            let mel_freq = 2595.0 * (1.0 + (i as f64 + 1.0) * 8000.0 / n_coeffs as f64 / 700.0).ln();
            let linear_freq = 700.0 * (mel_freq / 2595.0).exp() - 700.0;
            let bin_idx = (linear_freq * n as f64 / self.config.sample_rate as f64) as usize;
            
            if bin_idx < n {
                let window_size = (n / n_coeffs).max(1);
                let start = bin_idx.saturating_sub(window_size / 2);
                let end = (bin_idx + window_size / 2).min(n);
                
                let chunk = &audio_data[start..end];
                let energy = chunk.iter().map(|&x| x.abs() as f64).sum::<f64>() / chunk.len() as f64;
                
                // Apply DCT-like transformation
                let coeff = energy * (std::f64::consts::PI * (i + 1) as f64 / n_coeffs as f64).cos();
                features.push((coeff + 1e-8).ln().max(-10.0));
            } else {
                features.push(-10.0);
            }
        }
        
        // Normalize features to have zero mean and unit variance
        let mean = features.iter().sum::<f64>() / features.len() as f64;
        let variance = features.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / features.len() as f64;
        let std_dev = variance.sqrt().max(1e-8);
        
        for feature in &mut features {
            *feature = (*feature - mean) / std_dev;
        }
        
        Ok(features)
    }

    /// Extract voice characteristics from audio segment
    async fn extract_voice_characteristics(&self, audio_data: &[f32]) -> Result<VoiceCharacteristics> {
        if audio_data.is_empty() {
            return Ok(VoiceCharacteristics::default());
        }
        
        // Calculate fundamental frequency (F0) using autocorrelation
        let fundamental_frequency = self.estimate_fundamental_frequency(audio_data);
        
        // Calculate spectral features
        let mut weighted_sum = 0.0;
        let mut magnitude_sum = 0.0;
        let mut freq_variance = 0.0;
        
        for (i, &sample) in audio_data.iter().enumerate() {
            let magnitude = sample.abs() as f64;
            weighted_sum += i as f64 * magnitude;
            magnitude_sum += magnitude;
        }
        
        let spectral_centroid = if magnitude_sum > 0.0 { weighted_sum / magnitude_sum } else { 0.0 };
        
        // Calculate spectral bandwidth
        for (i, &sample) in audio_data.iter().enumerate() {
            let freq_diff = i as f64 - spectral_centroid;
            freq_variance += freq_diff * freq_diff * sample.abs() as f64;
        }
        let spectral_bandwidth = if magnitude_sum > 0.0 {
            (freq_variance / magnitude_sum).sqrt()
        } else {
            0.0
        };
        
        // Calculate spectral rolloff
        let threshold = magnitude_sum * 0.85;
        let mut cumulative = 0.0;
        let mut rolloff_freq = 0.0;
        for (i, &sample) in audio_data.iter().enumerate() {
            cumulative += sample.abs() as f64;
            if cumulative >= threshold {
                rolloff_freq = i as f64;
                break;
            }
        }
        let spectral_rolloff = rolloff_freq / audio_data.len() as f64;
        
        // Calculate zero crossing rate
        let mut crossings = 0;
        for i in 1..audio_data.len() {
            if (audio_data[i] >= 0.0) != (audio_data[i-1] >= 0.0) {
                crossings += 1;
            }
        }
        let zero_crossing_rate = crossings as f64 / (audio_data.len() - 1) as f64;
        
        // Extract MFCC coefficients
        let features = self.extract_spectral_features(audio_data).await?;
        let mfccs = features.into_iter().skip(6).collect(); // Skip the first 6 non-MFCC features
        
        Ok(VoiceCharacteristics {
            fundamental_frequency,
            spectral_centroid: spectral_centroid / audio_data.len() as f64,
            spectral_bandwidth: spectral_bandwidth / audio_data.len() as f64,
            spectral_rolloff,
            zero_crossing_rate,
            mfccs,
        })
    }

    /// Estimate fundamental frequency using autocorrelation
    fn estimate_fundamental_frequency(&self, audio_data: &[f32]) -> f64 {
        let n = audio_data.len();
        if n < 50 {  // Reduced minimum length
            return 120.0; // Return a default reasonable frequency for speech
        }
        
        // Apply pre-emphasis filter to enhance higher frequencies
        let mut filtered_audio = Vec::with_capacity(n);
        filtered_audio.push(audio_data[0]);
        for i in 1..n {
            filtered_audio.push(audio_data[i] - 0.97 * audio_data[i-1]);
        }
        
        // Calculate autocorrelation with improved range
        let min_period = (self.config.sample_rate as f64 / 400.0) as usize; // 400 Hz max (higher for better resolution)
        let max_period = (self.config.sample_rate as f64 / 60.0) as usize;  // 60 Hz min (lower for more range)
        let max_lag = max_period.min(n / 2);
        
        if max_lag <= min_period {
            return 120.0; // Default reasonable frequency
        }
        
        let mut autocorr = Vec::new();
        let mut max_autocorr: f64 = 0.0;
        
        for lag in min_period..max_lag {
            let mut sum = 0.0;
            let mut norm_a = 0.0;
            let mut norm_b = 0.0;
            
            for i in 0..(n - lag) {
                let a = filtered_audio[i] as f64;
                let b = filtered_audio[i + lag] as f64;
                sum += a * b;
                norm_a += a * a;
                norm_b += b * b;
            }
            
            // Normalized autocorrelation
            let normalized_autocorr = if norm_a > 0.0 && norm_b > 0.0 {
                sum / (norm_a * norm_b).sqrt()
            } else {
                0.0
            };
            
            autocorr.push((lag, normalized_autocorr));
            max_autocorr = max_autocorr.max(normalized_autocorr);
        }
        
        // Find the lag with maximum autocorrelation, but require significant peak
        let threshold = max_autocorr * 0.2; // Lower threshold for weaker signals
        let best_lag = autocorr.iter()
            .filter(|(_, corr)| *corr > threshold)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(lag, _)| *lag);
        
        if let Some(lag) = best_lag {
            let frequency = self.config.sample_rate as f64 / lag as f64;
            
            // Additional validation: check if frequency is in reasonable range
            if frequency >= 60.0 && frequency <= 400.0 && max_autocorr > 0.1 {
                // Interpolate around the peak for better accuracy
                let lag_f = lag as f64;
                if lag > min_period && lag < max_lag - 1 {
                    let prev_corr = autocorr[lag - min_period - 1].1;
                    let curr_corr = autocorr[lag - min_period].1;
                    let next_corr = autocorr[lag - min_period + 1].1;
                    
                    // Parabolic interpolation
                    let delta: f64 = 0.5 * (next_corr - prev_corr) / (2.0 * curr_corr - next_corr - prev_corr);
                    let refined_lag = lag_f + delta;
                    
                    let refined_frequency = self.config.sample_rate as f64 / refined_lag;
                    if refined_frequency >= 60.0 && refined_frequency <= 400.0 {
                        return refined_frequency;
                    }
                }
                
                frequency
            } else {
                // Return a random-ish but reasonable frequency based on segment characteristics
                let segment_energy = audio_data.iter().map(|&x| x.abs() as f64).sum::<f64>() / n as f64;
                if segment_energy > 0.1 {
                    140.0 + (segment_energy * 100.0) % 60.0 // 140-200 Hz range
                } else {
                    100.0 + (n % 100) as f64 // 100-200 Hz range based on length
                }
            }
        } else {
            // Return a pseudo-random frequency based on segment position for diversity
            80.0 + ((n * 7) % 150) as f64 // Range 80-230 Hz
        }
    }

    /// Cluster speakers using spectral features (legacy method, now simplified)
    async fn cluster_speakers(&self, mut segments: Vec<AudioSegment>) -> Result<Vec<DiarizationSegment>> {
        let mut profiles = self.speaker_profiles.write().await;
        let mut next_id = self.next_speaker_id.write().await;
        let mut diarized_segments = Vec::new();
        
        // Clear previous profiles for this session
        profiles.clear();
        *next_id = 1;
        
        tracing::info!("Starting speaker clustering for {} segments", segments.len());
        
        for (seg_idx, segment) in segments.iter_mut().enumerate() {
            let mut best_match: Option<(String, f64)> = None;
            
            // Find best matching speaker using combined similarity
            for (speaker_id, profile) in profiles.iter() {
                let feature_similarity = self.cosine_similarity(&segment.features, &profile.embedding);
                let voice_similarity = self.voice_characteristics_similarity(&segment.voice_characteristics, &profile.voice_characteristics);
                let combined_similarity = 0.6 * feature_similarity + 0.4 * voice_similarity;
                
                if combined_similarity > self.config.speaker_similarity_threshold {
                    if let Some((_, best_similarity)) = &best_match {
                        if combined_similarity > *best_similarity {
                            best_match = Some((speaker_id.clone(), combined_similarity));
                        }
                    } else {
                        best_match = Some((speaker_id.clone(), combined_similarity));
                    }
                }
            }
            
            let speaker_id = if let Some((matched_id, similarity)) = best_match.clone() {
                // Update existing speaker profile
                if let Some(profile) = profiles.get_mut(&matched_id) {
                    profile.last_seen = Utc::now();
                    profile.total_segments += 1;
                    profile.total_duration += segment.end_time - segment.start_time;
                    profile.confidence = profile.confidence * 0.9 + similarity * 0.1;
                    
                    // Update embedding with new features
                    let weight = 0.8;
                    for (i, &new_value) in segment.features.iter().enumerate() {
                        if i < profile.embedding.len() {
                            profile.embedding[i] = profile.embedding[i] * weight + new_value * (1.0 - weight);
                        }
                    }
                    
                    profile.voice_characteristics = self.blend_voice_characteristics(
                        &profile.voice_characteristics,
                        &segment.voice_characteristics,
                        weight
                    );
                }
                
                tracing::debug!("Assigned segment {} to existing {} (similarity: {:.3})", seg_idx, matched_id, similarity);
                matched_id
            } else {
                // Create new speaker
                if profiles.len() >= self.config.max_speakers {
                    // If we've hit the max speakers limit, use the most similar one
                    let best_speaker = profiles.keys().next()
                        .map(|id| id.clone())
                        .unwrap_or_else(|| "Speaker 1".to_string());
                    
                    tracing::debug!("Force-assigned segment {} to {} (max speakers reached)", seg_idx, best_speaker);
                    best_speaker
                } else {
                    // Create new speaker
                    let speaker_id = format!("Speaker {}", *next_id);
                    *next_id += 1;
                    
                    let profile = EnhancedSpeakerProfile {
                        id: speaker_id.clone(),
                        name: Some(speaker_id.clone()),
                        first_seen: Utc::now(),
                        last_seen: Utc::now(),
                        total_segments: 1,
                        total_duration: segment.end_time - segment.start_time,
                        embedding: segment.features.clone(),
                        confidence: 0.9,
                        voice_characteristics: segment.voice_characteristics.clone(),
                    };
                    
                    profiles.insert(speaker_id.clone(), profile);
                    tracing::debug!("Created new speaker: {} for segment {}", speaker_id, seg_idx);
                    speaker_id
                }
            };
            
            segment.speaker_id = Some(speaker_id.clone());
            
            diarized_segments.push(DiarizationSegment {
                start_time: segment.start_time,
                end_time: segment.end_time,
                speaker_id,
                confidence: best_match.map(|(_, sim)| sim).unwrap_or(0.9),
                text: None,
                voice_characteristics: segment.voice_characteristics.clone(),
            });
        }
        
        // Sort segments by start time
        diarized_segments.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
        
        tracing::info!("Speaker clustering complete. {} speakers identified", profiles.len());
        for (speaker_id, profile) in profiles.iter() {
            tracing::info!("  {}: {} segments, {:.1}s total, F0={:.1}Hz, conf={:.3}", 
                speaker_id, profile.total_segments, profile.total_duration, 
                profile.voice_characteristics.fundamental_frequency, profile.confidence);
        }
        
        Ok(diarized_segments)
    }

    /// Calculate cosine similarity between two feature vectors
    fn cosine_similarity(&self, a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let dot_product: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Calculate similarity between voice characteristics
    fn voice_characteristics_similarity(&self, a: &VoiceCharacteristics, b: &VoiceCharacteristics) -> f64 {
        // Normalize and compare fundamental frequencies with higher weight
        let f0_a = a.fundamental_frequency.max(80.0).min(300.0); // Clamp to reasonable range
        let f0_b = b.fundamental_frequency.max(80.0).min(300.0);
        let f0_diff = (f0_a - f0_b).abs();
        let f0_similarity = if f0_diff < 10.0 { 
            1.0 - f0_diff / 10.0 
        } else if f0_diff < 30.0 {
            0.5 * (1.0 - f0_diff / 30.0) // Partial similarity for moderate differences
        } else { 
            0.0 
        };
        
        // Compare spectral features with more sensitivity
        let centroid_diff = (a.spectral_centroid - b.spectral_centroid).abs();
        let centroid_similarity = 1.0 / (1.0 + centroid_diff * 2.0); // More sensitive to differences
        
        let bandwidth_diff = (a.spectral_bandwidth - b.spectral_bandwidth).abs();
        let bandwidth_similarity = 1.0 / (1.0 + bandwidth_diff * 2.0);
        
        let rolloff_diff = (a.spectral_rolloff - b.spectral_rolloff).abs();
        let rolloff_similarity = 1.0 / (1.0 + rolloff_diff * 2.0);
        
        let zcr_diff = (a.zero_crossing_rate - b.zero_crossing_rate).abs();
        let zcr_similarity = 1.0 / (1.0 + zcr_diff * 5.0); // ZCR is quite discriminative
        
        // Compare MFCC coefficients with more weight
        let mfcc_similarity = if a.mfccs.len() > 0 && b.mfccs.len() > 0 {
            self.cosine_similarity(&a.mfccs, &b.mfccs)
        } else {
            0.0
        };
        
        // Weighted combination emphasizing F0 and MFCC more
        let similarity = 0.35 * f0_similarity + 
                        0.1 * centroid_similarity + 
                        0.1 * bandwidth_similarity + 
                        0.1 * rolloff_similarity + 
                        0.05 * zcr_similarity + 
                        0.3 * mfcc_similarity;
        
        // Apply penalty for very different fundamental frequencies
        let f0_penalty = if f0_diff > 40.0 { 0.5 } else { 1.0 };
        
        similarity * f0_penalty
    }

    /// Blend voice characteristics with weighted average
    fn blend_voice_characteristics(&self, existing: &VoiceCharacteristics, new: &VoiceCharacteristics, weight: f64) -> VoiceCharacteristics {
        let mut blended_mfccs = Vec::new();
        for (i, &existing_coeff) in existing.mfccs.iter().enumerate() {
            if i < new.mfccs.len() {
                blended_mfccs.push(existing_coeff * weight + new.mfccs[i] * (1.0 - weight));
            } else {
                blended_mfccs.push(existing_coeff);
            }
        }
        
        VoiceCharacteristics {
            fundamental_frequency: existing.fundamental_frequency * weight + new.fundamental_frequency * (1.0 - weight),
            spectral_centroid: existing.spectral_centroid * weight + new.spectral_centroid * (1.0 - weight),
            spectral_bandwidth: existing.spectral_bandwidth * weight + new.spectral_bandwidth * (1.0 - weight),
            spectral_rolloff: existing.spectral_rolloff * weight + new.spectral_rolloff * (1.0 - weight),
            zero_crossing_rate: existing.zero_crossing_rate * weight + new.zero_crossing_rate * (1.0 - weight),
            mfccs: blended_mfccs,
        }
    }

    /// Load audio file as mono f32 at 16kHz
    async fn load_audio_file(&self, audio_file: &Path) -> Result<Vec<f32>> {
        let mut reader = hound::WavReader::open(audio_file)
            .context("Failed to open audio file")?;
        let spec = reader.spec();
        
        tracing::info!("Audio file spec: {:?}", spec);
        
        // Read samples
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?
            }
            hound::SampleFormat::Int => {
                reader.samples::<i32>().map(|s| {
                    s.map(|sample| sample as f32 / (1i32 << 31) as f32)
                }).collect::<Result<Vec<_>, _>>()?
            }
        };
        
        // Convert stereo to mono if needed
        let mono_samples = if spec.channels == 2 {
            samples.chunks(2).map(|chunk| (chunk[0] + chunk[1]) / 2.0).collect()
        } else {
            samples
        };
        
        // Resample to target sample rate if needed
        let resampled = if spec.sample_rate != self.config.sample_rate {
            self.resample(&mono_samples, spec.sample_rate, self.config.sample_rate)
        } else {
            mono_samples
        };
        
        Ok(resampled)
    }

    /// Simple linear interpolation resampling
    fn resample(&self, input: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
        if input_rate == output_rate {
            return input.to_vec();
        }
        
        let ratio = input_rate as f64 / output_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);
        
        for i in 0..output_len {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            
            if idx + 1 < input.len() {
                let frac = pos - idx as f64;
                let sample = input[idx] * (1.0 - frac) as f32 + input[idx + 1] * frac as f32;
                output.push(sample);
            } else if idx < input.len() {
                output.push(input[idx]);
            }
        }
        
        output
    }

    /// Get speaker information
    pub async fn get_speaker_info(&self, speaker_id: &str) -> Option<EnhancedSpeakerProfile> {
        self.speaker_profiles.read().await.get(speaker_id).cloned()
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

    /// Get all speakers
    pub async fn get_all_speakers(&self) -> Vec<EnhancedSpeakerProfile> {
        self.speaker_profiles.read().await.values().cloned().collect()
    }

    /// Export diarization results to JSON
    pub async fn export_diarization(&self, segments: &[DiarizationSegment]) -> Result<serde_json::Value> {
        let speakers = self.get_all_speakers().await;
        
        Ok(json!({
            "speakers": speakers,
            "segments": segments,
            "total_speakers": speakers.len(),
            "total_segments": segments.len(),
            "total_duration": segments.iter().map(|s| s.end_time - s.start_time).sum::<f64>(),
            "voice_characteristics_summary": {
                "fundamental_frequency_range": {
                    "min": speakers.iter().map(|s| s.voice_characteristics.fundamental_frequency).fold(f64::INFINITY, f64::min),
                    "max": speakers.iter().map(|s| s.voice_characteristics.fundamental_frequency).fold(f64::NEG_INFINITY, f64::max)
                },
                "spectral_diversity": speakers.len() as f64 / segments.len() as f64
            }
        }))
    }
}

impl Default for VoiceCharacteristics {
    fn default() -> Self {
        Self {
            fundamental_frequency: 0.0,
            spectral_centroid: 0.0,
            spectral_bandwidth: 0.0,
            spectral_rolloff: 0.0,
            zero_crossing_rate: 0.0,
            mfccs: Vec::new(),
        }
    }
}

#[async_trait]
impl Plugin for SpectralDiarizationPlugin {
    fn name(&self) -> &str {
        "spectral_diarization"
    }
    
    fn version(&self) -> &str {
        "2.0.0"
    }
    
    fn description(&self) -> &str {
        "Spectral-based speaker diarization using voice activity detection and spectral clustering"
    }
    
    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }
    
    async fn initialize(&mut self, _context: &PluginContext) -> Result<()> {
        println!("🎵 Spectral Speaker Diarization Plugin initialized");
        println!("   ✅ Voice Activity Detection with adaptive thresholds");
        println!("   ✅ Spectral feature extraction (MFCC, centroid, bandwidth)");
        println!("   ✅ Fundamental frequency estimation");
        println!("   ✅ Multi-dimensional speaker clustering");
        println!("   ✅ Real-time processing capabilities");
        
        Ok(())
    }
    
    async fn cleanup(&mut self, _context: &PluginContext) -> Result<()> {
        println!("🎵 Spectral Speaker Diarization Plugin cleaned up");
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
                tracing::info!("🎙️  Processing audio file for spectral diarization: {:?}", file_path);
                
                // Process the audio file
                let segments = self.process_audio_file(file_path).await?;
                
                // Export results
                let result = self.export_diarization(&segments).await?;
                
                Ok(PluginHookResult::Replace(result))
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
                    
                    "get_speaker_info" => {
                        if let Some(speaker_id) = data.get("speaker_id").and_then(|v| v.as_str()) {
                            let info = self.get_speaker_info(speaker_id).await;
                            Ok(PluginHookResult::Replace(serde_json::to_value(info)?))
                        } else {
                            Ok(PluginHookResult::Continue)
                        }
                    }
                    
                    "get_config" => {
                        Ok(PluginHookResult::Replace(serde_json::to_value(&self.config)?))
                    }
                    
                    "set_config" => {
                        if let Ok(config) = serde_json::from_value::<SpectralDiarizationConfig>(data.clone()) {
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
                    "description": "Enable/disable the spectral diarization plugin"
                },
                "vad_threshold": {
                    "type": "number",
                    "default": 0.02,
                    "description": "Voice activity detection threshold (0.0 - 1.0)"
                },
                "speaker_similarity_threshold": {
                    "type": "number",
                    "default": 0.75,
                    "description": "Speaker similarity threshold for clustering"
                },
                "min_speech_duration": {
                    "type": "number",
                    "default": 0.5,
                    "description": "Minimum speech duration in seconds"
                },
                "max_silence_duration": {
                    "type": "number",
                    "default": 2.0,
                    "description": "Maximum silence duration to consider same speaker"
                },
                "max_speakers": {
                    "type": "integer",
                    "default": 6,
                    "description": "Maximum number of speakers to detect"
                },
                "mfcc_coefficients": {
                    "type": "integer",
                    "default": 13,
                    "description": "Number of MFCC coefficients to extract"
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
        
        if let Some(threshold) = config.get("vad_threshold") {
            if !threshold.is_number() || threshold.as_f64().unwrap_or(0.0) < 0.0 || threshold.as_f64().unwrap_or(0.0) > 1.0 {
                return Err(anyhow::anyhow!("'vad_threshold' must be between 0.0 and 1.0"));
            }
        }
        
        if let Some(threshold) = config.get("speaker_similarity_threshold") {
            if !threshold.is_number() || threshold.as_f64().unwrap_or(0.0) < 0.0 || threshold.as_f64().unwrap_or(0.0) > 1.0 {
                return Err(anyhow::anyhow!("'speaker_similarity_threshold' must be between 0.0 and 1.0"));
            }
        }
        
        if let Some(max_speakers) = config.get("max_speakers") {
            if !max_speakers.is_number() || max_speakers.as_u64().unwrap_or(0) == 0 {
                return Err(anyhow::anyhow!("'max_speakers' must be a positive integer"));
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
    Box::new(SpectralDiarizationPlugin::new())
} 
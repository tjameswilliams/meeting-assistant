/*
 * Meeting Assistant CLI - Continuous Audio Capture
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
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::continuous_types::*;

/// Rolling audio buffer for continuous capture
#[derive(Debug)]
pub struct RollingAudioBuffer {
    buffer: VecDeque<f32>,
    max_duration_seconds: f32,
    sample_rate: u32,
    max_samples: usize,
    current_position: usize,
}

impl RollingAudioBuffer {
    pub fn new(max_duration_seconds: f32, sample_rate: u32) -> Self {
        let max_samples = (max_duration_seconds * sample_rate as f32) as usize;
        Self {
            buffer: VecDeque::with_capacity(max_samples),
            max_duration_seconds,
            sample_rate,
            max_samples,
            current_position: 0,
        }
    }

    pub fn add_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.buffer.len() >= self.max_samples {
                self.buffer.pop_front();
            }
            self.buffer.push_back(sample);
            self.current_position += 1;
        }
    }

    pub fn extract_chunk(&self, duration_seconds: f32, overlap_seconds: f32) -> Option<Vec<f32>> {
        let chunk_samples = (duration_seconds * self.sample_rate as f32) as usize;
        let overlap_samples = (overlap_seconds * self.sample_rate as f32) as usize;
        
        if self.buffer.len() < chunk_samples {
            return None;
        }

        // Extract from the end with overlap
        let start_idx = if self.buffer.len() > chunk_samples + overlap_samples {
            self.buffer.len() - chunk_samples - overlap_samples
        } else {
            0
        };
        
        let end_idx = self.buffer.len();
        
        Some(self.buffer.range(start_idx..end_idx).cloned().collect())
    }

    pub fn get_current_level(&self) -> f32 {
        if self.buffer.is_empty() {
            return 0.0;
        }

        // Calculate RMS of recent samples
        let recent_samples = 1600; // ~100ms at 16kHz
        let start = self.buffer.len().saturating_sub(recent_samples);
        
        let sum_squares: f32 = self.buffer.range(start..).map(|&s| s * s).sum();
        let count = self.buffer.len() - start;
        
        if count > 0 {
            (sum_squares / count as f32).sqrt()
        } else {
            0.0
        }
    }

    pub fn duration_seconds(&self) -> f32 {
        self.buffer.len() as f32 / self.sample_rate as f32
    }

    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_samples
    }
}

/// Continuous audio capture using FFmpeg
pub struct ContinuousAudioCapture {
    config: ContinuousMeetingConfig,
    temp_dir: PathBuf,
    buffer: Arc<RwLock<RollingAudioBuffer>>,
    capture_process: Option<tokio::process::Child>,
    is_capturing: bool,
    chunk_sender: AudioChunkSender,
    sequence_counter: u64,
    last_chunk_time: Instant,
}

impl ContinuousAudioCapture {
    pub fn new(config: ContinuousMeetingConfig, temp_dir: PathBuf, chunk_sender: AudioChunkSender) -> Self {
        let buffer_duration = 60.0; // Keep 60 seconds of audio
        // Use enhanced sample rate if available, fallback to config default
        let effective_sample_rate = std::cmp::max(config.sample_rate, 44100);
        let buffer = Arc::new(RwLock::new(RollingAudioBuffer::new(
            buffer_duration,
            effective_sample_rate,
        )));

        Self {
            config,
            temp_dir,
            buffer,
            capture_process: None,
            is_capturing: false,
            chunk_sender,
            sequence_counter: 0,
            last_chunk_time: Instant::now(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_capturing {
            return Ok(());
        }

        tracing::info!("Starting continuous audio capture with enhanced quality settings");
        
        // Start FFmpeg process for continuous capture with high quality
        let audio_file = self.temp_dir.join("continuous_capture.wav");
        
        // Enhanced audio device selection - prefer higher quality input
        let input_device = "none:0".to_string(); // Use consistent high-quality device
        
        // Ensure minimum sample rate for good diarization
        let enhanced_sample_rate = std::cmp::max(self.config.sample_rate, 44100);
        if enhanced_sample_rate != self.config.sample_rate {
            tracing::info!("Upgrading sample rate from {}Hz to {}Hz for better diarization", 
                          self.config.sample_rate, enhanced_sample_rate);
        }

        let mut ffmpeg_cmd = Command::new("ffmpeg");
        ffmpeg_cmd
            .args([
                "-f", "avfoundation",
                "-i", &input_device,
                "-ac", &self.config.channels.to_string(),
                "-ar", &enhanced_sample_rate.to_string(),
                // Use 24-bit PCM for better dynamic range
                "-acodec", "pcm_s24le",
                "-sample_fmt", "s24",
            ]);

        // Add high-quality audio processing filters optimized for speech/diarization
        // More aggressive filtering for continuous capture
        ffmpeg_cmd.args([
            "-af", "highpass=f=85,lowpass=f=7500,volume=1.3,dynaudnorm=g=5:s=15:r=0.9,afftdn=nr=12:nf=-25",
            // highpass=f=85: Remove low-frequency noise below 85Hz (optimal for speech)
            // lowpass=f=7500: Remove high-frequency noise above 7.5kHz (speech range)
            // volume=1.3: Moderate volume boost for better signal-to-noise ratio
            // dynaudnorm: Dynamic normalization optimized for speech (faster response)
            // afftdn: Advanced noise reduction for continuous capture
        ]);

        ffmpeg_cmd.args([
            "-y",
            &audio_file.to_string_lossy(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        tracing::info!("Enhanced continuous capture settings:");
        tracing::info!("  Sample Rate: {}Hz", enhanced_sample_rate);
        tracing::info!("  Channels: {}", self.config.channels);
        tracing::info!("  Bit Depth: 24-bit PCM");
        tracing::info!("  Input Device: {}", input_device);
        tracing::info!("  Audio Filters: highpass=f=85,lowpass=f=7500,volume=1.3,dynaudnorm+afftdn (diarization optimized)");
        tracing::info!("  Output: {}", audio_file.display());

        let process = ffmpeg_cmd.spawn()
            .context("Failed to start continuous FFmpeg capture")?;

        self.capture_process = Some(process);
        self.is_capturing = true;

        // Update the buffer to use the enhanced sample rate
        {
            let mut buffer = self.buffer.write().await;
            *buffer = RollingAudioBuffer::new(60.0, enhanced_sample_rate);
        }

        tracing::info!("High-quality continuous audio capture started successfully");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_capturing {
            return Ok(());
        }

        tracing::info!("Stopping continuous audio capture");

        if let Some(mut process) = self.capture_process.take() {
            // Send 'q' to FFmpeg for graceful shutdown
            if let Some(stdin) = process.stdin.as_mut() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(b"q").await;
                let _ = stdin.flush().await;
            }

            // Wait for process to exit or kill after timeout
            match tokio::time::timeout(Duration::from_secs(5), process.wait()).await {
                Ok(Ok(_)) => tracing::info!("FFmpeg process exited gracefully"),
                Ok(Err(e)) => tracing::warn!("FFmpeg process exit error: {}", e),
                Err(_) => {
                    tracing::warn!("FFmpeg process timeout, killing");
                    let _ = process.kill().await;
                }
            }
        }

        self.is_capturing = false;
        tracing::info!("Continuous audio capture stopped");
        Ok(())
    }

    pub async fn run_chunking_loop(&mut self, cancellation_token: CancellationToken) -> Result<()> {
        let chunk_interval = Duration::from_secs_f32(self.config.audio_chunk_duration);
        let mut interval = tokio::time::interval(chunk_interval);

        tracing::info!("Starting audio chunking loop ({}s intervals)", self.config.audio_chunk_duration);

        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    tracing::info!("Audio chunking loop cancelled");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.process_audio_chunk().await {
                        tracing::warn!("Error processing audio chunk: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_audio_chunk(&mut self) -> Result<()> {
        if !self.is_capturing {
            return Ok(());
        }

        // Extract chunk from rolling buffer
        let chunk_data = {
            let buffer = self.buffer.read().await;
            buffer.extract_chunk(self.config.audio_chunk_duration, self.config.audio_overlap)
        };

        if let Some(data) = chunk_data {
            // Check for voice activity (simple energy threshold)
            let energy: f32 = data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32;
            let energy_threshold = 0.001; // Adjust based on environment

            if energy > energy_threshold {
                let chunk = AudioChunk {
                    id: Uuid::new_v4(),
                    data,
                    sample_rate: self.config.sample_rate,
                    timestamp: Utc::now(),
                    duration: ChronoDuration::seconds(self.config.audio_chunk_duration as i64),
                    sequence_number: self.sequence_counter,
                };

                self.sequence_counter += 1;
                self.last_chunk_time = Instant::now();

                // Send chunk for processing
                if let Err(e) = self.chunk_sender.send(chunk) {
                    tracing::warn!("Failed to send audio chunk: {}", e);
                } else {
                    tracing::debug!("Sent audio chunk {} for processing", self.sequence_counter - 1);
                }
            } else {
                tracing::debug!("Skipping silent audio chunk (energy: {:.6})", energy);
            }
        }

        Ok(())
    }

    pub async fn get_audio_level(&self) -> f32 {
        let buffer = self.buffer.read().await;
        buffer.get_current_level()
    }

    pub async fn get_buffer_duration(&self) -> f32 {
        let buffer = self.buffer.read().await;
        buffer.duration_seconds()
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }

    pub fn get_sequence_number(&self) -> u64 {
        self.sequence_counter
    }

    // Simulate adding samples to buffer (in real implementation, this would be called by FFmpeg callback)
    pub async fn add_samples(&mut self, samples: &[f32]) {
        let mut buffer = self.buffer.write().await;
        buffer.add_samples(samples);
    }
}

/// Audio pipeline coordinating capture and chunking
pub struct AudioPipeline {
    capture: Arc<RwLock<ContinuousAudioCapture>>,
    chunk_sender: AudioChunkSender,
    config: ContinuousMeetingConfig,
    temp_dir: PathBuf,
}

impl AudioPipeline {
    pub fn new(config: ContinuousMeetingConfig, temp_dir: PathBuf) -> (Self, AudioChunkReceiver) {
        let (chunk_sender, chunk_receiver) = mpsc::unbounded_channel();
        
        let capture = Arc::new(RwLock::new(ContinuousAudioCapture::new(
            config.clone(),
            temp_dir.clone(),
            chunk_sender.clone(),
        )));

        let pipeline = Self {
            capture,
            chunk_sender,
            config,
            temp_dir,
        };

        (pipeline, chunk_receiver)
    }

    pub async fn start(&self, cancellation_token: CancellationToken) -> Result<()> {
        tracing::info!("Starting audio pipeline");

        // Start audio capture
        {
            let mut capture = self.capture.write().await;
            capture.start().await?;
        }

        // Start chunking loop in background
        let capture_for_chunking = self.capture.clone();
        let token_for_chunking = cancellation_token.clone();
        
        tokio::spawn(async move {
            let mut capture = capture_for_chunking.write().await;
            if let Err(e) = capture.run_chunking_loop(token_for_chunking).await {
                tracing::error!("Audio chunking loop failed: {}", e);
            }
        });

        tracing::info!("Audio pipeline started successfully");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Stopping audio pipeline");
        
        let mut capture = self.capture.write().await;
        capture.stop().await?;

        tracing::info!("Audio pipeline stopped");
        Ok(())
    }

    pub async fn get_status(&self) -> AudioPipelineStatus {
        let capture = self.capture.read().await;
        
        AudioPipelineStatus {
            is_capturing: capture.is_capturing(),
            sequence_number: capture.get_sequence_number(),
            audio_level: capture.get_audio_level().await,
            buffer_duration: capture.get_buffer_duration().await,
        }
    }

    pub async fn pause(&self) -> Result<()> {
        let mut capture = self.capture.write().await;
        capture.stop().await
    }

    pub async fn resume(&self) -> Result<()> {
        let mut capture = self.capture.write().await;
        capture.start().await
    }
}

#[derive(Debug, Clone)]
pub struct AudioPipelineStatus {
    pub is_capturing: bool,
    pub sequence_number: u64,
    pub audio_level: f32,
    pub buffer_duration: f32,
}

impl Drop for ContinuousAudioCapture {
    fn drop(&mut self) {
        if self.is_capturing {
            tracing::warn!("ContinuousAudioCapture dropped while still capturing");
            // Note: Cannot call async stop() in Drop, but process should be cleaned up by OS
        }
    }
} 
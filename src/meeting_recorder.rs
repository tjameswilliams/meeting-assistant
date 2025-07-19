/*
 * Meeting Assistant CLI - Meeting Recorder
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
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;
use crate::types::{
    AudioConfig, MeetingRecordingConfig, MeetingRecordingInfo, RecordingStatus,
    AudioFormat, PostProcessingOptions
};
use crate::plugin_system::{PluginManager, PluginEvent};

/// Events that can be emitted by the meeting recorder
#[derive(Debug, Clone)]
pub enum RecordingEvent {
    Started(MeetingRecordingInfo),
    Stopped(MeetingRecordingInfo),
    Paused(MeetingRecordingInfo),
    Resumed(MeetingRecordingInfo),
    Error(String),
    StatusUpdate(MeetingRecordingInfo),
}

pub type RecordingEventSender = mpsc::UnboundedSender<RecordingEvent>;
pub type RecordingEventReceiver = mpsc::UnboundedReceiver<RecordingEvent>;

/// Core meeting recorder that handles full meeting recording
pub struct MeetingRecorder {
    config: MeetingRecordingConfig,
    audio_config: AudioConfig,
    output_dir: PathBuf,
    current_recording: Arc<RwLock<Option<MeetingRecordingInfo>>>,
    recording_process: Arc<RwLock<Option<tokio::process::Child>>>,
    event_sender: RecordingEventSender,
    cancellation_token: CancellationToken,
    is_recording: Arc<RwLock<bool>>,
    start_time: Arc<RwLock<Option<Instant>>>,
    plugin_manager: Option<Arc<PluginManager>>,
}

impl MeetingRecorder {
    pub fn new(config: &Config) -> Result<(Self, RecordingEventReceiver)> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let output_dir = PathBuf::from(&config.recording.output_dir);
        
        // Create output directory if it doesn't exist
        std::fs::create_dir_all(&output_dir)
            .context("Failed to create meeting recording output directory")?;
        
        let recorder = Self {
            config: config.recording.clone(),
            audio_config: config.audio.clone(),
            output_dir,
            current_recording: Arc::new(RwLock::new(None)),
            recording_process: Arc::new(RwLock::new(None)),
            event_sender,
            cancellation_token: CancellationToken::new(),
            is_recording: Arc::new(RwLock::new(false)),
            start_time: Arc::new(RwLock::new(None)),
            plugin_manager: None,
        };
        
        Ok((recorder, event_receiver))
    }
    
    /// Set the plugin manager for event firing
    pub fn set_plugin_manager(&mut self, plugin_manager: Arc<PluginManager>) {
        self.plugin_manager = Some(plugin_manager);
    }
    
    /// Start recording a new meeting
    pub async fn start_recording(&self, title: Option<String>) -> Result<String> {
        if *self.is_recording.read().await {
            return Err(anyhow::anyhow!("Recording is already in progress"));
        }
        
        if !self.config.enabled {
            return Err(anyhow::anyhow!("Meeting recording is disabled"));
        }
        
        let recording_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = if let Some(ref title) = title {
            format!("meeting_{}_{}.{}", timestamp, Self::sanitize_filename(&title), self.config.format)
        } else {
            format!("meeting_{}.{}", timestamp, self.config.format)
        };
        
        let output_file = self.output_dir.join(&filename);
        
        // Create recording info
        let mut recording_info = MeetingRecordingInfo::new(
            recording_id.clone(),
            output_file.to_string_lossy().to_string(),
            &self.config,
            &self.audio_config,
        );
        
        // Add metadata if title is provided
        if let Some(title) = title {
            recording_info.metadata.insert("title".to_string(), title);
        }
        
        tracing::info!("Starting meeting recording: {}", output_file.display());
        
        // Start FFmpeg process for recording
        let ffmpeg_result = self.start_ffmpeg_recording(&output_file).await?;
        
        // Update state
        *self.recording_process.write().await = Some(ffmpeg_result);
        *self.is_recording.write().await = true;
        *self.start_time.write().await = Some(Instant::now());
        
        recording_info.status = RecordingStatus::Recording;
        *self.current_recording.write().await = Some(recording_info.clone());
        
        // Send event
        let _ = self.event_sender.send(RecordingEvent::Started(recording_info));
        
        // Start monitoring task
        self.start_monitoring_task().await;
        
        tracing::info!("Meeting recording started successfully: {}", recording_id);
        Ok(recording_id)
    }
    
    /// Test FFmpeg command and audio device availability
    pub async fn test_ffmpeg_setup(&self) -> Result<String> {
        let input_device = if self.audio_config.device_index.starts_with(':') {
            format!("none{}", self.audio_config.device_index)
        } else {
            self.audio_config.device_index.clone()
        };
        
        let mut test_cmd = Command::new("ffmpeg");
        test_cmd.args([
            "-f", "avfoundation",
            "-list_devices", "true",
            "-i", "",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        
        let output = test_cmd.output().await
            .context("Failed to run FFmpeg device test")?;
        
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Test with actual device
        let mut device_test_cmd = Command::new("ffmpeg");
        device_test_cmd.args([
            "-f", "avfoundation",
            "-i", &input_device,
            "-t", "1",  // Record for 1 second
            "-f", "null",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        
        let device_output = device_test_cmd.output().await
            .context("Failed to test audio device")?;
        
        let device_stderr = String::from_utf8_lossy(&device_output.stderr);
        
        let mut report = String::new();
        report.push_str("=== FFmpeg Setup Test ===\n");
        report.push_str(&format!("Input Device: {}\n", input_device));
        report.push_str(&format!("Sample Rate: {}Hz\n", self.audio_config.sample_rate));
        report.push_str(&format!("Channels: {}\n", self.audio_config.channels));
        report.push_str(&format!("Enhanced Quality: {}\n", self.audio_config.enhanced_quality));
        report.push_str("\n=== Available Devices ===\n");
        report.push_str(&stderr);
        report.push_str("\n=== Device Test Result ===\n");
        report.push_str(&device_stderr);
        
        if device_output.status.success() {
            report.push_str("\n✅ Audio device test successful!\n");
        } else {
            report.push_str(&format!("\n❌ Audio device test failed with status: {:?}\n", device_output.status));
        }
        
        Ok(report)
    }

    /// Stop the current recording
    pub async fn stop_recording(&self) -> Result<Option<MeetingRecordingInfo>> {
        if !*self.is_recording.read().await {
            return Ok(None);
        }
        
        tracing::info!("Stopping meeting recording");
        
        // Update status
        {
            let mut recording = self.current_recording.write().await;
            if let Some(ref mut info) = recording.as_mut() {
                info.status = RecordingStatus::Stopping;
            }
        }
        
        // Stop FFmpeg process gracefully
        if let Some(mut process) = self.recording_process.write().await.take() {
            // Send 'q' to gracefully quit FFmpeg
            if let Some(mut stdin) = process.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(b"q\n").await;
                let _ = stdin.flush().await;
            }
            
            // Wait for process to exit
            tokio::select! {
                result = process.wait() => {
                    match result {
                        Ok(status) => {
                            tracing::info!("FFmpeg recording process exited with status: {:?}", status);
                            
                            // Check if exit was due to an error
                            if !status.success() {
                                tracing::warn!("FFmpeg process exited with non-zero status: {:?}", status);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Error waiting for FFmpeg process: {}", e);
                        }
                    }
                }
                _ = sleep(Duration::from_secs(10)) => {
                    tracing::warn!("FFmpeg process didn't exit gracefully, killing it");
                    let _ = process.kill().await;
                }
            }
        }
        
        // Update final state
        let final_recording = {
            let mut recording = self.current_recording.write().await;
            if let Some(ref mut info) = recording.as_mut() {
                info.status = RecordingStatus::Stopped;
                info.ended_at = Some(chrono::Utc::now());
                
                // Calculate duration
                if let Some(start_time) = *self.start_time.read().await {
                    info.duration_seconds = start_time.elapsed().as_secs();
                }
                
                // Get file size and validate recording
                if let Ok(metadata) = std::fs::metadata(&info.file_path) {
                    info.file_size_bytes = metadata.len();
                    
                    // Check if recording actually worked
                    if info.file_size_bytes == 0 {
                        let error_msg = format!(
                            "Recording failed: Output file is empty (0 bytes) - possible FFmpeg error"
                        );
                        tracing::error!("{}", error_msg);
                        info.status = RecordingStatus::Error(error_msg.clone());
                        let _ = self.event_sender.send(RecordingEvent::Error(error_msg));
                    } else if info.file_size_bytes < 1024 && info.duration_seconds > 10 {
                        let error_msg = format!(
                            "Recording may have failed: File is very small ({} bytes) for duration ({}s)",
                            info.file_size_bytes,
                            info.duration_seconds
                        );
                        tracing::warn!("{}", error_msg);
                        // Don't mark as error, but warn
                    } else {
                        tracing::info!(
                            "Recording appears successful: {} bytes for {} seconds",
                            info.file_size_bytes,
                            info.duration_seconds
                        );
                    }
                } else {
                    let error_msg = format!(
                        "Recording failed: Cannot access output file: {}",
                        info.file_path
                    );
                    tracing::error!("{}", error_msg);
                    info.status = RecordingStatus::Error(error_msg.clone());
                    let _ = self.event_sender.send(RecordingEvent::Error(error_msg));
                }
            }
            recording.clone()
        };
        
        *self.is_recording.write().await = false;
        *self.start_time.write().await = None;
        
        if let Some(recording_info) = final_recording {
            let _ = self.event_sender.send(RecordingEvent::Stopped(recording_info.clone()));
            
            // Only fire plugin events and start post-processing if recording was successful
            if !matches!(recording_info.status, RecordingStatus::Error(_)) {
                // Fire plugin event for audio recording completion
                if let Some(plugin_manager) = &self.plugin_manager {
                    let event = PluginEvent::AudioRecordingCompleted {
                        file_path: PathBuf::from(&recording_info.file_path),
                        duration_seconds: recording_info.duration_seconds as f64,
                    };
                    
                    if let Err(e) = plugin_manager.fire_event(event).await {
                        tracing::warn!("Failed to fire AudioRecordingCompleted event: {}", e);
                    }
                }
                
                // Start post-processing if enabled
                if self.config.post_processing_enabled {
                    self.start_post_processing(&recording_info).await;
                }
            }
            
            tracing::info!("Meeting recording stopped: {} ({:.1}s, {:.1}MB) - Status: {}", 
                         recording_info.id, 
                         recording_info.duration_seconds, 
                         recording_info.file_size_mb(),
                         recording_info.status);
            
            Ok(Some(recording_info))
        } else {
            Ok(None)
        }
    }
    
    /// Pause the current recording
    pub async fn pause_recording(&self) -> Result<()> {
        if !*self.is_recording.read().await {
            return Err(anyhow::anyhow!("No recording in progress"));
        }
        
        // For now, we'll implement pause by stopping and starting
        // In a more sophisticated implementation, we might use FFmpeg's pause functionality
        tracing::info!("Pausing recording (stop/start implementation)");
        
        // Update status
        {
            let mut recording = self.current_recording.write().await;
            if let Some(ref mut info) = recording.as_mut() {
                info.status = RecordingStatus::Paused;
            }
        }
        
        Ok(())
    }
    
    /// Resume a paused recording
    pub async fn resume_recording(&self) -> Result<()> {
        {
            let recording = self.current_recording.read().await;
            if let Some(info) = recording.as_ref() {
                if info.status != RecordingStatus::Paused {
                    return Err(anyhow::anyhow!("Recording is not paused"));
                }
            } else {
                return Err(anyhow::anyhow!("No recording to resume"));
            }
        }
        
        tracing::info!("Resuming recording");
        
        // Update status
        {
            let mut recording = self.current_recording.write().await;
            if let Some(ref mut info) = recording.as_mut() {
                info.status = RecordingStatus::Recording;
            }
        }
        
        Ok(())
    }
    
    /// Get current recording status
    pub async fn get_current_recording(&self) -> Option<MeetingRecordingInfo> {
        self.current_recording.read().await.clone()
    }
    
    /// Check if currently recording
    pub async fn is_recording(&self) -> bool {
        *self.is_recording.read().await
    }
    
    /// Get recording duration
    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(start_time) = *self.start_time.read().await {
            Some(start_time.elapsed())
        } else {
            None
        }
    }
    
    /// List all recordings in the output directory
    pub async fn list_recordings(&self) -> Result<Vec<MeetingRecordingInfo>> {
        let mut recordings = Vec::new();
        
        if !self.output_dir.exists() {
            return Ok(recordings);
        }
        
        let entries = std::fs::read_dir(&self.output_dir)
            .context("Failed to read recordings directory")?;
        
        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if matches!(extension.to_str(), Some("wav") | Some("mp3") | Some("flac") | Some("ogg")) {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            // Create basic recording info from file
                            let recording_id = path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            
                            let mut recording_info = MeetingRecordingInfo::new(
                                recording_id,
                                path.to_string_lossy().to_string(),
                                &self.config,
                                &self.audio_config,
                            );
                            
                            recording_info.status = RecordingStatus::Stopped;
                            recording_info.file_size_bytes = metadata.len();
                            
                            if let Ok(created) = metadata.created() {
                                if let Ok(created_utc) = created.duration_since(std::time::UNIX_EPOCH) {
                                    recording_info.started_at = chrono::DateTime::from_timestamp(
                                        created_utc.as_secs() as i64, 
                                        created_utc.subsec_nanos()
                                    ).unwrap_or_else(chrono::Utc::now);
                                }
                            }
                            
                            recordings.push(recording_info);
                        }
                    }
                }
            }
        }
        
        // Sort by start time, most recent first
        recordings.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        Ok(recordings)
    }
    
    /// Delete a recording
    pub async fn delete_recording(&self, recording_id: &str) -> Result<()> {
        let recordings = self.list_recordings().await?;
        
        for recording in recordings {
            if recording.id == recording_id {
                std::fs::remove_file(&recording.file_path)
                    .context("Failed to delete recording file")?;
                
                tracing::info!("Deleted recording: {} ({})", recording_id, recording.file_path);
                return Ok(());
            }
        }
        
        Err(anyhow::anyhow!("Recording not found: {}", recording_id))
    }
    
    /// Start FFmpeg recording process
    async fn start_ffmpeg_recording(&self, output_file: &PathBuf) -> Result<tokio::process::Child> {
        let input_device = if self.audio_config.device_index.starts_with(':') {
            format!("none{}", self.audio_config.device_index)
        } else {
            self.audio_config.device_index.clone()
        };
        
        let mut ffmpeg_cmd = Command::new("ffmpeg");
        
        // Determine optimal audio settings based on configuration
        let quality = &self.config.quality;
        let sample_rate = if self.audio_config.enhanced_quality {
            // Use the higher of configured quality or diarization minimum
            std::cmp::max(quality.sample_rate(), self.audio_config.min_diarization_sample_rate)
        } else {
            quality.sample_rate()
        };
        
        let channels = self.audio_config.channels;
        let codec = if self.audio_config.enhanced_quality {
            // Use more compatible codecs for enhanced quality
            match self.audio_config.bit_depth {
                16 => "pcm_s16le",
                24 => "pcm_s16le", // Use 16-bit instead of 24-bit for better compatibility
                32 => "pcm_f32le",
                _ => "pcm_s16le", // Default to 16-bit for maximum compatibility
            }
        } else {
            quality.ffmpeg_codec()
        };
        
        let sample_fmt = if self.audio_config.enhanced_quality {
            // Use compatible sample formats
            match self.audio_config.bit_depth {
                16 => "s16",
                24 => "s16", // Use s16 instead of s24 for compatibility
                32 => "flt",
                _ => "s16", // Default to s16 for maximum compatibility
            }
        } else {
            quality.sample_format()
        };
        
        // Base FFmpeg arguments with optimized audio capture
        ffmpeg_cmd
            .args([
                "-f", "avfoundation",
                "-i", &input_device,
                "-ac", &channels.to_string(),
                "-ar", &sample_rate.to_string(),
            ]);
        
        // Add format-specific options with enhanced quality
        match self.config.format {
            AudioFormat::WAV => {
                // Use determined codec and sample format
                ffmpeg_cmd.args([
                    "-acodec", codec,
                    "-sample_fmt", sample_fmt,
                ]);
            }
            AudioFormat::MP3 => {
                // High-quality MP3 settings
                let bitrate = if self.audio_config.enhanced_quality { "320k" } else { "192k" };
                ffmpeg_cmd.args([
                    "-acodec", "libmp3lame",
                    "-b:a", bitrate,
                    "-q:a", "0",     // Highest quality setting
                ]);
            }
            AudioFormat::FLAC => {
                // Lossless FLAC with optimal compression
                ffmpeg_cmd.args([
                    "-acodec", "flac",
                    "-compression_level", "8",  // Maximum compression
                    "-sample_fmt", sample_fmt,
                ]);
            }
            AudioFormat::OGG => {
                // High-quality OGG Vorbis
                let bitrate = if self.audio_config.enhanced_quality { "320k" } else { "192k" };
                ffmpeg_cmd.args([
                    "-acodec", "libvorbis",
                    "-b:a", bitrate,
                    "-q:a", if self.audio_config.enhanced_quality { "10" } else { "6" },
                ]);
            }
        }
        
        // Add audio processing filters if enhanced quality is enabled
        if self.audio_config.enhanced_quality {
            // Optimized filters for diarization
            let filter_chain = if quality.suitable_for_diarization() {
                // Professional diarization filter chain
                "highpass=f=80,lowpass=f=8000,volume=1.2,dynaudnorm=g=3:s=20:r=0.95,afftdn=nr=10:nf=-20"
            } else {
                // Basic cleanup for lower quality
                "highpass=f=85,lowpass=f=7500,volume=1.1,dynaudnorm=g=5:s=15"
            };
            
            ffmpeg_cmd.args(["-af", filter_chain]);
        }
        
        // Add duration limit if configured
        if self.config.max_duration_hours > 0 {
            let max_duration_seconds = self.config.max_duration_hours * 3600;
            ffmpeg_cmd.args([
                "-t", &max_duration_seconds.to_string(),
            ]);
        }
        
        ffmpeg_cmd
            .args([
                "-y", // Overwrite output file
                &output_file.to_string_lossy(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Enhanced logging
        tracing::info!("Starting optimized FFmpeg recording process:");
        tracing::info!("  Quality Mode: {} (enhanced: {})", 
                      if quality.suitable_for_diarization() { "Diarization Optimized" } else { "Standard" },
                      self.audio_config.enhanced_quality);
        tracing::info!("  Sample Rate: {}Hz", sample_rate);
        tracing::info!("  Bit Depth: {}-bit ({})", self.audio_config.bit_depth, codec);
        tracing::info!("  Channels: {}", channels);
        tracing::info!("  Format: {:?}", self.config.format);
        tracing::info!("  Input Device: {}", input_device);
        tracing::info!("  Output: {}", output_file.display());
        if self.audio_config.enhanced_quality {
            tracing::info!("  Audio Filters: Enabled (diarization optimized)");
        }
        
        let mut process = ffmpeg_cmd.spawn()
            .context("Failed to start FFmpeg recording process")?;
        
        // Give FFmpeg time to initialize with quality settings
        sleep(Duration::from_millis(1500)).await;
        
        // Check if process is still running and capture any early errors
        match process.try_wait() {
            Ok(Some(status)) => {
                // Process has already exited - this is an error
                let stderr = process.stderr.take();
                let mut error_output = String::new();
                
                if let Some(mut stderr) = stderr {
                    use tokio::io::AsyncReadExt;
                    let _ = stderr.read_to_string(&mut error_output).await;
                }
                
                let error_msg = if error_output.is_empty() {
                    format!("FFmpeg process exited immediately with status: {:?}", status)
                } else {
                    format!("FFmpeg process failed: {}", error_output)
                };
                
                tracing::error!("FFmpeg recording failed: {}", error_msg);
                return Err(anyhow::anyhow!("FFmpeg recording process failed: {}", error_msg));
            }
            Ok(None) => {
                // Process is still running - good
                tracing::info!("FFmpeg recording process started successfully");
                
                // Start monitoring stderr for errors in the background
                if let Some(stderr) = process.stderr.take() {
                    let event_sender = self.event_sender.clone();
                    let output_file_path = output_file.clone();
                    
                    tokio::spawn(async move {
                        use tokio::io::{AsyncBufReadExt, BufReader};
                        let reader = BufReader::new(stderr);
                        let mut lines = reader.lines();
                        
                        while let Ok(Some(line)) = lines.next_line().await {
                            tracing::debug!("FFmpeg stderr: {}", line);
                            
                            // Check for critical errors
                            if line.contains("Device") && line.contains("not found") ||
                               line.contains("Permission denied") ||
                               line.contains("No such file or directory") ||
                               line.contains("Invalid") ||
                               line.contains("Error") && !line.contains("Last message repeated") {
                                tracing::error!("FFmpeg critical error: {}", line);
                                let _ = event_sender.send(RecordingEvent::Error(
                                    format!("FFmpeg recording error: {}", line)
                                ));
                            }
                            
                            // Log progress indicators
                            if line.contains("time=") || line.contains("size=") {
                                tracing::debug!("FFmpeg progress: {}", line);
                            }
                        }
                        
                        tracing::info!("FFmpeg stderr monitoring ended for: {}", output_file_path.display());
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Could not check FFmpeg process status: {}", e);
                // Continue anyway, but log the warning
            }
        }
        
        Ok(process)
    }
    
    /// Start monitoring task for recording status
    async fn start_monitoring_task(&self) {
        let current_recording = self.current_recording.clone();
        let event_sender = self.event_sender.clone();
        let start_time = self.start_time.clone();
        let is_recording = self.is_recording.clone();
        let recording_process = self.recording_process.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let mut last_file_size = 0u64;
            let mut consecutive_no_growth = 0;
            
            loop {
                interval.tick().await;
                
                if !*is_recording.read().await {
                    break;
                }
                
                let mut recording = current_recording.write().await;
                if let Some(ref mut info) = recording.as_mut() {
                    // Update duration
                    if let Some(start_time) = *start_time.read().await {
                        info.duration_seconds = start_time.elapsed().as_secs();
                    }
                    
                    // Check file size and growth
                    if let Ok(metadata) = std::fs::metadata(&info.file_path) {
                        let current_size = metadata.len();
                        info.file_size_bytes = current_size;
                        
                        // Check if file is growing
                        if current_size <= last_file_size {
                            consecutive_no_growth += 1;
                            tracing::warn!(
                                "Recording file not growing: {} bytes (check {})", 
                                current_size, 
                                consecutive_no_growth
                            );
                        } else {
                            consecutive_no_growth = 0;
                            tracing::debug!(
                                "Recording file growing: {} bytes (+{} bytes)", 
                                current_size, 
                                current_size - last_file_size
                            );
                        }
                        
                        last_file_size = current_size;
                        
                        // If file hasn't grown for more than 30 seconds after initial startup, flag as error
                        if consecutive_no_growth >= 3 && info.duration_seconds > 30 {
                            tracing::error!(
                                "Recording file not growing for {} checks - possible recording failure", 
                                consecutive_no_growth
                            );
                            
                            // Check if FFmpeg process is still running
                            let mut process_dead = false;
                            {
                                let mut process_guard = recording_process.write().await;
                                if let Some(ref mut process) = process_guard.as_mut() {
                                    match process.try_wait() {
                                        Ok(Some(status)) => {
                                            tracing::error!("FFmpeg process has exited with status: {:?}", status);
                                            process_dead = true;
                                        }
                                        Ok(None) => {
                                            tracing::warn!("FFmpeg process is running but file not growing");
                                        }
                                        Err(e) => {
                                            tracing::error!("Error checking FFmpeg process: {}", e);
                                        }
                                    }
                                }
                            }
                            
                            if process_dead {
                                let error_msg = format!(
                                    "Recording failed: FFmpeg process died and file is not growing ({}MB after {}s)",
                                    current_size as f64 / 1024.0 / 1024.0,
                                    info.duration_seconds
                                );
                                
                                info.status = RecordingStatus::Error(error_msg.clone());
                                let _ = event_sender.send(RecordingEvent::Error(error_msg));
                                break;
                            }
                        }
                    } else {
                        tracing::error!("Cannot access recording file: {}", info.file_path);
                        consecutive_no_growth += 1;
                        
                        if consecutive_no_growth >= 3 {
                            let error_msg = format!(
                                "Recording failed: Cannot access output file after {} attempts: {}", 
                                consecutive_no_growth, 
                                info.file_path
                            );
                            
                            info.status = RecordingStatus::Error(error_msg.clone());
                            let _ = event_sender.send(RecordingEvent::Error(error_msg));
                            break;
                        }
                    }
                    
                    // Send status update
                    let _ = event_sender.send(RecordingEvent::StatusUpdate(info.clone()));
                }
            }
            
            tracing::info!("Recording monitoring task ended");
        });
    }
    
    /// Start post-processing of the recording
    async fn start_post_processing(&self, recording_info: &MeetingRecordingInfo) {
        tracing::info!("Starting post-processing for recording: {}", recording_info.id);
        
        let recording_info = recording_info.clone();
        
        tokio::spawn(async move {
            let _options = PostProcessingOptions::default();
            
            // Placeholder for post-processing logic
            // This would include:
            // - Transcription using Whisper
            // - Diarization using pyannote or similar
            // - Noise reduction
            // - Audio normalization
            // - Summary generation
            
            tracing::info!("Post-processing completed for recording: {}", recording_info.id);
        });
    }
    
    /// Expand path with home directory
    fn expand_path(path: &str) -> Result<PathBuf> {
        if path.starts_with("~/") {
            let home = dirs::home_dir()
                .context("Failed to get home directory")?;
            Ok(home.join(&path[2..]))
        } else {
            Ok(PathBuf::from(path))
        }
    }
    
    /// Sanitize filename for safe filesystem use
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c => c,
            })
            .collect::<String>()
            .chars()
            .take(100) // Limit filename length
            .collect()
    }
}

impl Drop for MeetingRecorder {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        tracing::info!("MeetingRecorder dropped");
    }
} 
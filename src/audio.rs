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
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{sleep, Duration};
use std::fs;
use crate::config::Config;
use crate::types::AudioConfig;

pub struct AudioCapture {
    config: AudioConfig,
    temp_dir: PathBuf,
    current_buffer_file: Option<PathBuf>,
    buffer_process: Option<tokio::process::Child>,
    is_buffering: bool,
}

impl AudioCapture {
    pub async fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.audio.clone(),
            temp_dir: config.temp_dir.clone(),
            current_buffer_file: None,
            buffer_process: None,
            is_buffering: false,
        })
    }
    
    pub async fn start_buffering(&mut self) -> Result<()> {
        tracing::debug!("start_buffering called - is_buffering: {}, current_buffer_file: {:?}", 
                       self.is_buffering, self.current_buffer_file);
        
        if self.is_buffering {
            tracing::debug!("Already buffering, returning early");
            return Ok(());
        }
        
        let timestamp = chrono::Utc::now().timestamp_millis();
        let buffer_file = self.temp_dir.join(format!("buffer_{}.wav", timestamp));
        
        tracing::info!("Starting audio buffering to: {:?}", buffer_file);
        
        // Start FFmpeg buffering process
        let mut ffmpeg_cmd = Command::new("ffmpeg");
        
        // For audio-only capture with AVFoundation, we need to specify "none" for video
        // Format: "video_device:audio_device" where video_device can be "none" for audio-only
        let input_device = if self.config.device_index.starts_with(':') {
            // Audio-only device (e.g., ":2" becomes "none:2")
            format!("none{}", self.config.device_index)
        } else {
            // Keep as-is for other formats
            self.config.device_index.clone()
        };
        
        ffmpeg_cmd
            .args([
                "-f", "avfoundation",
                "-i", &input_device,
                "-ac", &self.config.channels.to_string(),
                "-ar", &self.config.sample_rate.to_string(),
                "-acodec", "pcm_s16le",
                "-y",
                &buffer_file.to_string_lossy(),
            ])
            .stdin(Stdio::piped())   // Enable stdin to send 'q' for graceful exit
            .stdout(Stdio::piped())  // Capture stdout to see what's happening
            .stderr(Stdio::piped()); // Capture stderr to see errors
        
        tracing::info!("FFmpeg command: ffmpeg -f avfoundation -i {} -ac {} -ar {} -acodec pcm_s16le -y {}", 
                      input_device, self.config.channels, self.config.sample_rate, buffer_file.display());
        
        let process = ffmpeg_cmd.spawn()
            .context("Failed to start FFmpeg buffering process")?;
        
        self.buffer_process = Some(process);
        self.current_buffer_file = Some(buffer_file.clone());
        self.is_buffering = true;
        
        tracing::debug!("Set buffering state - is_buffering: {}, buffer_file: {:?}", 
                       self.is_buffering, self.current_buffer_file);
        
        // Give FFmpeg a moment to start and check if it's working
        sleep(Duration::from_millis(1000)).await;
        
        // Check if the process is still running and if the file was created
        if let Some(ref mut process) = self.buffer_process {
            match process.try_wait() {
                Ok(Some(status)) => {
                    // Process exited, capture the error
                    let stderr = if let Some(stderr) = process.stderr.take() {
                        let mut error_output = Vec::new();
                        use tokio::io::AsyncReadExt;
                        let mut stderr_reader = stderr;
                        let _ = stderr_reader.read_to_end(&mut error_output).await;
                        String::from_utf8_lossy(&error_output).to_string()
                    } else {
                        "No error output".to_string()
                    };
                    
                    tracing::error!("FFmpeg process exited with status: {:?}, stderr: {}", status, stderr);
                    self.is_buffering = false;
                    self.buffer_process = None;
                    self.current_buffer_file = None;
                    
                    // Provide more helpful error message
                    let helpful_error = Self::create_helpful_error_message(&stderr, &input_device);
                    return Err(anyhow::anyhow!("FFmpeg audio capture failed: {}", helpful_error));
                }
                Ok(None) => {
                    // Process is still running
                    tracing::info!("FFmpeg buffering process started successfully");
                    
                    if !buffer_file.exists() {
                        tracing::warn!("Buffer file not created yet, FFmpeg may need more time");
                    }
                }
                Err(e) => {
                    tracing::error!("Error checking FFmpeg process status: {}", e);
                }
            }
        }
        
        // Auto-restart buffering every 60 seconds to prevent file corruption
        let _temp_dir = self.temp_dir.clone();
        let _device_index = self.config.device_index.clone();
        let _sample_rate = self.config.sample_rate;
        let _channels = self.config.channels;
        
        tokio::spawn(async move {
            sleep(Duration::from_secs(60)).await;
            // This will be handled by the main loop restarting
        });
        
        tracing::debug!("start_buffering completed successfully");
        Ok(())
    }
    
    pub async fn stop_buffering(&mut self) -> Result<()> {
        if !self.is_buffering {
            return Ok(());
        }
        
        self.is_buffering = false;
        
        // Gracefully stop the FFmpeg process
        if let Some(mut process) = self.buffer_process.take() {
            // Send 'q' to gracefully quit FFmpeg
            if let Some(mut stdin) = process.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(b"q\n").await;
                let _ = stdin.flush().await;
            }
            
            // Wait for process to exit, with a longer timeout for proper finalization
            tokio::select! {
                result = process.wait() => {
                    match result {
                        Ok(status) => {
                            tracing::info!("FFmpeg process exited gracefully with status: {:?}", status);
                        }
                        Err(e) => {
                            tracing::warn!("Error waiting for FFmpeg process: {}", e);
                        }
                    }
                }
                _ = sleep(Duration::from_secs(5)) => {
                    tracing::warn!("FFmpeg process didn't exit gracefully within 5 seconds, killing it");
                    let _ = process.kill().await;
                }
            }
        }
        
        // Give FFmpeg extra time to finalize the file
        sleep(Duration::from_millis(500)).await;
        
        // Clean up buffer file for regular stop (not extraction)
        if let Some(buffer_file) = self.current_buffer_file.take() {
            if buffer_file.exists() {
                let _ = fs::remove_file(buffer_file);
            }
        }
        
        Ok(())
    }
    
    pub async fn stop_buffering_for_extraction(&mut self) -> Result<()> {
        tracing::debug!("stop_buffering_for_extraction called - is_buffering: {}", self.is_buffering);
        
        if !self.is_buffering {
            tracing::debug!("Not buffering, returning early");
            return Ok(());
        }
        
        self.is_buffering = false;
        tracing::debug!("Set is_buffering to false");
        
        // Gracefully stop the FFmpeg process with extra care for file finalization
        if let Some(mut process) = self.buffer_process.take() {
            // Send 'q' to gracefully quit FFmpeg
            if let Some(mut stdin) = process.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(b"q\n").await;
                let _ = stdin.flush().await;
            }
            
            // Wait longer for process to exit properly
            tokio::select! {
                result = process.wait() => {
                    match result {
                        Ok(status) => {
                            tracing::info!("FFmpeg process exited for extraction with status: {:?}", status);
                        }
                        Err(e) => {
                            tracing::warn!("Error waiting for FFmpeg process: {}", e);
                        }
                    }
                }
                _ = sleep(Duration::from_secs(8)) => {
                    tracing::warn!("FFmpeg process didn't exit gracefully within 8 seconds, killing it");
                    let _ = process.kill().await;
                }
            }
        }
        
        // Give FFmpeg significant time to finalize the WAV file properly
        sleep(Duration::from_millis(1000)).await;
        
        // NOTE: We don't clear current_buffer_file here since we need it for extraction
        // The caller will handle cleanup of the old file
        
        tracing::debug!("stop_buffering_for_extraction completed - is_buffering: {}, current_buffer_file: {:?}", 
                       self.is_buffering, self.current_buffer_file);
        
        Ok(())
    }
    
    pub async fn extract_recent_buffer(&mut self, duration_seconds: u64) -> Result<Option<PathBuf>> {
        tracing::debug!("extract_recent_buffer called - is_buffering: {}, current_buffer_file: {:?}", 
                       self.is_buffering, self.current_buffer_file);
        
        if !self.is_buffering || self.current_buffer_file.is_none() {
            tracing::warn!("Cannot extract buffer - is_buffering: {}, current_buffer_file: {:?}", 
                          self.is_buffering, self.current_buffer_file);
            
            // Try to restart buffering if it's not running
            if !self.is_buffering {
                tracing::info!("Attempting to restart buffering...");
                match self.start_buffering().await {
                    Ok(()) => {
                        tracing::info!("Successfully restarted buffering");
                        // Wait a bit for the buffer to accumulate some audio
                        sleep(Duration::from_secs(2)).await;
                        if self.current_buffer_file.is_none() {
                            tracing::warn!("Still no buffer file after restart");
                            return Ok(None);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to restart buffering: {}", e);
                        return Ok(None);
                    }
                }
            } else {
                return Ok(None);
            }
        }
        
        let buffer_file_path = self.current_buffer_file.as_ref().unwrap().clone();
        
        if !buffer_file_path.exists() {
            tracing::warn!("Buffer file doesn't exist: {:?}", buffer_file_path);
            
            // Try to restart buffering
            tracing::info!("Attempting to restart buffering due to missing buffer file...");
            self.is_buffering = false;
            self.current_buffer_file = None;
            if let Some(mut process) = self.buffer_process.take() {
                let _ = process.kill().await;
            }
            
            match self.start_buffering().await {
                Ok(()) => {
                    tracing::info!("Successfully restarted buffering");
                    sleep(Duration::from_secs(2)).await;
                    return Ok(None); // Return None for this attempt, next one should work
                }
                Err(e) => {
                    tracing::error!("Failed to restart buffering: {}", e);
                    return Ok(None);
                }
            }
        }
        
        tracing::info!("Stopping buffering to capture most recent audio...");
        
        // Stop the buffering process to ensure we get the most recent audio
        // This prevents any timing issues where new audio is recorded while we're processing
        let was_buffering = self.is_buffering;
        let old_buffer_file = self.current_buffer_file.clone(); // Store the old buffer file path
        
        // Use a timeout for the stop operation to prevent hanging
        let stop_result = tokio::time::timeout(
            Duration::from_secs(10),
            self.stop_buffering_for_extraction()
        ).await;
        
        match stop_result {
            Ok(Ok(())) => {
                tracing::info!("Successfully stopped buffering for extraction");
            }
            Ok(Err(e)) => {
                tracing::error!("Error stopping buffering for extraction: {}", e);
                // Continue anyway, try to work with what we have
            }
            Err(_) => {
                tracing::error!("Timeout stopping buffering for extraction");
                // Force cleanup and try to continue
                self.is_buffering = false;
                if let Some(mut process) = self.buffer_process.take() {
                    let _ = process.kill().await;
                }
                // Continue anyway, try to work with what we have
            }
        }
        
        let timestamp = chrono::Utc::now().timestamp_millis();
        
        // Work directly with the buffer file since we've stopped recording
        let buffer_duration = match self.get_audio_duration(&buffer_file_path).await {
            Ok(duration) => duration,
            Err(e) => {
                tracing::warn!("Failed to get buffer duration directly: {}", e);
                
                // Try to fix the file using ffmpeg if it's incomplete
                tracing::info!("Attempting to fix potentially incomplete buffer file...");
                let fixed_file = self.temp_dir.join(format!("fixed_buffer_{}.wav", timestamp));
                
                let fix_result = tokio::time::timeout(
                    Duration::from_secs(10),
                    Command::new("ffmpeg")
                        .args([
                            "-i", &buffer_file_path.to_string_lossy(),
                            "-c", "copy",
                            "-y",
                            &fixed_file.to_string_lossy(),
                        ])
                        .output()
                ).await;
                
                match fix_result {
                    Ok(Ok(output)) if output.status.success() => {
                        // Try to get duration from the fixed file
                        match self.get_audio_duration(&fixed_file).await {
                            Ok(duration) => {
                                tracing::info!("Successfully fixed buffer file, duration: {:.1}s", duration);
                                // Replace the original buffer file with the fixed one
                                let _ = fs::remove_file(&buffer_file_path);
                                let _ = fs::rename(&fixed_file, &buffer_file_path);
                                duration
                            }
                            Err(e) => {
                                tracing::warn!("Fixed file is also unreadable: {}", e);
                                let _ = fs::remove_file(&fixed_file);
                                // Restart buffering if it was running
                                if was_buffering {
                                    let _ = self.start_buffering().await;
                                }
                                return Ok(None);
                            }
                        }
                    }
                    Ok(Ok(output)) => {
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        tracing::warn!("Failed to fix buffer file: {}", error_msg);
                        // Restart buffering if it was running
                        if was_buffering {
                            let _ = self.start_buffering().await;
                        }
                        return Ok(None);
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Failed to run ffmpeg to fix buffer file: {}", e);
                        // Restart buffering if it was running
                        if was_buffering {
                            let _ = self.start_buffering().await;
                        }
                        return Ok(None);
                    }
                    Err(_) => {
                        tracing::warn!("Timeout waiting for ffmpeg to fix buffer file");
                        let _ = fs::remove_file(&fixed_file);
                        // Restart buffering if it was running
                        if was_buffering {
                            let _ = self.start_buffering().await;
                        }
                        return Ok(None);
                    }
                }
            }
        };
        
        if buffer_duration < 0.5 {
            tracing::warn!("Buffer too short: {:.1}s", buffer_duration);
            // Restart buffering if it was running
            if was_buffering {
                let _ = self.start_buffering().await;
            }
            return Ok(None);
        }
        
        // Calculate extraction parameters - get the most recent audio
        let capture_duration = (duration_seconds as f64).min(buffer_duration);
        let start_position = (buffer_duration - capture_duration).max(0.0);
        
        tracing::info!(
            "Extracting most recent {:.1}s from buffer (total: {:.1}s, start: {:.1}s)",
            capture_duration, buffer_duration, start_position
        );
        
        // Create output file
        let captured_file = self.temp_dir.join(format!("captured_{}.wav", timestamp));
        
        // Extract the most recent audio segment with timeout
        let extract_result = tokio::time::timeout(
            Duration::from_secs(15),
            Command::new("ffmpeg")
                .args([
                    "-i", &buffer_file_path.to_string_lossy(),
                    "-ss", &start_position.to_string(),
                    "-t", &capture_duration.to_string(),
                    "-c", "copy",
                    "-y",
                    &captured_file.to_string_lossy(),
                ])
                .output()
        ).await;
        
        // Always try to restart buffering first, before checking extraction result
        if was_buffering {
            tracing::info!("Restarting buffering after extraction...");
            let restart_result = tokio::time::timeout(
                Duration::from_secs(5),
                self.start_buffering()
            ).await;
            
            match restart_result {
                Ok(Ok(())) => {
                    tracing::info!("Successfully restarted buffering after extraction");
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to restart buffering after extraction: {}", e);
                    // Continue with extraction result anyway
                }
                Err(_) => {
                    tracing::error!("Timeout restarting buffering after extraction");
                    // Continue with extraction result anyway
                }
            }
        }
        
        // Check extraction result
        let output = match extract_result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                tracing::error!("FFmpeg extraction failed: {}", e);
                return Err(anyhow::anyhow!("FFmpeg extraction failed: {}", e));
            }
            Err(_) => {
                tracing::error!("FFmpeg extraction timed out");
                return Err(anyhow::anyhow!("FFmpeg extraction timed out"));
            }
        };
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            tracing::error!("FFmpeg extraction failed: {}", error_msg);
            return Err(anyhow::anyhow!("FFmpeg extraction failed: {}", error_msg));
        }
        
        // Verify the extracted file
        if !captured_file.exists() {
            return Err(anyhow::anyhow!("Extracted file was not created"));
        }
        
        let extracted_duration = self.get_audio_duration(&captured_file).await?;
        tracing::info!("Successfully extracted {:.1}s of most recent audio", extracted_duration);
        
        // Clean up the OLD buffer file after successful extraction
        if let Some(buffer_file) = old_buffer_file {
            if buffer_file.exists() {
                let _ = fs::remove_file(buffer_file);
                tracing::debug!("Cleaned up old buffer file after extraction");
            }
        }
        
        Ok(Some(captured_file))
    }
    
    async fn get_audio_duration(&self, file_path: &PathBuf) -> Result<f64> {
        let output = Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-show_entries", "format=duration",
                "-of", "csv=p=0",
                &file_path.to_string_lossy(),
            ])
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(anyhow::anyhow!("ffprobe failed"));
        }
        
        let duration_str = String::from_utf8_lossy(&output.stdout);
        let duration: f64 = duration_str.trim().parse()
            .context("Failed to parse audio duration")?;
        
        Ok(duration)
    }
    
    fn create_helpful_error_message(stderr: &str, input_device: &str) -> String {
        let mut message = String::new();
        
        if stderr.contains("Input/output error") {
            message.push_str(&format!(
                "Cannot access audio device '{}'. This usually means:\n\
                • The device doesn't exist or isn't available\n\
                • Microphone permissions not granted\n\
                • Another application is using the device\n\n",
                input_device
            ));
        }
        
        if stderr.contains("Permission denied") || stderr.contains("Operation not permitted") {
            message.push_str(
                "Permission denied. Please:\n\
                • Grant microphone access to your terminal app\n\
                • Go to System Preferences → Security & Privacy → Privacy → Microphone\n\
                • Add your terminal app to the allowed list\n\n"
            );
        }
        
        if stderr.contains("Selected framerate") || stderr.contains("not supported by the device") {
            message.push_str(
                "Device format issue. The audio device doesn't support the requested format.\n\n"
            );
        }
        
        if stderr.contains("AVFoundation") && stderr.contains("list_devices") {
            message.push_str(
                "Device enumeration failed. This might be a permissions issue.\n\n"
            );
        }
        
        // Always add troubleshooting steps
        message.push_str("Troubleshooting steps:\n");
        message.push_str("1. Check available devices: ffmpeg -f avfoundation -list_devices true -i \"\"\n");
        message.push_str("2. Update AUDIO_DEVICE in .env file if needed\n");
        message.push_str("3. Run setup: ./target/release/meeting-assistant setup\n");
        message.push_str("4. Check permissions in System Preferences\n\n");
        
        message.push_str("Original error:\n");
        message.push_str(stderr);
        
        message
    }
    
    pub async fn cleanup_temp_files(&self) -> Result<()> {
        let temp_dir = &self.temp_dir;
        
        if !temp_dir.exists() {
            return Ok(());
        }
        
        let mut entries = fs::read_dir(temp_dir)?;
        while let Some(entry) = entries.next() {
            let entry = entry?;
            let path = entry.path();
            
            if let Some(filename) = path.file_name() {
                let filename_str = filename.to_string_lossy();
                
                // Remove old buffer, captured, and other temporary audio files
                if filename_str.starts_with("buffer_") ||
                   filename_str.starts_with("captured_") ||
                   filename_str.starts_with("buffer_copy_") ||
                   filename_str.starts_with("fixed_buffer_") ||
                   filename_str.starts_with("screenshot_") {
                    
                    // Check if file is older than 1 hour
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            let age = std::time::SystemTime::now()
                                .duration_since(modified)
                                .unwrap_or_default();
                            
                            if age.as_secs() > 3600 { // 1 hour
                                let _ = fs::remove_file(&path);
                                tracing::debug!("Cleaned up old temp file: {:?}", path);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        // Clean up on drop
        if let Some(mut process) = self.buffer_process.take() {
            let _ = process.start_kill();
        }
        
        if let Some(buffer_file) = self.current_buffer_file.take() {
            if buffer_file.exists() {
                let _ = fs::remove_file(buffer_file);
            }
        }
    }
} 
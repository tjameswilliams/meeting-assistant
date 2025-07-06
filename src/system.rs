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
use colored::*;
use std::fs;
use crate::types::WhisperBackend;

pub struct SystemInfo {
    temp_dir: PathBuf,
    whisper_backend: Option<WhisperBackend>,
    whisper_command: Option<String>,
}

impl SystemInfo {
    pub async fn new() -> Result<Self> {
        let mut system_info = Self {
            temp_dir: std::env::temp_dir(),
            whisper_backend: None,
            whisper_command: None,
        };
        
        // Initialize whisper backend detection
        let (whisper_ready, backend, command) = system_info.check_whisper_available().await;
        system_info.whisper_backend = backend;
        system_info.whisper_command = command;
        
        if whisper_ready {
            tracing::info!("Whisper backend initialized: {}", system_info.get_display_name());
        } else {
            tracing::warn!("No whisper backend available - will use OpenAI API");
        }
        
        Ok(system_info)
    }
    
    pub async fn check_system_status(&mut self) -> Result<()> {
        // Check audio setup
        let audio_ready = self.check_audio_devices().await.unwrap_or(true); // Assume ready if can't check
        
        // Check local Whisper
        let (whisper_ready, backend, command) = self.check_whisper_available().await;
        self.whisper_backend = backend;
        self.whisper_command = command;
        
        // Display status
        let audio_status = if audio_ready { "✅".green() } else { "❌".red() };
        let whisper_status = if whisper_ready {
            format!("✅ ({})", self.get_display_name()).green()
        } else {
            "❌".red()
        };
        let openai_status = if std::env::var("OPENAI_API_KEY").is_ok() {
            "✅".green()
        } else {
            "❌".red()
        };
        
        println!("   🎤 Audio Setup: {}", audio_status);
        println!("   🗣️  Local Whisper: {}", whisper_status);
        println!("   🔑 OpenAI API: {}", openai_status);
        println!();
        
        if !audio_ready {
            println!("⚠️  Audio not ready - ensure BlackHole is configured");
        }
        
        if !whisper_ready {
            println!("⚠️  Local Whisper not found - will fallback to OpenAI API (higher latency)");
            println!("   ULTRA-FAST: whisper.cpp");
            println!("     • brew install whisper-cpp");
            println!("     • Download models: bash ./models/download-ggml-model.sh base.en");
            println!("   FAST: brew install whisper (no Python deps)");
            println!("   GOOD: pip install faster-whisper (no NumPy conflicts)");
            println!("   FALLBACK: pip install openai-whisper (may have NumPy 2.x issues)");
        } else if matches!(self.whisper_backend, Some(WhisperBackend::WhisperCpp)) {
            println!("💡 whisper.cpp detected! If models are missing:");
            println!("   • mkdir -p /opt/homebrew/share/whisper.cpp/models");
            println!("   • cd /opt/homebrew/share/whisper.cpp/models");
            println!("   • curl -L -o ggml-base.en.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin");
        }
        
        println!("{}", "🟢 Ready! 'A' for audio, 'S' for clipboard, 'Q' for combined, 'W' for screenshot, 'R' to cancel...".green().bold());
        println!();
        
        // Show troubleshooting and features info
        self.show_troubleshooting_info().await?;
        
        Ok(())
    }
    
    async fn check_audio_devices(&self) -> Result<bool> {
        let output = Command::new("system_profiler")
            .args(["SPAudioDataType", "-json"])
            .output()
            .await;
        
        match output {
            Ok(output) => {
                if output.status.success() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    
                    // Check for BlackHole, Tim's Input/Output, or Aggregate devices
                    let has_audio_setup = output_str.to_lowercase().contains("blackhole") ||
                        output_str.contains("Tim's Input") ||
                        output_str.contains("Tim's Output") ||
                        output_str.to_lowercase().contains("aggregate");
                    
                    Ok(has_audio_setup)
                } else {
                    Ok(false)
                }
            }
            Err(_) => {
                // Could not check devices, assume ready
                Ok(true)
            }
        }
    }
    
    async fn check_whisper_available(&self) -> (bool, Option<WhisperBackend>, Option<String>) {
        // Check in order of preference:
        // 1. whisper.cpp (ultra-fast C++ implementation)
        // 2. Homebrew whisper (fast, no Python deps)
        // 3. faster-whisper (fast, no NumPy conflicts)
        // 4. Standard Python whisper (potential NumPy issues)
        
        if let Some(command) = self.check_cpp_whisper().await {
            tracing::info!("Found whisper.cpp backend: {}", command);
            return (true, Some(WhisperBackend::WhisperCpp), Some(command));
        }
        
        if self.check_brew_whisper().await {
            tracing::info!("Found Homebrew whisper backend");
            return (true, Some(WhisperBackend::WhisperBrew), Some("whisper".to_string()));
        }
        
        if self.check_python_whisper("faster-whisper", &["--help"]).await {
            tracing::info!("Found faster-whisper backend");
            return (true, Some(WhisperBackend::FasterWhisper), Some("faster-whisper".to_string()));
        }
        
        if self.check_python_whisper("whisper", &["--help"]).await {
            tracing::info!("Found standard Python whisper backend");
            return (true, Some(WhisperBackend::StandardWhisper), Some("whisper".to_string()));
        }
        
        tracing::warn!("No working whisper backend found");
        (false, None, None)
    }
    
    async fn check_cpp_whisper(&self) -> Option<String> {
        // Check for whisper.cpp - can be called 'whisper-cli', 'whisper-cpp', or 'whisper'
        if self.check_command("whisper-cli", &["--help"]).await {
            return Some("whisper-cli".to_string());
        }
        
        if self.check_command("whisper-cpp", &["--help"]).await {
            return Some("whisper-cpp".to_string());
        }
        
        // Check if 'whisper' is actually whisper.cpp
        if self.check_command("whisper", &["--help"]).await {
            if self.check_if_cpp_version().await {
                return Some("whisper".to_string());
            }
        }
        
        None
    }
    
    async fn check_if_cpp_version(&self) -> bool {
        let output = Command::new("whisper")
            .args(["--help"])
            .output()
            .await;
        
        if let Ok(output) = output {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout).to_lowercase();
                
                // whisper.cpp typically mentions specific indicators
                return output_str.contains("ggml") ||
                    output_str.contains("file0.wav file1.wav") ||
                    output_str.contains("--processors") ||
                    output_str.contains("--threads") ||
                    (output_str.contains("usage: whisper") && !output_str.contains("--model_size"));
            }
        }
        
        false
    }
    
    async fn check_brew_whisper(&self) -> bool {
        // First check if whisper command exists and get its path
        let which_output = Command::new("which")
            .args(["whisper"])
            .output()
            .await;
        
        if let Ok(output) = which_output {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                let is_brew_path = path.contains("/opt/homebrew") ||
                    path.contains("/usr/local/bin") ||
                    (!path.contains("python") && !path.contains("site-packages"));
                
                if is_brew_path {
                    // Test if the command actually works (not affected by NumPy issues)
                    return self.check_command("whisper", &["--version"]).await;
                }
            }
        }
        
        false
    }
    
    async fn check_command(&self, command: &str, args: &[&str]) -> bool {
        let result = Command::new(command)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        
        matches!(result, Ok(status) if status.success())
    }
    
    async fn check_python_whisper(&self, command: &str, args: &[&str]) -> bool {
        let result = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;
        
        match result {
            Ok(output) => {
                if output.status.success() {
                    true
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    // Check if the error is related to NumPy compatibility
                    if stderr.contains("NumPy") || stderr.contains("numpy") {
                        tracing::warn!("Python whisper command '{}' failed due to NumPy compatibility issues", command);
                        false
                    } else {
                        // Other errors might be temporary, so we'll consider it available
                        true
                    }
                }
            }
            Err(_) => false,
        }
    }
    
    fn get_display_name(&self) -> String {
        match &self.whisper_backend {
            Some(WhisperBackend::WhisperCpp) => "whisper.cpp".to_string(),
            Some(WhisperBackend::WhisperBrew) => "brew".to_string(),
            Some(WhisperBackend::FasterWhisper) => "faster-whisper".to_string(),
            Some(WhisperBackend::StandardWhisper) => "python".to_string(),
            Some(WhisperBackend::OpenAIAPI) => "openai-api".to_string(),
            None => "none".to_string(),
        }
    }
    
    async fn show_troubleshooting_info(&self) -> Result<()> {
        println!("{}", "🔧 Troubleshooting:".yellow().bold());
        println!("{}", "   • If double-tap 'A' isn't detected, check System Preferences > Security & Privacy > Accessibility".bright_black());
        println!("{}", "   • Add Terminal (or your terminal app) to the accessibility list".bright_black());
        println!("{}", "   • You should see debug messages when pressing 'A'".bright_black());
        println!();
        
        println!("{}", "✨ Features:".blue().bold());
        println!("{}", "   • Smart buffer capture - press 'A' to capture recent audio".white());
        println!("{}", "   • Clipboard code analysis - press 'S' to analyze copied code".white());
        println!("{}", "   • Combined mode - 'Q' combines audio + clipboard for commented code solutions".white());
        println!("{}", "   • Screenshot analysis - 'W' captures active window + audio for visual reasoning".white());
        println!("{}", "   • Multi-platform Whisper support (whisper.cpp, Homebrew, faster-whisper, Python)".white());
        println!("{}", "   • Ultra-low latency local transcription".white());
        println!("{}", "   • Smart question classification (portfolio vs technical vs behavioral)".white());
        println!("{}", "   • Conversation memory - follow-up questions build on previous context".white());
        println!("{}", "   • Session history with double-tap 'H', reset context with double-tap 'R'".white());
        println!("{}", "   • Automatic fallback to OpenAI API if local Whisper fails".white());
        println!("{}", "   • Real-time streaming responses with markdown formatting".white());
        println!("{}", "   • Syntax highlighting for code blocks (JavaScript, Python, Java, SQL, etc.)".white());
        println!("{}", "   • Multi-language code detection and analysis with interview insights".white());
        println!("{}", "   • Code memory system - reference previously analyzed code in follow-up questions".white());
        println!("{}", "   • Quick cancel - double-tap 'R' to instantly cancel any running request".white());
        println!("{}", "   • Native Rust performance - 10x faster than Node.js version".white());
        println!();
        
        Ok(())
    }
    
    pub async fn transcribe_audio(&self, audio_file: &PathBuf) -> Result<Option<String>> {
        tracing::debug!("transcribe_audio called with file: {:?}", audio_file);
        tracing::debug!("Backend: {:?}, Command: {:?}", self.whisper_backend, self.whisper_command);
        
        // Try local Whisper first for better latency
        if let (Some(backend), Some(command)) = (&self.whisper_backend, &self.whisper_command) {
            tracing::info!("Attempting local transcription with {}", self.get_display_name());
            match self.transcribe_with_local_whisper(audio_file, backend, command).await {
                Ok(transcript) if !transcript.trim().is_empty() => {
                    tracing::info!("Local {} transcription successful", self.get_display_name());
                    return Ok(Some(transcript));
                }
                Err(e) => {
                    tracing::warn!("Local {} failed, falling back to OpenAI API: {}", self.get_display_name(), e);
                }
                Ok(transcript) => {
                    tracing::warn!("Local {} returned empty transcript: '{}'", self.get_display_name(), transcript);
                }
            }
        } else {
            tracing::warn!("No local whisper backend available - backend: {:?}, command: {:?}", self.whisper_backend, self.whisper_command);
        }
        
        // Return None to indicate local transcription failed - caller should handle OpenAI fallback
        Ok(None)
    }
    
    async fn transcribe_with_local_whisper(
        &self,
        audio_file: &PathBuf,
        backend: &WhisperBackend,
        command: &str,
    ) -> Result<String> {
        match backend {
            WhisperBackend::WhisperCpp => self.transcribe_with_cpp_whisper(audio_file, command).await,
            WhisperBackend::WhisperBrew => self.transcribe_with_brew_whisper(audio_file).await,
            WhisperBackend::FasterWhisper => self.transcribe_with_faster_whisper(audio_file).await,
            WhisperBackend::StandardWhisper => self.transcribe_with_standard_whisper(audio_file).await,
            WhisperBackend::OpenAIAPI => Err(anyhow::anyhow!("OpenAI API should not be called from this method")),
        }
    }
    
    async fn transcribe_with_cpp_whisper(&self, audio_file: &PathBuf, command: &str) -> Result<String> {
        // Find available model file
        let model_path = self.find_whisper_cpp_model().await
            .context("No whisper.cpp model files found")?;
        
        tracing::info!("Using whisper.cpp model: {}", model_path.display());
        
        let args = [
            "-m", &model_path.to_string_lossy(),
            "-f", &audio_file.to_string_lossy(),
            "-nt",  // No timestamps
            "-l", "en",  // Language
            "-t", "4",   // Use 4 threads
            "-otxt",  // Force text output
        ];
        
        tracing::debug!("Running command: {} {}", command, args.join(" "));
        
        let output = Command::new(command)
            .args(args)
            .output()
            .await?;
        
        tracing::debug!("Command exit status: {}", output.status);
        tracing::debug!("Command stdout length: {}", output.stdout.len());
        tracing::debug!("Command stderr length: {}", output.stderr.len());
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            tracing::error!("Command stderr: {}", error_msg);
            return Err(anyhow::anyhow!("{} failed: {}", command, error_msg));
        }
        
        // With -otxt flag, whisper-cli creates a text file instead of outputting to stdout
        // Try to read the output file first
        let audio_dir = audio_file.parent().unwrap_or(&self.temp_dir);
        let base_name = audio_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let transcript_file = audio_dir.join(format!("{}.txt", base_name));
        
        tracing::debug!("Looking for transcript file: {:?}", transcript_file);
        tracing::debug!("Transcript file exists: {}", transcript_file.exists());
        
        if transcript_file.exists() {
            let transcript = fs::read_to_string(&transcript_file)?;
            tracing::debug!("Raw transcript from file: '{}'", transcript);
            // Clean up the output file
            let _ = fs::remove_file(transcript_file);
            let cleaned_transcript = transcript.trim().to_string();
            
            if !cleaned_transcript.is_empty() {
                tracing::info!("Successfully read transcript from file: '{}'", cleaned_transcript);
                return Ok(cleaned_transcript);
            } else {
                tracing::warn!("Transcript file was empty after trimming");
            }
        } else {
            tracing::warn!("Expected transcript file does not exist: {:?}", transcript_file);
        }
        
        // Fallback to stdout parsing in case file output didn't work
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // Extract transcription from stdout (filter out system info)
        let transcript_lines: Vec<&str> = stdout
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() &&
                !trimmed.starts_with("whisper_") &&
                !trimmed.starts_with("ggml_") &&
                !trimmed.starts_with("main:") &&
                !trimmed.contains("load time") &&
                !trimmed.contains("total time") &&
                !trimmed.contains("threads") &&
                trimmed.len() > 10 &&
                trimmed.chars().any(|c| c.is_alphabetic())
            })
            .collect();
        
        let transcript = transcript_lines.join(" ").trim().to_string();
        
        if transcript.is_empty() {
            return Err(anyhow::anyhow!("No transcript extracted from {} output", command));
        }
        
        Ok(transcript)
    }
    
    async fn transcribe_with_brew_whisper(&self, audio_file: &PathBuf) -> Result<String> {
        let output_dir = audio_file.parent().unwrap_or(&self.temp_dir);
        
        let output = Command::new("whisper")
            .args([
                &audio_file.to_string_lossy(),
                "--model", "base",
                "--language", "en",
                "--output_format", "txt",
                "--output_dir", &output_dir.to_string_lossy(),
                "--verbose", "False",
            ])
            .output()
            .await?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Brew whisper failed: {}", error_msg));
        }
        
        // Read the output file
        let base_name = audio_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let transcript_file = output_dir.join(format!("{}.txt", base_name));
        
        if !transcript_file.exists() {
            return Err(anyhow::anyhow!("Brew whisper output file not found"));
        }
        
        let transcript = fs::read_to_string(&transcript_file)?;
        
        // Clean up the output file
        let _ = fs::remove_file(transcript_file);
        
        Ok(transcript.trim().to_string())
    }
    
    async fn transcribe_with_faster_whisper(&self, audio_file: &PathBuf) -> Result<String> {
        let output_dir = audio_file.parent().unwrap_or(&self.temp_dir);
        
        let output = Command::new("faster-whisper")
            .args([
                &audio_file.to_string_lossy(),
                "--model", "base",
                "--language", "en",
                "--output_format", "txt",
                "--output_dir", &output_dir.to_string_lossy(),
            ])
            .output()
            .await?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("faster-whisper failed: {}", error_msg));
        }
        
        // Check for stdout output first
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stdout.is_empty() {
            return Ok(stdout);
        }
        
        // Otherwise check for file output
        let base_name = audio_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let transcript_file = output_dir.join(format!("{}.txt", base_name));
        
        if transcript_file.exists() {
            let transcript = fs::read_to_string(&transcript_file)?;
            let _ = fs::remove_file(transcript_file);
            return Ok(transcript.trim().to_string());
        }
        
        Err(anyhow::anyhow!("No output from faster-whisper"))
    }
    
    async fn transcribe_with_standard_whisper(&self, audio_file: &PathBuf) -> Result<String> {
        let output_dir = audio_file.parent().unwrap_or(&self.temp_dir);
        
        let output = Command::new("whisper")
            .args([
                &audio_file.to_string_lossy(),
                "--model", "base",
                "--language", "en",
                "--output_format", "txt",
                "--output_dir", &output_dir.to_string_lossy(),
                "--verbose", "False",
            ])
            .output()
            .await?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Standard whisper failed: {}", error_msg));
        }
        
        // Read the output file
        let base_name = audio_file.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let transcript_file = output_dir.join(format!("{}.txt", base_name));
        
        if !transcript_file.exists() {
            return Err(anyhow::anyhow!("Standard whisper output file not found"));
        }
        
        let transcript = fs::read_to_string(&transcript_file)?;
        
        // Clean up the output file
        let _ = fs::remove_file(transcript_file);
        
        Ok(transcript.trim().to_string())
    }
    
    async fn find_whisper_cpp_model(&self) -> Option<PathBuf> {
        let possible_paths = [
            "/opt/homebrew/share/whisper.cpp/models/ggml-base.en.bin",
            "/opt/homebrew/share/whisper.cpp/models/ggml-base.bin",
            "/usr/local/share/whisper.cpp/models/ggml-base.en.bin",
            "/usr/local/share/whisper.cpp/models/ggml-base.bin",
            "./models/ggml-base.en.bin",
            "./models/ggml-base.bin",
        ];
        
        for path_str in possible_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
        
        // Check in user home directory
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            for model in ["ggml-base.en.bin", "ggml-base.bin"] {
                let path = home_path.join(".whisper.cpp/models").join(model);
                if path.exists() {
                    return Some(path);
                }
            }
        }
        
        None
    }
    
    pub async fn capture_active_window(&self) -> Result<Option<PathBuf>> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let screenshot_path = self.temp_dir.join(format!("screenshot_{}.png", timestamp));
        
        tracing::info!("Capturing active window screenshot...");
        
        // Try to get the frontmost window ID first
        let window_id_output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get the id of the first window of the first application process whose frontmost is true",
            ])
            .output()
            .await;
        
        let capture_result = if let Ok(output) = window_id_output {
            if output.status.success() {
                let window_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !window_id.is_empty() {
                    // Capture specific window by ID
                    Command::new("screencapture")
                        .args([
                            "-l", &window_id,
                            "-x",  // Don't play sounds
                            &screenshot_path.to_string_lossy(),
                        ])
                        .status()
                        .await
                } else {
                    // Fallback to interactive capture
                    Command::new("screencapture")
                        .args([
                            "-w",  // Capture window interactively
                            "-x",  // Don't play sounds
                            "-T", "1",  // Wait 1 second then capture frontmost
                            &screenshot_path.to_string_lossy(),
                        ])
                        .status()
                        .await
                }
            } else {
                // Fallback to interactive capture
                Command::new("screencapture")
                    .args([
                        "-w",  // Capture window interactively
                        "-x",  // Don't play sounds
                        &screenshot_path.to_string_lossy(),
                    ])
                    .status()
                    .await
            }
        } else {
            // Fallback to simple screencapture
            Command::new("screencapture")
                .args([
                    "-w",  // Capture window interactively
                    "-x",  // Don't play sounds
                    &screenshot_path.to_string_lossy(),
                ])
                .status()
                .await
        };
        
        match capture_result {
            Ok(status) if status.success() => {
                if screenshot_path.exists() {
                    tracing::info!("Screenshot captured successfully");
                    Ok(Some(screenshot_path))
                } else {
                    Err(anyhow::anyhow!("Screenshot file was not created"))
                }
            }
            Ok(status) => {
                Err(anyhow::anyhow!("Screenshot capture failed with exit code: {}", status.code().unwrap_or(-1)))
            }
            Err(e) => {
                Err(anyhow::anyhow!("Screenshot capture error: {}", e))
            }
        }
    }
} 
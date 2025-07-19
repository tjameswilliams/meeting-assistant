#!/usr/bin/env rust-script

/*
 * Meeting Assistant CLI - Interactive Transcript Generator
 * 
 * This script allows users to:
 * - Browse recent meeting audio files
 * - Select which meeting to transcribe
 * - Choose diarization provider (ElevenLabs, Whisper+PyAnnote, etc.)
 * - Generate and save transcripts with speaker identification
 */

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

// Audio file extensions to search for
const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "m4a", "flac", "aac", "ogg"];

// Colors for terminal output
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const BLUE: &str = "\x1b[34m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";

#[derive(Debug, Clone)]
struct MeetingAudio {
    path: PathBuf,
    name: String,
    size: u64,
    modified: SystemTime,
    duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiarizationProvider {
    id: String,
    name: String,
    description: String,
    requirements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptSegment {
    start_time: f32,
    end_time: f32,
    speaker_id: String,
    text: String,
    confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptResult {
    provider: String,
    segments: Vec<TranscriptSegment>,
    speakers: Vec<String>,
    total_duration: f32,
    generated_at: DateTime<Utc>,
}

fn main() -> Result<()> {
    println!("{}{}🎙️  Meeting Assistant - Interactive Transcript Generator{}", BOLD, CYAN, RESET);
    println!("{}========================================================{}", CYAN, RESET);
    println!();

    // Check if we're in the right directory
    if !Path::new("Cargo.toml").exists() {
        eprintln!("{}❌ Error: Please run this script from the meeting-assistant project root directory{}", RED, RESET);
        std::process::exit(1);
    }

    // Find meeting audio files
    let audio_files = find_meeting_audios()?;
    
    if audio_files.is_empty() {
        println!("{}⚠️  No meeting audio files found.{}", YELLOW, RESET);
        println!("Looking in common locations:");
        println!("  • ~/.meeting-assistant/meetings/");
        println!("  • ~/.meeting-assistant/temp/");
        println!("  • ./meetings/");
        println!("  • ./temp/");
        println!();
        println!("Make sure you have recorded some meetings first!");
        return Ok(());
    }

    // Show available audio files
    let selected_audio = select_audio_file(&audio_files)?;
    
    // Get available diarization providers
    let providers = get_available_providers()?;
    
    // Select provider
    let selected_provider = select_provider(&providers)?;
    
    // Generate transcript
    println!("{}🔄 Generating transcript...{}", BLUE, RESET);
    let transcript = generate_transcript(&selected_audio, &selected_provider)?;
    
    // Save and display transcript
    save_transcript(&transcript, &selected_audio, &selected_provider)?;
    display_transcript(&transcript)?;
    
    Ok(())
}

fn find_meeting_audios() -> Result<Vec<MeetingAudio>> {
    let mut audio_files = Vec::new();
    
    // Common locations where meeting audios might be stored
    let search_paths = vec![
        dirs::home_dir().unwrap_or_default().join(".meeting-assistant/meetings"),
        dirs::home_dir().unwrap_or_default().join(".meeting-assistant/temp"),
        PathBuf::from("meetings"),
        PathBuf::from("temp"),
        PathBuf::from("recordings"),
        dirs::home_dir().unwrap_or_default().join("Downloads"),
    ];
    
    for search_path in search_paths {
        if search_path.exists() {
            find_audio_files_in_directory(&search_path, &mut audio_files)?;
        }
    }
    
    // Sort by modification time (newest first)
    audio_files.sort_by(|a, b| b.modified.cmp(&a.modified));
    
    // Take only the last 5 files
    audio_files.truncate(5);
    
    Ok(audio_files)
}

fn find_audio_files_in_directory(dir: &Path, audio_files: &mut Vec<MeetingAudio>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    
    let entries = fs::read_dir(dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if let Some(ext_str) = extension.to_str() {
                    if AUDIO_EXTENSIONS.contains(&ext_str.to_lowercase().as_str()) {
                        let metadata = entry.metadata()?;
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        
                        // Get audio duration if ffprobe is available
                        let duration = get_audio_duration(&path);
                        
                        audio_files.push(MeetingAudio {
                            path,
                            name,
                            size: metadata.len(),
                            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                            duration,
                        });
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn get_audio_duration(path: &Path) -> Option<String> {
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-show_entries", "format=duration",
            "-of", "csv=p=0",
            path.to_str()?,
        ])
        .output()
        .ok()?;
    
    if output.status.success() {
        let duration_str = String::from_utf8_lossy(&output.stdout);
        let duration_secs: f64 = duration_str.trim().parse().ok()?;
        let minutes = (duration_secs / 60.0) as u32;
        let seconds = (duration_secs % 60.0) as u32;
        Some(format!("{}:{:02}", minutes, seconds))
    } else {
        None
    }
}

fn select_audio_file(audio_files: &[MeetingAudio]) -> Result<MeetingAudio> {
    println!("{}📁 Found {} recent meeting audio files:{}", BOLD, audio_files.len(), RESET);
    println!();
    
    for (i, audio) in audio_files.iter().enumerate() {
        let size_mb = audio.size as f64 / 1_048_576.0;
        let modified = DateTime::<Utc>::from(audio.modified);
        let duration_str = audio.duration.as_deref().unwrap_or("Unknown");
        
        println!("{}{}. {}{}", GREEN, i + 1, BOLD, audio.name);
        println!("   {}📅 Modified: {}{}", BLUE, modified.format("%Y-%m-%d %H:%M:%S UTC"), RESET);
        println!("   {}📊 Size: {:.1} MB, Duration: {}{}", BLUE, size_mb, duration_str, RESET);
        println!("   {}📍 Path: {}{}", BLUE, audio.path.display(), RESET);
        println!();
    }
    
    loop {
        print!("{}Select audio file to transcribe (1-{}): {}", YELLOW, audio_files.len(), RESET);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().parse::<usize>() {
            Ok(choice) if choice >= 1 && choice <= audio_files.len() => {
                return Ok(audio_files[choice - 1].clone());
            }
            _ => {
                println!("{}❌ Invalid choice. Please enter a number between 1 and {}.{}", RED, audio_files.len(), RESET);
            }
        }
    }
}

fn get_available_providers() -> Result<Vec<DiarizationProvider>> {
    let mut providers = Vec::new();
    
    // Check for ElevenLabs
    if std::env::var("ELEVENLABS_API_KEY").is_ok() {
        providers.push(DiarizationProvider {
            id: "elevenlabs".to_string(),
            name: "ElevenLabs Scribe v1".to_string(),
            description: "Cloud-based, highest quality diarization with up to 32 speakers".to_string(),
            requirements: vec!["ElevenLabs API key".to_string()],
        });
    } else {
        providers.push(DiarizationProvider {
            id: "elevenlabs".to_string(),
            name: "ElevenLabs Scribe v1 (API key required)".to_string(),
            description: "Cloud-based, highest quality diarization with up to 32 speakers".to_string(),
            requirements: vec!["ElevenLabs API key (not configured)".to_string()],
        });
    }
    
    // Check for Whisper + PyAnnote
    if check_python_dependency("whisper") {
        if check_python_dependency("pyannote.audio") {
            providers.push(DiarizationProvider {
                id: "whisper_pyannote".to_string(),
                name: "Whisper + PyAnnote (Full)".to_string(),
                description: "Local processing with OpenAI Whisper and PyAnnote diarization".to_string(),
                requirements: vec!["Python", "whisper", "pyannote.audio".to_string()],
            });
        } else {
            providers.push(DiarizationProvider {
                id: "whisper_only".to_string(),
                name: "Whisper + Smart Detection".to_string(),
                description: "Local processing with OpenAI Whisper and intelligent speaker detection".to_string(),
                requirements: vec!["Python", "whisper".to_string()],
            });
        }
    }
    
    // Always add basic transcription option
    providers.push(DiarizationProvider {
        id: "basic".to_string(),
        name: "Basic Transcription".to_string(),
        description: "Simple transcription without speaker identification".to_string(),
        requirements: vec!["OpenAI API key".to_string()],
    });
    
    Ok(providers)
}

fn check_python_dependency(package: &str) -> bool {
    Command::new("python3")
        .args(&["-c", &format!("import {}", package)])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn select_provider(providers: &[DiarizationProvider]) -> Result<DiarizationProvider> {
    println!("{}🔧 Available diarization providers:{}", BOLD, RESET);
    println!();
    
    for (i, provider) in providers.iter().enumerate() {
        let status = if provider.requirements.iter().any(|req| req.contains("not configured")) {
            format!("{}⚠️  Requires setup", YELLOW)
        } else {
            format!("{}✅ Ready", GREEN)
        };
        
        println!("{}{}. {}{} {}", GREEN, i + 1, BOLD, provider.name, status);
        println!("   {}📝 {}{}", BLUE, provider.description, RESET);
        println!("   {}📋 Requirements: {}{}", BLUE, provider.requirements.join(", "), RESET);
        println!();
    }
    
    loop {
        print!("{}Select provider (1-{}): {}", YELLOW, providers.len(), RESET);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        match input.trim().parse::<usize>() {
            Ok(choice) if choice >= 1 && choice <= providers.len() => {
                let selected = &providers[choice - 1];
                
                // Check if provider requires setup
                if selected.requirements.iter().any(|req| req.contains("not configured")) {
                    println!("{}⚠️  This provider requires additional setup.{}", YELLOW, RESET);
                    print!("Do you want to continue anyway? (y/n): ");
                    io::stdout().flush()?;
                    
                    let mut confirm = String::new();
                    io::stdin().read_line(&mut confirm)?;
                    
                    if !confirm.trim().to_lowercase().starts_with('y') {
                        continue;
                    }
                }
                
                return Ok(selected.clone());
            }
            _ => {
                println!("{}❌ Invalid choice. Please enter a number between 1 and {}.{}", RED, providers.len(), RESET);
            }
        }
    }
}

fn generate_transcript(audio: &MeetingAudio, provider: &DiarizationProvider) -> Result<TranscriptResult> {
    println!("{}📄 Transcribing: {}{}", BLUE, audio.name, RESET);
    println!("{}🔧 Using provider: {}{}", BLUE, provider.name, RESET);
    println!();
    
    match provider.id.as_str() {
        "elevenlabs" => generate_elevenlabs_transcript(audio),
        "whisper_pyannote" => generate_whisper_pyannote_transcript(audio),
        "whisper_only" => generate_whisper_only_transcript(audio),
        "basic" => generate_basic_transcript(audio),
        _ => Err(anyhow::anyhow!("Unknown provider: {}", provider.id)),
    }
}

fn generate_elevenlabs_transcript(audio: &MeetingAudio) -> Result<TranscriptResult> {
    println!("{}🌐 Sending audio to ElevenLabs API...{}", BLUE, RESET);
    
    // Use the built-in advanced diarization plugin
    let output = Command::new("cargo")
        .args(&[
            "run", "--release", "--",
            "test-plugin",
            "advanced_diarization",
            "--audio-file", audio.path.to_str().unwrap(),
            "--provider", "elevenlabs",
        ])
        .output()
        .context("Failed to run advanced diarization plugin")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("ElevenLabs transcription failed: {}", stderr));
    }
    
    // Parse the output (assuming it's JSON)
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_plugin_output(&stdout, "elevenlabs")
}

fn generate_whisper_pyannote_transcript(audio: &MeetingAudio) -> Result<TranscriptResult> {
    println!("{}🤖 Processing with Whisper + PyAnnote...{}", BLUE, RESET);
    
    // Use the built-in advanced diarization plugin
    let output = Command::new("cargo")
        .args(&[
            "run", "--release", "--",
            "test-plugin",
            "advanced_diarization",
            "--audio-file", audio.path.to_str().unwrap(),
            "--provider", "whisper_pyannote",
        ])
        .output()
        .context("Failed to run advanced diarization plugin")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Whisper + PyAnnote transcription failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_plugin_output(&stdout, "whisper_pyannote")
}

fn generate_whisper_only_transcript(audio: &MeetingAudio) -> Result<TranscriptResult> {
    println!("{}🤖 Processing with Whisper + Smart Detection...{}", BLUE, RESET);
    
    // Use Python script directly
    let script_path = "scripts/whisper_pyannote_helper.py";
    let output = Command::new("python3")
        .args(&[
            script_path,
            audio.path.to_str().unwrap(),
            "--whisper-model", "base",
            "--whisper-only",
        ])
        .output()
        .context("Failed to run whisper-only transcription")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Whisper-only transcription failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_whisper_output(&stdout, "whisper_only")
}

fn generate_basic_transcript(audio: &MeetingAudio) -> Result<TranscriptResult> {
    println!("{}🔤 Generating basic transcript...{}", BLUE, RESET);
    
    // Use OpenAI Whisper API through the main application
    let output = Command::new("cargo")
        .args(&[
            "run", "--release", "--",
            "transcribe",
            "--audio-file", audio.path.to_str().unwrap(),
            "--no-diarization",
        ])
        .output()
        .context("Failed to run basic transcription")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Basic transcription failed: {}", stderr));
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Create a simple transcript result
    Ok(TranscriptResult {
        provider: "basic".to_string(),
        segments: vec![TranscriptSegment {
            start_time: 0.0,
            end_time: 0.0,
            speaker_id: "Unknown".to_string(),
            text: stdout.trim().to_string(),
            confidence: 0.8,
        }],
        speakers: vec!["Unknown".to_string()],
        total_duration: 0.0,
        generated_at: Utc::now(),
    })
}

fn parse_plugin_output(output: &str, provider: &str) -> Result<TranscriptResult> {
    // Try to parse JSON output from the plugin
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(output) {
        if let Some(segments) = json_value.get("segments").and_then(|s| s.as_array()) {
            let mut parsed_segments = Vec::new();
            let mut speakers = std::collections::HashSet::new();
            
            for segment in segments {
                if let (Some(start), Some(end), Some(speaker), Some(text)) = (
                    segment.get("start_time").and_then(|v| v.as_f64()),
                    segment.get("end_time").and_then(|v| v.as_f64()),
                    segment.get("speaker_id").and_then(|v| v.as_str()),
                    segment.get("text").and_then(|v| v.as_str()),
                ) {
                    speakers.insert(speaker.to_string());
                    parsed_segments.push(TranscriptSegment {
                        start_time: start as f32,
                        end_time: end as f32,
                        speaker_id: speaker.to_string(),
                        text: text.to_string(),
                        confidence: segment.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                    });
                }
            }
            
            let total_duration = parsed_segments.last()
                .map(|s| s.end_time)
                .unwrap_or(0.0);
            
            return Ok(TranscriptResult {
                provider: provider.to_string(),
                segments: parsed_segments,
                speakers: speakers.into_iter().collect(),
                total_duration,
                generated_at: Utc::now(),
            });
        }
    }
    
    // Fallback: treat as plain text
    Ok(TranscriptResult {
        provider: provider.to_string(),
        segments: vec![TranscriptSegment {
            start_time: 0.0,
            end_time: 0.0,
            speaker_id: "Unknown".to_string(),
            text: output.trim().to_string(),
            confidence: 0.8,
        }],
        speakers: vec!["Unknown".to_string()],
        total_duration: 0.0,
        generated_at: Utc::now(),
    })
}

fn parse_whisper_output(output: &str, provider: &str) -> Result<TranscriptResult> {
    // Try to parse JSON output from whisper helper
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(output) {
        if let Some(segments) = json_value.get("segments").and_then(|s| s.as_array()) {
            let mut parsed_segments = Vec::new();
            let mut speakers = std::collections::HashSet::new();
            
            for segment in segments {
                if let (Some(start), Some(end), Some(speaker), Some(text)) = (
                    segment.get("start_time").and_then(|v| v.as_f64()),
                    segment.get("end_time").and_then(|v| v.as_f64()),
                    segment.get("speaker_id").and_then(|v| v.as_str()),
                    segment.get("text").and_then(|v| v.as_str()),
                ) {
                    speakers.insert(speaker.to_string());
                    parsed_segments.push(TranscriptSegment {
                        start_time: start as f32,
                        end_time: end as f32,
                        speaker_id: speaker.to_string(),
                        text: text.to_string(),
                        confidence: segment.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.8) as f32,
                    });
                }
            }
            
            let total_duration = parsed_segments.last()
                .map(|s| s.end_time)
                .unwrap_or(0.0);
            
            return Ok(TranscriptResult {
                provider: provider.to_string(),
                segments: parsed_segments,
                speakers: speakers.into_iter().collect(),
                total_duration,
                generated_at: Utc::now(),
            });
        }
    }
    
    // Fallback for plain text
    Ok(TranscriptResult {
        provider: provider.to_string(),
        segments: vec![TranscriptSegment {
            start_time: 0.0,
            end_time: 0.0,
            speaker_id: "Unknown".to_string(),
            text: output.trim().to_string(),
            confidence: 0.8,
        }],
        speakers: vec!["Unknown".to_string()],
        total_duration: 0.0,
        generated_at: Utc::now(),
    })
}

fn save_transcript(transcript: &TranscriptResult, audio: &MeetingAudio, provider: &DiarizationProvider) -> Result<()> {
    // Create transcripts directory in the production location
    let transcripts_dir = dirs::home_dir()
        .context("Failed to get home directory")?
        .join(".meeting-assistant")
        .join("transcripts");
    if !transcripts_dir.exists() {
        fs::create_dir_all(&transcripts_dir)?;
    }
    
    // Generate filename
    let audio_name = audio.path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("meeting");
    let timestamp = transcript.generated_at.format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}_transcript.txt", audio_name, timestamp);
    let filepath = transcripts_dir.join(&filename);
    
    // Generate human-readable transcript
    let mut content = String::new();
    content.push_str(&format!("Meeting Transcript\n"));
    content.push_str(&format!("=================\n\n"));
    content.push_str(&format!("Source: {}\n", audio.name));
    content.push_str(&format!("Provider: {}\n", provider.name));
    content.push_str(&format!("Generated: {}\n", transcript.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
    content.push_str(&format!("Duration: {:.1} minutes\n", transcript.total_duration / 60.0));
    content.push_str(&format!("Speakers: {}\n", transcript.speakers.join(", ")));
    content.push_str(&format!("\n"));
    
    // Add segments
    for segment in &transcript.segments {
        let minutes = (segment.start_time / 60.0) as u32;
        let seconds = (segment.start_time % 60.0) as u32;
        
        content.push_str(&format!("[{:02}:{:02}] {}: {}\n", 
            minutes, seconds, segment.speaker_id, segment.text));
    }
    
    // Save to file
    fs::write(&filepath, content)?;
    
    println!("{}💾 Transcript saved to: {}{}", GREEN, filepath.display(), RESET);
    
    // Also save JSON version
    let json_filename = format!("{}_{}_transcript.json", audio_name, timestamp);
    let json_filepath = transcripts_dir.join(&json_filename);
    let json_content = serde_json::to_string_pretty(&transcript)?;
    fs::write(&json_filepath, json_content)?;
    
    println!("{}💾 JSON version saved to: {}{}", GREEN, json_filepath.display(), RESET);
    
    Ok(())
}

fn display_transcript(transcript: &TranscriptResult) -> Result<()> {
    println!("\n{}📋 Generated Transcript:{}", BOLD, RESET);
    println!("{}========================{}", BOLD, RESET);
    println!();
    
    // Display summary
    println!("{}📊 Summary:{}", BOLD, RESET);
    println!("  Provider: {}", transcript.provider);
    println!("  Speakers: {}", transcript.speakers.join(", "));
    println!("  Segments: {}", transcript.segments.len());
    println!("  Duration: {:.1} minutes", transcript.total_duration / 60.0);
    println!();
    
    // Display transcript segments
    println!("{}📝 Transcript:{}", BOLD, RESET);
    println!();
    
    for segment in &transcript.segments {
        let minutes = (segment.start_time / 60.0) as u32;
        let seconds = (segment.start_time % 60.0) as u32;
        
        println!("{}[{:02}:{:02}] {}{}: {}", 
            BLUE, minutes, seconds, GREEN, segment.speaker_id, RESET);
        println!("  {}", segment.text);
        println!();
    }
    
    Ok(())
} 
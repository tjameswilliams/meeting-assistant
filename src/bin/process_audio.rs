use anyhow::Result;
use std::path::PathBuf;
use tokio;
use tracing_subscriber;
use dirs::home_dir;
use meeting_assistant_rs::plugins::rust_native_diarization::SpectralDiarizationPlugin;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Get the audio file path from command line
    let audio_file = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("❌ No audio file specified");
            eprintln!("Usage: cargo run --bin process_audio <audio_file_path>");
            eprintln!();
            eprintln!("Examples:");
            eprintln!("  cargo run --bin process_audio ./recordings/meeting.wav");
            if let Some(home) = home_dir() {
                let recordings_dir = home.join(".meeting-assistant").join("recordings");
                eprintln!("  cargo run --bin process_audio {:?}/your_file.wav", recordings_dir);
            }
            std::process::exit(1);
        }
    };
    
    if !audio_file.exists() {
        eprintln!("❌ Audio file not found: {:?}", audio_file);
        eprintln!("Please check the file path and try again.");
        std::process::exit(1);
    }
    
    println!("🎵 Processing audio file with improved spectral diarization...");
    println!("📁 File: {:?}", audio_file);
    
    // Create the improved spectral diarization plugin
    let diarization_plugin = SpectralDiarizationPlugin::new();
    
    // Process the audio file
    match diarization_plugin.process_audio_file(&audio_file).await {
        Ok(segments) => {
            println!("✅ Diarization completed successfully!");
            println!();
            
            // Show summary
            let speakers: std::collections::HashSet<_> = segments.iter()
                .map(|s| &s.speaker_id)
                .collect();
            
            println!("📊 Diarization Results:");
            println!("  🎙️  Total speakers detected: {}", speakers.len());
            println!("  📝 Total segments: {}", segments.len());
            println!("  ⏱️  Total duration: {:.1}s", 
                segments.iter().map(|s| s.end_time - s.start_time).sum::<f64>());
            println!();
            
            // Show speaker breakdown
            for speaker_id in speakers {
                let speaker_segments: Vec<_> = segments.iter()
                    .filter(|s| &s.speaker_id == speaker_id)
                    .collect();
                
                let total_duration: f64 = speaker_segments.iter()
                    .map(|s| s.end_time - s.start_time)
                    .sum();
                
                let avg_confidence: f64 = speaker_segments.iter()
                    .map(|s| s.confidence)
                    .sum::<f64>() / speaker_segments.len() as f64;
                
                let f0 = speaker_segments.first()
                    .map(|s| s.voice_characteristics.fundamental_frequency)
                    .unwrap_or(0.0);
                
                println!("  {}: {} segments, {:.1}s total, F0={:.1}Hz, conf={:.3}", 
                    speaker_id, speaker_segments.len(), total_duration, f0, avg_confidence);
            }
            println!();
            
            // Show detailed segments
            println!("🎯 Detailed Segments:");
            for (i, segment) in segments.iter().enumerate() {
                println!("  [{}] {:.2}s-{:.2}s: {} (conf: {:.3})", 
                    i + 1,
                    segment.start_time, 
                    segment.end_time,
                    segment.speaker_id,
                    segment.confidence
                );
            }
            
            // Export results to JSON
            let export_result = diarization_plugin.export_diarization(&segments).await?;
            let output_file = audio_file.with_extension("diarization.json");
            tokio::fs::write(&output_file, serde_json::to_string_pretty(&export_result)?).await?;
            println!();
            println!("💾 Results saved to: {:?}", output_file);
        }
        Err(e) => {
            eprintln!("❌ Diarization failed: {}", e);
            return Err(e);
        }
    }
    
    Ok(())
} 
/*
 * Meeting Assistant CLI - Continuous Mode Test Binary
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
use clap::{Parser, Subcommand};
use tokio::time::{sleep, Duration};

use meeting_assistant_rs::continuous_main::{
    ContinuousMeetingAssistant, Commands, Cli, handle_database_command,
    TranscriptionPipeline, DiarizationPipeline, VectorizationPipeline, StoragePipeline,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    println!("🎯 Continuous Meeting Assistant - Test Mode");
    println!("============================================");
    
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Start { title, no_auto_record: _ }) => {
            println!("🚀 Starting continuous meeting recording...");
            
            let assistant = ContinuousMeetingAssistant::new().await?;
            let meeting_id = assistant.start_meeting(title).await?;
            
            println!("✅ Meeting started with ID: {}", meeting_id);
            println!();
            
            // Show status for 10 seconds
            for i in 1..=10 {
                let status = assistant.get_status().await;
                println!("⏱️  Status check {}/10:", i);
                println!("   Recording: {:?}", status.recording_status);
                println!("   Audio Pipeline: {:?}", status.pipeline_health.audio_capture);
                println!("   Queue sizes: {:?}", status.queue_status);
                println!("   Total errors: {}", status.error_count.total_errors);
                println!();
                
                sleep(Duration::from_secs(1)).await;
            }
            
            println!("🛑 Stopping meeting...");
            assistant.stop_meeting(false).await?;
            println!("✅ Test completed successfully!");
        }
        
        Some(Commands::Status) => {
            println!("📊 System Status Demo");
            println!("This would show real-time pipeline status in production");
        }
        
        Some(Commands::Search { query, mode, limit, .. }) => {
            println!("🔍 Search Demo");
            println!("Query: '{}' (mode: {}, limit: {})", query, mode, limit);
            println!("This would perform semantic search through meeting transcripts");
        }
        
        Some(Commands::Database { action }) => {
            handle_database_command(action).await?;
        }
        
        _ => {
            println!("🎯 Continuous Meeting Assistant - Test Mode");
            println!("============================================");
            println!();
            println!("Available commands:");
            println!("  cargo run --bin continuous start --title 'Test Meeting'");
            println!("  cargo run --bin continuous status");
            println!("  cargo run --bin continuous search 'project timeline' --mode semantic");
            println!();
            println!("This demonstrates the new continuous architecture with:");
            println!("• 🎙️  Real-time audio capture and processing");
            println!("• 📝 Continuous transcription pipeline");
            println!("• 👥 Speaker diarization");
            println!("• 🔮 Vector embeddings for semantic search");
            println!("• 💾 Automatic SQLite storage");
            println!("• 📊 Real-time status monitoring");
        }
    }
    
    Ok(())
} 
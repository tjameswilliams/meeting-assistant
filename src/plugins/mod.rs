/*
 * Meeting Assistant CLI - Built-in Plugins
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

//! Built-in plugins that ship with Meeting Assistant CLI
//! 
//! These plugins provide core functionality and demonstrate the plugin system capabilities.

pub mod ollama_provider;
pub mod sentiment_analyzer;
pub mod rust_native_diarization;
pub mod stt_post_processor;
pub mod advanced_diarization;
pub mod transcript_interactive;

pub use ollama_provider::OllamaProvider;
pub use sentiment_analyzer::SentimentAnalyzerPlugin;
pub use rust_native_diarization::{SpectralDiarizationPlugin};
pub use advanced_diarization::{create_advanced_diarization_plugin, AdvancedDiarizationPlugin};
pub use stt_post_processor::STTPostProcessorPlugin;
pub use transcript_interactive::{create_transcript_interactive_plugin, TranscriptInteractivePlugin}; 
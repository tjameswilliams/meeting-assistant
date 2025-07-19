/*
 * Meeting Assistant CLI - Transcript Interactive Plugin
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
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fmt;
use std::str::FromStr;

use crate::plugin_system::*;
use crate::ui::TerminalUI;
use crate::ai::OpenAIClient;

/// Configuration for the Transcript Interactive plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptInteractiveConfig {
    /// Enable/disable the plugin
    pub enabled: bool,
    /// Default transcripts directory
    pub transcripts_dir: Option<PathBuf>,
    /// Number of transcripts to show in list
    pub max_display_count: usize,
    /// Enable markdown formatting for responses
    pub markdown_formatting: bool,
    /// Default output format
    pub default_output_format: OutputFormat,
    /// Always ask for output format (if false, uses default)
    pub always_ask_format: bool,
}

impl Default for TranscriptInteractiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transcripts_dir: None,
            max_display_count: 20,
            markdown_formatting: true,
            default_output_format: OutputFormat::Markdown,
            always_ask_format: true,
        }
    }
}

/// Information about a discovered transcript file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptFileInfo {
    pub file_path: PathBuf,
    pub file_name: String,
    pub created_at: DateTime<Utc>,
    pub file_size: u64,
    pub transcript_format: TranscriptFormat,
    pub preview: String,
}

/// Supported transcript formats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TranscriptFormat {
    PlainText,
    ElevenLabsJson,
    STTPluginJson,
    Unknown,
}

/// Supported output formats for AI responses
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OutputFormat {
    /// Standard markdown format (current default)
    Markdown,
    /// HTML format for rich text editors
    Html,
    /// Plain text with minimal formatting
    PlainText,
    /// Optimized for Outlook/Teams with simple formatting
    OutlookTeams,
}

impl OutputFormat {
    pub fn display_name(&self) -> &str {
        match self {
            OutputFormat::Markdown => "Markdown",
            OutputFormat::Html => "HTML",
            OutputFormat::PlainText => "Plain Text",
            OutputFormat::OutlookTeams => "Outlook/Teams Optimized",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            OutputFormat::Markdown => "Standard markdown format (good for GitHub, technical docs)",
            OutputFormat::Html => "HTML format (pasteable into some rich text editors)",
            OutputFormat::PlainText => "Clean plain text with minimal formatting",
            OutputFormat::OutlookTeams => "Simple formatting optimized for Outlook and Teams",
        }
    }

    /// Get all available output formats
    pub fn all() -> Vec<OutputFormat> {
        vec![
            OutputFormat::Markdown,
            OutputFormat::Html,
            OutputFormat::PlainText,
            OutputFormat::OutlookTeams,
        ]
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Markdown => write!(f, "Markdown"),
            OutputFormat::Html => write!(f, "Html"),
            OutputFormat::PlainText => write!(f, "PlainText"),
            OutputFormat::OutlookTeams => write!(f, "OutlookTeams"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Markdown" => Ok(OutputFormat::Markdown),
            "Html" => Ok(OutputFormat::Html),
            "PlainText" => Ok(OutputFormat::PlainText),
            "OutlookTeams" => Ok(OutputFormat::OutlookTeams),
            _ => Err(anyhow::anyhow!("Invalid output format: {}", s)),
        }
    }
}

/// Transcript Interactive Plugin
pub struct TranscriptInteractivePlugin {
    config: TranscriptInteractiveConfig,
    enabled: bool,
    terminal_ui: Option<Arc<TerminalUI>>,
    openai_client: Option<Arc<OpenAIClient>>,
}

impl TranscriptInteractivePlugin {
    pub fn new() -> Self {
        Self {
            config: TranscriptInteractiveConfig::default(),
            enabled: true,
            terminal_ui: None,
            openai_client: None,
        }
    }

    /// Set the terminal UI and OpenAI client references
    pub fn set_services(&mut self, terminal_ui: Arc<TerminalUI>, openai_client: Arc<OpenAIClient>) {
        self.terminal_ui = Some(terminal_ui);
        self.openai_client = Some(openai_client);
    }

    /// Run the interactive transcript selection and processing
    pub async fn run_interactive(&self) -> Result<()> {
        // Check if services are available
        let terminal_ui = self.terminal_ui.as_ref()
            .context("Terminal UI not available")?;
        let openai_client = self.openai_client.as_ref()
            .context("OpenAI client not available")?;

        // Get transcripts directory
        let transcripts_dir = self.get_transcripts_directory()?;
        
        // Discover available transcripts
        let transcripts = self.discover_transcripts(&transcripts_dir).await?;
        
        if transcripts.is_empty() {
            println!("{}", "📝 No transcripts found in the transcripts directory.".yellow());
            println!("{}", "   Start using the Meeting Assistant to generate transcripts!".bright_black());
            return Ok(());
        }

        // Display available transcripts
        self.display_transcript_list(&transcripts).await?;

        // Get user selection
        let selected_transcript = self.get_user_selection(&transcripts).await?;
        
        if let Some(transcript_info) = selected_transcript {
            // Load the selected transcript
            let transcript_content = self.load_transcript(&transcript_info).await?;
            
            // Display transcript preview
            self.display_transcript_preview(&transcript_info, &transcript_content).await?;
            
            // Get output format preference
            let output_format = self.get_output_format_selection().await?;
            
            // Get user prompt
            let user_prompt = self.get_user_prompt().await?;
            
            if let Some(prompt) = user_prompt {
                // Combine transcript and user prompt with format instructions
                let combined_prompt = self.create_combined_prompt(&transcript_content, &prompt, &output_format);
                
                // Send to LLM and display response
                self.process_and_display_response(terminal_ui, openai_client, &combined_prompt, &output_format).await?;
            }
        }

        Ok(())
    }

    /// Get the transcripts directory from config or default location
    fn get_transcripts_directory(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.config.transcripts_dir {
            return Ok(dir.clone());
        }

        // Prioritize the production directory ~/.meeting-assistant/transcripts
        if let Some(home_dir) = dirs::home_dir() {
            let home_transcripts = home_dir.join(".meeting-assistant").join("transcripts");
            // Create the directory if it doesn't exist and return it
            std::fs::create_dir_all(&home_transcripts)?;
            return Ok(home_transcripts);
        }

        // Fallback to local directory if home directory is not available
        let default_dir = std::env::current_dir()?.join("transcripts");
        std::fs::create_dir_all(&default_dir)?;
        Ok(default_dir)
    }

    /// Discover and analyze transcript files in the directory
    async fn discover_transcripts(&self, transcripts_dir: &Path) -> Result<Vec<TranscriptFileInfo>> {
        if !transcripts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut transcripts = Vec::new();
        let entries = fs::read_dir(transcripts_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            // Only process files, not directories
            if !path.is_file() {
                continue;
            }

            // Check if it's a transcript file (by extension or naming pattern)
            if let Some(transcript_info) = self.analyze_transcript_file(&path).await? {
                transcripts.push(transcript_info);
            }
        }

        // Sort by creation time (newest first)
        transcripts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Limit to max display count
        transcripts.truncate(self.config.max_display_count);

        Ok(transcripts)
    }

    /// Analyze a file to determine if it's a transcript and extract metadata
    async fn analyze_transcript_file(&self, file_path: &Path) -> Result<Option<TranscriptFileInfo>> {
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Check file extension and naming patterns
        let is_transcript = file_name.contains("transcript") || 
                           file_path.extension().map_or(false, |ext| {
                               ext == "txt" || ext == "json"
                           });

        if !is_transcript {
            return Ok(None);
        }

        // Get file metadata
        let metadata = fs::metadata(file_path)?;
        let file_size = metadata.len();
        
        // Try to get creation time, fall back to modified time
        let created_at = metadata.created()
            .or_else(|_| metadata.modified())
            .map(|time| DateTime::<Utc>::from(time))
            .unwrap_or_else(|_| Utc::now());

        // Determine transcript format by examining content
        let transcript_format = self.detect_transcript_format(file_path).await?;
        
        // Generate preview
        let preview = self.generate_transcript_preview(file_path, &transcript_format).await?;

        Ok(Some(TranscriptFileInfo {
            file_path: file_path.to_path_buf(),
            file_name,
            created_at,
            file_size,
            transcript_format,
            preview,
        }))
    }

    /// Detect the format of a transcript file
    async fn detect_transcript_format(&self, file_path: &Path) -> Result<TranscriptFormat> {
        // Read first few lines to determine format
        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => return Ok(TranscriptFormat::Unknown),
        };

        // Check for JSON formats first
        if file_path.extension().map_or(false, |ext| ext == "json") {
            if content.contains("language_code") && content.contains("words") {
                return Ok(TranscriptFormat::ElevenLabsJson);
            } else if content.contains("transcript") || content.contains("id") {
                return Ok(TranscriptFormat::STTPluginJson);
            }
        }

        // Check for plain text format
        if content.contains("Meeting Transcript") || content.contains("Provider:") {
            return Ok(TranscriptFormat::PlainText);
        }

        Ok(TranscriptFormat::Unknown)
    }

    /// Generate a preview of the transcript content
    async fn generate_transcript_preview(&self, file_path: &Path, format: &TranscriptFormat) -> Result<String> {
        let content = fs::read_to_string(file_path).unwrap_or_default();
        
        match format {
            TranscriptFormat::PlainText => {
                // Extract the main content from plain text transcripts
                let lines: Vec<&str> = content.lines().collect();
                for line in lines.iter().skip(5) { // Skip header lines
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('[') && trimmed.len() > 20 {
                        return Ok(self.truncate_text(trimmed, 100));
                    }
                }
                Ok("No preview available".to_string())
            }
            TranscriptFormat::ElevenLabsJson => {
                // Parse JSON and extract text field
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(text) = json_value.get("text").and_then(|t| t.as_str()) {
                        return Ok(self.truncate_text(text, 100));
                    }
                }
                Ok("ElevenLabs transcript (preview unavailable)".to_string())
            }
            TranscriptFormat::STTPluginJson => {
                // Parse JSON and look for transcript content
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(transcript) = json_value.get("transcript").and_then(|t| t.as_str()) {
                        if !transcript.trim().is_empty() {
                            return Ok(self.truncate_text(transcript, 100));
                        }
                    }
                    if let Some(full_text) = json_value.get("full_text").and_then(|t| t.as_str()) {
                        return Ok(self.truncate_text(full_text, 100));
                    }
                }
                Ok("STT Plugin transcript (preview unavailable)".to_string())
            }
            TranscriptFormat::Unknown => {
                Ok(self.truncate_text(&content, 100))
            }
        }
    }

    /// Truncate text to specified length with ellipsis
    fn truncate_text(&self, text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len])
        }
    }

    /// Display the list of available transcripts
    async fn display_transcript_list(&self, transcripts: &[TranscriptFileInfo]) -> Result<()> {
        println!();
        println!("{}", "📝 Available Transcripts".cyan().bold());
        println!("{}", "=".repeat(60).bright_black());

        for (index, transcript) in transcripts.iter().enumerate() {
            let number = format!("{}.", index + 1);
            let date = transcript.created_at.format("%Y-%m-%d %H:%M:%S UTC");
            let size = format!("{:.1} KB", transcript.file_size as f64 / 1024.0);
            let format_label = match transcript.transcript_format {
                TranscriptFormat::PlainText => "TXT".green(),
                TranscriptFormat::ElevenLabsJson => "ElevenLabs".blue(),
                TranscriptFormat::STTPluginJson => "STT Plugin".yellow(),
                TranscriptFormat::Unknown => "Unknown".red(),
            };

            println!("{:>3} {} [{}] {}", 
                number.cyan().bold(), 
                transcript.file_name.white(),
                format_label,
                size.bright_black()
            );
            println!("    {} | {}", 
                date.to_string().bright_black(),
                transcript.preview.white()
            );
            println!();
        }

        Ok(())
    }

    /// Get user selection from the transcript list
    async fn get_user_selection(&self, transcripts: &[TranscriptFileInfo]) -> Result<Option<TranscriptFileInfo>> {
        loop {
            print!("Enter transcript number (1-{}) or 'q' to quit: ", transcripts.len());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") {
                return Ok(None);
            }

            if let Ok(selection) = input.parse::<usize>() {
                if selection >= 1 && selection <= transcripts.len() {
                    return Ok(Some(transcripts[selection - 1].clone()));
                }
            }

            println!("{}", "Invalid selection. Please try again.".red());
        }
    }

    /// Load the content of a selected transcript
    async fn load_transcript(&self, transcript_info: &TranscriptFileInfo) -> Result<String> {
        let content = fs::read_to_string(&transcript_info.file_path)
            .context("Failed to read transcript file")?;

        // Extract the main transcript text based on format
        match transcript_info.transcript_format {
            TranscriptFormat::PlainText => {
                // For plain text, return as-is
                Ok(content)
            }
            TranscriptFormat::ElevenLabsJson => {
                // Parse JSON and extract text field
                let json_value: serde_json::Value = serde_json::from_str(&content)
                    .context("Failed to parse ElevenLabs JSON")?;
                
                let text = json_value.get("text")
                    .and_then(|t| t.as_str())
                    .context("No text field found in ElevenLabs JSON")?;
                
                Ok(text.to_string())
            }
            TranscriptFormat::STTPluginJson => {
                // Parse JSON and look for transcript content
                let json_value: serde_json::Value = serde_json::from_str(&content)
                    .context("Failed to parse STT Plugin JSON")?;
                
                // Try different possible fields
                if let Some(transcript) = json_value.get("transcript").and_then(|t| t.as_str()) {
                    if !transcript.trim().is_empty() {
                        return Ok(transcript.to_string());
                    }
                }
                
                if let Some(full_text) = json_value.get("full_text").and_then(|t| t.as_str()) {
                    return Ok(full_text.to_string());
                }
                
                // Fallback to raw JSON if no specific text field
                Ok(content)
            }
            TranscriptFormat::Unknown => {
                // Return raw content
                Ok(content)
            }
        }
    }

    /// Display a preview of the selected transcript
    async fn display_transcript_preview(&self, transcript_info: &TranscriptFileInfo, content: &str) -> Result<()> {
        println!();
        println!("{}", "📄 Selected Transcript".green().bold());
        println!("{}", "-".repeat(60).bright_black());
        println!("{} {}", "File:".blue(), transcript_info.file_name.white());
        println!("{} {}", "Created:".blue(), transcript_info.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string().white());
        println!("{} {}", "Size:".blue(), format!("{:.1} KB", transcript_info.file_size as f64 / 1024.0).white());
        println!("{} {:?}", "Format:".blue(), transcript_info.transcript_format);
        println!();
        
        // Show a preview of the content
        let preview = self.truncate_text(content, 300);
        println!("{}", "Preview:".yellow().bold());
        println!("{}", preview.white());
        if content.len() > 300 {
            println!("{}", "... (content truncated)".bright_black());
        }
        println!();

        Ok(())
    }

    /// Get user prompt for processing the transcript
    async fn get_user_prompt(&self) -> Result<Option<String>> {
        println!("{}", "💬 Enter your question or analysis prompt:".cyan().bold());
        println!("{}", "   (This will be sent to the AI along with the transcript content)".bright_black());
        print!("{}", "Prompt: ".yellow());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") {
            return Ok(None);
        }

        Ok(Some(input.to_string()))
    }

    /// Get output format preference from user
    async fn get_output_format_selection(&self) -> Result<OutputFormat> {
        if !self.config.always_ask_format {
            return Ok(self.config.default_output_format);
        }

        println!();
        println!("{}", "📝 Select output format for AI response:".cyan().bold());
        println!("{}", "=".repeat(60).bright_black());

        for (i, format) in OutputFormat::all().iter().enumerate() {
            println!("  {}. {} ({})", i + 1, format.display_name(), format.description());
        }
        println!();

        loop {
            print!("Enter format number (1-{}) or 'q' to quit: ", OutputFormat::all().len());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.eq_ignore_ascii_case("q") || input.eq_ignore_ascii_case("quit") {
                return Ok(OutputFormat::Markdown); // Default to markdown if user quits
            }

            if let Ok(selection) = input.parse::<usize>() {
                if selection >= 1 && selection <= OutputFormat::all().len() {
                    let formats = OutputFormat::all();
                    return Ok(formats[selection - 1]);
                }
            }

            println!("{}", "Invalid selection. Please try again.".red());
        }
    }

    /// Create a combined prompt with transcript and user input
    fn create_combined_prompt(&self, transcript_content: &str, user_prompt: &str, output_format: &OutputFormat) -> String {
        let format_instructions = match output_format {
            OutputFormat::Markdown => "Please provide your response in markdown format.",
            OutputFormat::Html => "Please provide your response in HTML format.",
            OutputFormat::PlainText => "Please provide your response in plain text format.",
            OutputFormat::OutlookTeams => "Please provide your response in a format optimized for Outlook and Teams, with simple formatting.",
        };

        format!(
            "I have a meeting transcript that I'd like you to analyze. Please review the transcript below and answer my question.\n\n\
            Transcript:\n\
            ```\n{}\n```\n\n\
            My question: {}\n\n\
            Please provide a detailed analysis based on the transcript content. {}\n\n\
            Please ensure your response is in the selected output format.",
            transcript_content,
            user_prompt,
            format_instructions
        )
    }

    /// Process the combined prompt and display the AI response
    async fn process_and_display_response(
        &self, 
        terminal_ui: &TerminalUI, 
        openai_client: &OpenAIClient, 
        prompt: &str,
        output_format: &OutputFormat,
    ) -> Result<()> {
        println!();
        println!("{}", "🤖 Processing with AI...".cyan().bold());
        println!("{}", "-".repeat(60).bright_black());

        // Generate AI response
        let system_prompt = self.create_system_prompt(output_format);
        match openai_client.simple_completion(&system_prompt, prompt, 1800).await {
            Ok(response) => {
                // Post-process the response based on the selected format
                let formatted_response = self.post_process_response(&response, output_format)?;
                
                // Display the response based on format
                self.display_formatted_response(&formatted_response, output_format, terminal_ui).await?;
            }
            Err(e) => {
                println!("{}", format!("❌ Error generating AI response: {}", e).red());
            }
        }

        println!();
        println!("{}", "✅ Analysis complete!".green());
        
        Ok(())
    }

    /// Create a system prompt based on the selected output format
    fn create_system_prompt(&self, output_format: &OutputFormat) -> String {
        let base_prompt = "You are an expert analyst helping with meeting transcript analysis. Provide clear, insightful analysis based on the transcript content.";
        
        match output_format {
            OutputFormat::Markdown => {
                format!("{} Format your response in clean markdown with appropriate headers, bullet points, and emphasis.", base_prompt)
            }
            OutputFormat::Html => {
                format!("{} Format your response in clean HTML with appropriate tags like <h2>, <p>, <ul>, <li>, <strong>, and <em>. Do not include full HTML document structure, just the content elements.", base_prompt)
            }
            OutputFormat::PlainText => {
                format!("{} Format your response in plain text without any special formatting characters. Use simple line breaks and spacing for structure.", base_prompt)
            }
            OutputFormat::OutlookTeams => {
                format!("{} Format your response for easy copy-paste into Outlook or Teams. Use simple formatting: **bold text**, bullet points with -, numbered lists with 1., and clear paragraph breaks. Avoid complex markdown syntax.", base_prompt)
            }
        }
    }

    /// Post-process the AI response to ensure proper formatting
    fn post_process_response(&self, response: &str, output_format: &OutputFormat) -> Result<String> {
        match output_format {
            OutputFormat::Markdown => {
                // Response should already be in markdown, just clean it up
                Ok(response.to_string())
            }
            OutputFormat::Html => {
                // Ensure proper HTML formatting
                Ok(self.ensure_html_formatting(response))
            }
            OutputFormat::PlainText => {
                // Strip all formatting and clean up
                Ok(self.strip_formatting(response))
            }
            OutputFormat::OutlookTeams => {
                // Optimize for Outlook/Teams compatibility
                Ok(self.optimize_for_outlook_teams(response))
            }
        }
    }

    /// Ensure proper HTML formatting
    fn ensure_html_formatting(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // If it doesn't look like HTML, add basic HTML structure
        if !result.contains("<p>") && !result.contains("<h") {
            // Convert markdown-like formatting to HTML
            result = result
                .replace("**", "<strong>")
                .replace("**", "</strong>")
                .replace("*", "<em>")
                .replace("*", "</em>");
            
            // Convert line breaks to paragraphs
            let paragraphs: Vec<&str> = result.split("\n\n").collect();
            result = paragraphs
                .iter()
                .filter(|p| !p.trim().is_empty())
                .map(|p| format!("<p>{}</p>", p.trim()))
                .collect::<Vec<_>>()
                .join("\n");
        }
        
        result
    }

    /// Strip all formatting for plain text
    fn strip_formatting(&self, text: &str) -> String {
        text
            .replace("**", "")
            .replace("*", "")
            .replace("#", "")
            .replace("`", "")
            .replace("<", "")
            .replace(">", "")
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Optimize formatting for Outlook/Teams compatibility
    fn optimize_for_outlook_teams(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // Ensure bullet points use simple dashes
        result = result
            .replace("•", "-")
            .replace("*", "-");
        
        // Clean up headers to be simple
        result = result
            .replace("###", "")
            .replace("##", "")
            .replace("#", "");
        
        // Ensure proper line spacing for readability
        let lines: Vec<&str> = result.lines().collect();
        let mut formatted_lines = Vec::new();
        
        for line in lines {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                formatted_lines.push(trimmed.to_string());
            }
        }
        
        // Join with appropriate spacing
        formatted_lines.join("\n\n")
    }

    /// Display the response based on the selected format
    async fn display_formatted_response(
        &self,
        response: &str,
        output_format: &OutputFormat,
        terminal_ui: &TerminalUI,
    ) -> Result<()> {
        println!();
        println!("{}", format!("📄 Response ({})", output_format.display_name()).green().bold());
        println!("{}", "=".repeat(60).bright_black());
        
        match output_format {
            OutputFormat::Markdown => {
                // Use the existing terminal UI markdown rendering
                let system_status = crate::types::SystemStatus::new();
                terminal_ui.stream_response(response, &system_status).await?;
            }
            OutputFormat::Html | OutputFormat::PlainText | OutputFormat::OutlookTeams => {
                // Display as-is for easy copy-paste
                println!("{}", response.white());
                
                // Add helpful copy instructions
                println!();
                println!("{}", "💡 Copy instructions:".yellow().bold());
                match output_format {
                    OutputFormat::Html => {
                        println!("{}", "   Select the text above and copy it. Paste into any rich text editor that supports HTML.".bright_black());
                    }
                    OutputFormat::PlainText => {
                        println!("{}", "   Select the text above and copy it. This plain text will paste cleanly anywhere.".bright_black());
                    }
                    OutputFormat::OutlookTeams => {
                        println!("{}", "   Select the text above and copy it. This format is optimized for Outlook and Teams.".bright_black());
                        println!("{}", "   Bold text (**text**) and simple formatting should work when pasted.".bright_black());
                    }
                    _ => {}
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl Plugin for TranscriptInteractivePlugin {
    fn name(&self) -> &str {
        "transcript_interactive"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Interactive CLI for analyzing existing transcripts with AI assistance"
    }
    
    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom configuration if available
        let plugin_data = context.plugin_data.read().await;
        if let Some(data) = plugin_data.get("transcript_interactive") {
            // Handle both the new config format and individual field updates
            if let Ok(mut custom_config) = serde_json::from_value::<TranscriptInteractiveConfig>(data.clone()) {
                // Handle string-based output format deserialization
                if let Some(format_str) = data.get("default_output_format").and_then(|v| v.as_str()) {
                    if let Ok(output_format) = OutputFormat::from_str(format_str) {
                        custom_config.default_output_format = output_format;
                    }
                }
                self.config = custom_config;
            } else {
                // Handle individual field updates for backwards compatibility
                if let Some(enabled) = data.get("enabled").and_then(|v| v.as_bool()) {
                    self.config.enabled = enabled;
                }
                if let Some(max_count) = data.get("max_display_count").and_then(|v| v.as_u64()) {
                    self.config.max_display_count = max_count as usize;
                }
                if let Some(markdown) = data.get("markdown_formatting").and_then(|v| v.as_bool()) {
                    self.config.markdown_formatting = markdown;
                }
                if let Some(always_ask) = data.get("always_ask_format").and_then(|v| v.as_bool()) {
                    self.config.always_ask_format = always_ask;
                }
                if let Some(format_str) = data.get("default_output_format").and_then(|v| v.as_str()) {
                    if let Ok(output_format) = OutputFormat::from_str(format_str) {
                        self.config.default_output_format = output_format;
                    }
                }
                if let Some(dir_str) = data.get("transcripts_dir").and_then(|v| v.as_str()) {
                    self.config.transcripts_dir = Some(PathBuf::from(dir_str));
                }
            }
        }

        // Set default transcripts directory if not configured
        if self.config.transcripts_dir.is_none() {
            let default_dir = context.temp_dir.parent()
                .map(|p| p.join("transcripts"))
                .unwrap_or_else(|| PathBuf::from("transcripts"));
            self.config.transcripts_dir = Some(default_dir);
        }

        println!("📝 Transcript Interactive Plugin initialized");
        println!("   ✅ Interactive transcript analysis with AI");
        println!("   ✅ Support for multiple transcript formats");
        println!("   ✅ Multiple output formats: Markdown, HTML, Plain Text, Outlook/Teams");
        println!("   ✅ Configurable format selection");
        
        Ok(())
    }
    
    async fn cleanup(&mut self, _context: &PluginContext) -> Result<()> {
        println!("📝 Transcript Interactive Plugin cleaned up");
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
            PluginEvent::Custom { event_type, data } => {
                match event_type.as_str() {
                    "transcript_interactive_run" => {
                        // Run the interactive session
                        if let Err(e) = self.run_interactive().await {
                            eprintln!("Error running transcript interactive session: {}", e);
                        }
                        Ok(PluginHookResult::Replace(json!({"status": "completed"})))
                    }
                    
                    "get_config" => {
                        Ok(PluginHookResult::Replace(serde_json::to_value(&self.config)?))
                    }
                    
                    "set_config" => {
                        if let Ok(mut config) = serde_json::from_value::<TranscriptInteractiveConfig>(data.clone()) {
                            // Handle string-based output format deserialization
                            if let Some(format_str) = data.get("default_output_format").and_then(|v| v.as_str()) {
                                if let Ok(output_format) = OutputFormat::from_str(format_str) {
                                    config.default_output_format = output_format;
                                }
                            }
                            self.config = config;
                            Ok(PluginHookResult::Replace(json!({"status": "config_updated"})))
                        } else {
                            // Handle individual field updates for backwards compatibility
                            if let Some(enabled) = data.get("enabled").and_then(|v| v.as_bool()) {
                                self.config.enabled = enabled;
                            }
                            if let Some(max_count) = data.get("max_display_count").and_then(|v| v.as_u64()) {
                                self.config.max_display_count = max_count as usize;
                            }
                            if let Some(markdown) = data.get("markdown_formatting").and_then(|v| v.as_bool()) {
                                self.config.markdown_formatting = markdown;
                            }
                            if let Some(always_ask) = data.get("always_ask_format").and_then(|v| v.as_bool()) {
                                self.config.always_ask_format = always_ask;
                            }
                            if let Some(format_str) = data.get("default_output_format").and_then(|v| v.as_str()) {
                                if let Ok(output_format) = OutputFormat::from_str(format_str) {
                                    self.config.default_output_format = output_format;
                                }
                            }
                            if let Some(dir_str) = data.get("transcripts_dir").and_then(|v| v.as_str()) {
                                self.config.transcripts_dir = Some(PathBuf::from(dir_str));
                            }
                            Ok(PluginHookResult::Replace(json!({"status": "config_updated"})))
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
                    "description": "Enable/disable the transcript interactive plugin"
                },
                "transcripts_dir": {
                    "type": "string",
                    "description": "Directory path where transcripts are stored"
                },
                "max_display_count": {
                    "type": "integer",
                    "default": 20,
                    "description": "Maximum number of transcripts to show in list"
                },
                "markdown_formatting": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable markdown formatting for AI responses (legacy)"
                },
                "default_output_format": {
                    "type": "string",
                    "enum": ["Markdown", "Html", "PlainText", "OutlookTeams"],
                    "default": "Markdown",
                    "description": "Default output format for AI responses"
                },
                "always_ask_format": {
                    "type": "boolean",
                    "default": true,
                    "description": "Always ask user to select output format (if false, uses default_output_format)"
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
        
        if let Some(transcripts_dir) = config.get("transcripts_dir") {
            if !transcripts_dir.is_string() {
                return Err(anyhow::anyhow!("'transcripts_dir' must be a string"));
            }
        }
        
        if let Some(max_display_count) = config.get("max_display_count") {
            if !max_display_count.is_number() {
                return Err(anyhow::anyhow!("'max_display_count' must be a number"));
            }
        }

        if let Some(markdown_formatting) = config.get("markdown_formatting") {
            if !markdown_formatting.is_boolean() {
                return Err(anyhow::anyhow!("'markdown_formatting' must be a boolean"));
            }
        }

        if let Some(default_output_format) = config.get("default_output_format") {
            if let Some(format_str) = default_output_format.as_str() {
                match format_str {
                    "Markdown" | "Html" | "PlainText" | "OutlookTeams" => {},
                    _ => return Err(anyhow::anyhow!("'default_output_format' must be one of: Markdown, Html, PlainText, OutlookTeams")),
                }
            } else {
                return Err(anyhow::anyhow!("'default_output_format' must be a string"));
            }
        }

        if let Some(always_ask_format) = config.get("always_ask_format") {
            if !always_ask_format.is_boolean() {
                return Err(anyhow::anyhow!("'always_ask_format' must be a boolean"));
            }
        }
        
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Helper function to create the plugin instance
pub fn create_transcript_interactive_plugin() -> Box<dyn Plugin> {
    Box::new(TranscriptInteractivePlugin::new())
} 
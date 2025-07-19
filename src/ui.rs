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

use std::io::stdout;
use anyhow::Result;
use colored::*;
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
    style::{Color, SetForegroundColor, ResetColor},
};
use syntect::{
    parsing::SyntaxSet,
    highlighting::{ThemeSet, Style},
    easy::HighlightLines,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};
use pulldown_cmark::{Parser, Event, Tag, CodeBlockKind};
use regex::Regex;
use crate::types::{SessionEntry, CodeEntry, ContentAnalysis, SystemStatus, HotkeyInfo};

pub struct TerminalUI {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl TerminalUI {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
    
    /// Display the persistent toolbar at the top of the console
    pub async fn display_toolbar(&self, system_status: &SystemStatus) -> Result<()> {
        // Clear the entire screen
        execute!(stdout(), Clear(ClearType::All))?;
        
        // Move cursor to top-left
        execute!(stdout(), crossterm::cursor::MoveTo(0, 0))?;
        
        // Top border
        println!("{}", "═".repeat(80).bright_black());
        
        // Status line
        let status_text = system_status.get_status_summary();
        println!("{} {}", "🤝 Meeting Assistant".cyan().bold(), status_text);
        
        // Hotkeys line
        let hotkeys = HotkeyInfo::format_hotkeys();
        println!("{} {}", "⌨️  Hotkeys:".yellow().bold(), hotkeys.bright_black());
        
        // Bottom border
        println!("{}", "═".repeat(80).bright_black());
        println!();
        
        execute!(stdout(), ResetColor)?;
        Ok(())
    }
    
    /// Clear the console but preserve the toolbar
    pub async fn clear_console_preserve_toolbar(&self, system_status: &SystemStatus) -> Result<()> {
        self.display_toolbar(system_status).await?;
        Ok(())
    }
    
    /// Update the toolbar without clearing the content below it
    pub async fn update_toolbar(&self, system_status: &SystemStatus) -> Result<()> {
        // Save current cursor position
        execute!(stdout(), crossterm::cursor::SavePosition)?;
        
        // Move to top-left and redraw toolbar
        execute!(stdout(), crossterm::cursor::MoveTo(0, 0))?;
        
        // Top border
        println!("{}", "═".repeat(80).bright_black());
        
        // Status line
        let status_text = system_status.get_status_summary();
        println!("{} {}", "🤝 Meeting Assistant".cyan().bold(), status_text);
        
        // Hotkeys line
        let hotkeys = HotkeyInfo::format_hotkeys();
        println!("{} {}", "⌨️  Hotkeys:".yellow().bold(), hotkeys.bright_black());
        
        // Bottom border
        println!("{}", "═".repeat(80).bright_black());
        
        // Restore cursor position
        execute!(stdout(), crossterm::cursor::RestorePosition)?;
        execute!(stdout(), ResetColor)?;
        
        Ok(())
    }
    
    /// Simple welcome message for initial startup
    pub async fn print_welcome(&self, system_status: &SystemStatus) -> Result<()> {
        self.display_toolbar(system_status).await?;
        
        println!("{}", "🚀 Meeting Assistant initialized successfully!".green().bold());
        println!("{}", "   Double-tap hotkeys to interact with the system".bright_black());
        println!();
        
        Ok(())
    }
    
    pub async fn print_status(&self, message: &str) -> Result<()> {
        println!("{}", message.yellow());
        Ok(())
    }
    
    pub async fn print_warning(&self, message: &str) -> Result<()> {
        println!("{}", message.yellow());
        Ok(())
    }
    
    pub async fn print_ready(&self) -> Result<()> {
        println!();
        println!("{}", "🟢 Ready for next action...".green());
        println!();
        Ok(())
    }
    
    pub async fn print_shutdown(&self) -> Result<()> {
        println!();
        println!("{}", "🛑 Stopping Meeting Assistant...".red());
        println!("{}", "Thank you for using Meeting Assistant CLI! 🚀".cyan().bold());
        Ok(())
    }
    
    pub async fn print_transcript(&self, transcript: &str) -> Result<()> {
        println!("{} {}", "📝 Transcript:".blue(), format!("\"{}\"", transcript).white());
        println!();
        Ok(())
    }
    
    pub async fn print_clipboard_preview(&self, content: &str, analysis: &ContentAnalysis) -> Result<()> {
        println!("{}", "📝 Clipboard Content:".blue());
        println!("{}", "-".repeat(50).bright_black());
        
        // Show a preview with syntax highlighting
        let preview = if content.len() > 200 {
            format!("{}...", &content[..200])
        } else {
            content.to_string()
        };
        
        let highlighted = self.highlight_code(&preview, &analysis.language);
        println!("{}", highlighted);
        
        if content.len() > 200 {
            println!("{}", "... (truncated for display)".bright_black());
        }
        
        println!("{}", "-".repeat(50).bright_black());
        println!();
        
        println!("{} {}", "🔍 Content Type:".magenta(), analysis.content_type.bold());
        println!("{} {}", "📋 Language:".blue(), analysis.language.white());
        
        if analysis.confidence > 0.0 {
            let confidence_color = if analysis.confidence > 0.8 {
                "green"
            } else if analysis.confidence > 0.6 {
                "yellow"
            } else {
                "red"
            };
            
            let confidence_text = format!("{}%", (analysis.confidence * 100.0) as u32);
            println!("{} {}", "📊 Confidence:".cyan(), confidence_text.color(confidence_color));
        }
        
        println!();
        Ok(())
    }
    
    pub async fn stream_response(&self, response: &str, system_status: &SystemStatus) -> Result<()> {
        // Clear console but preserve toolbar for new response
        self.clear_console_preserve_toolbar(system_status).await?;
        
        println!("{}", "🤖 AI Support:".cyan().bold());
        println!("{}", "-".repeat(50).bright_black());
        
        // Remove thinking text if present (for thinking models like o1)
        let cleaned_response = self.remove_thinking_text(response);
        
        // Process the response with markdown formatting
        let formatted = self.format_markdown(&cleaned_response);
        println!("{}", formatted);
        
        println!("{}", "-".repeat(50).bright_black());
        println!();
        
        Ok(())
    }
    
    pub async fn print_session_history(
        &self,
        history: &[SessionEntry],
        summary: &str,
        code_memory: &[CodeEntry],
        system_status: &SystemStatus,
    ) -> Result<()> {
        // Clear console but preserve toolbar for history display
        self.clear_console_preserve_toolbar(system_status).await?;
        
        println!("{}", "📚 Session History:".cyan().bold());
        println!("{}", "=".repeat(50).bright_black());
        
        if history.is_empty() {
            println!("{}", "No session history yet. Start recording to build history!".yellow());
            println!();
            return Ok(());
        }
        
        // Show conversation summary if available
        if !summary.is_empty() {
            println!("{}", "💭 Conversation Summary:".blue().bold());
            println!("   {}", summary.white());
            println!();
        }
        
        println!("{}", "📝 Recent Exchanges:".green().bold());
        
        for (index, entry) in history.iter().enumerate() {
            let type_emoji = match entry.question_type.to_string().as_str() {
                "portfolio_history" => "🏢",
                "technical_knowledge" => "🧠",
                "behavioral" => "🤝",
                "general" => "💬",
                "code_analysis" => "💻",
                "combined" => "🔗",
                "screenshot" => "📸",
                _ => "❓",
            };
            
            let timestamp = entry.timestamp.format("%H:%M:%S");
            
            println!(
                "{}. [{}] {} {}",
                index + 1,
                timestamp.to_string().bright_black(),
                type_emoji,
                entry.question_type.to_string().to_uppercase().magenta()
            );
            
            // Show input
            println!("   {}: {}", "Q".blue(), format!("\"{}\"", entry.input).white());
            
            // Show confidence
            let confidence_color = if entry.confidence > 0.8 {
                "green"
            } else if entry.confidence > 0.6 {
                "yellow"
            } else {
                "red"
            };
            
            let confidence_text = format!("{}%", (entry.confidence * 100.0) as u32);
            println!("   {}: {}", "Confidence".cyan(), confidence_text.color(confidence_color));
            
            // Show key topics if available
            if !entry.key_topics.is_empty() {
                println!("   {}: {}", "Topics".yellow(), entry.key_topics.join(", ").white());
            }
            
            // Show response preview (remove thinking text first)
            let cleaned_response = self.remove_thinking_text(&entry.response);
            let preview = if cleaned_response.len() > 150 {
                format!("{}...", &cleaned_response[..150])
            } else {
                cleaned_response
            };
            
            let styled_preview = self.apply_basic_markdown(&preview);
            println!("   {}: {}", "A".green(), styled_preview);
            println!();
        }
        
        println!("{} {}", "📊 Total exchanges:".blue(), history.len());
        
        // Show code memory status
        if !code_memory.is_empty() {
            println!("{}", "💾 Code Memory:".magenta().bold());
            for entry in code_memory {
                println!(
                    "   #{}: {} ({}) - {}",
                    entry.id,
                    entry.language,
                    entry.analysis_type,
                    entry.preview.white()
                );
            }
            println!("{}", "   💡 Previous code can be referenced in follow-up questions".bright_black());
        } else {
            println!("{}", "💾 Code Memory: Empty".bright_black());
        }
        
        self.print_ready().await?;
        
        Ok(())
    }
    
    fn format_markdown(&self, text: &str) -> String {
        let parser = Parser::new(text);
        let mut output = String::new();
        let mut in_code_block = false;
        let mut code_language = String::new();
        let mut code_content = String::new();
        let mut list_depth: usize = 0;
        let mut in_list_item = false;
        let mut list_item_content = String::new();
        let mut just_started_list_item = false;
        let mut current_list_start: Option<u64> = None;
        let mut list_item_number = 1;
        let mut in_strong = false;
        let mut in_emphasis = false;
        let mut in_heading = false;
        let mut heading_content = String::new();
        let mut heading_level = 1;
        
        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    code_language = lang.to_string();
                    code_content.clear();
                }
                Event::End(Tag::CodeBlock(_)) => {
                    if in_code_block {
                        let highlighted = self.highlight_code(&code_content, &code_language);
                        output.push_str(&highlighted);
                        output.push('\n');
                        in_code_block = false;
                    }
                }
                Event::Text(text) => {
                    let mut formatted_text = text.to_string();
                    
                    // Apply formatting based on current context
                    if in_strong {
                        formatted_text = format!("{}", formatted_text.bold());
                    }
                    if in_emphasis {
                        formatted_text = format!("{}", formatted_text.italic());
                    }
                    
                    if in_code_block {
                        code_content.push_str(&text);
                    } else if in_heading {
                        heading_content.push_str(&formatted_text);
                    } else if in_list_item {
                        list_item_content.push_str(&formatted_text);
                        if just_started_list_item {
                            just_started_list_item = false;
                        }
                    } else {
                        output.push_str(&formatted_text);
                    }
                }
                Event::Code(code_text) => {
                    // Handle inline code (backticks)
                    let formatted_code = format!("{}", code_text.white().on_black());
                    if in_heading {
                        heading_content.push_str(&formatted_code);
                    } else if in_list_item {
                        list_item_content.push_str(&formatted_code);
                    } else {
                        output.push_str(&formatted_code);
                    }
                }
                Event::Start(Tag::Strong) => {
                    in_strong = true;
                }
                Event::End(Tag::Strong) => {
                    in_strong = false;
                }
                Event::Start(Tag::Emphasis) => {
                    in_emphasis = true;
                }
                Event::End(Tag::Emphasis) => {
                    in_emphasis = false;
                }
                Event::Start(Tag::Heading(level, _, _)) => {
                    in_heading = true;
                    heading_content.clear();
                    heading_level = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    
                    // Add extra spacing before headings if there's already content
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Event::End(Tag::Heading(_, _, _)) => {
                    if in_heading {
                        let prefix = match heading_level {
                            1 => "# ".blue().bold(),
                            2 => "## ".cyan().bold(),
                            3 => "### ".magenta().bold(),
                            _ => "#### ".yellow().bold(),
                        };
                        output.push_str(&format!("{}{}\n", prefix, heading_content.bold()));
                        output.push('\n'); // Extra spacing after headings
                        in_heading = false;
                        heading_content.clear();
                    }
                }
                Event::Start(Tag::List(list_start)) => {
                    list_depth += 1;
                    current_list_start = list_start;
                    list_item_number = list_start.unwrap_or(1);
                    
                    // Add spacing before lists if there's content
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Event::End(Tag::List(_)) => {
                    list_depth -= 1;
                    if list_depth == 0 {
                        current_list_start = None;
                    }
                    output.push('\n'); // Extra spacing after lists
                }
                Event::Start(Tag::Item) => {
                    in_list_item = true;
                    just_started_list_item = true;
                    list_item_content.clear();
                }
                Event::End(Tag::Item) => {
                    if in_list_item {
                        // Add proper indentation based on list depth
                        let indent = "  ".repeat(list_depth.saturating_sub(1));
                        
                        // Choose the appropriate list marker
                        let marker = match current_list_start {
                            Some(_) => {
                                // Ordered list
                                let marker = format!("{}.", list_item_number);
                                list_item_number += 1;
                                marker.cyan()
                            }
                            None => {
                                // Unordered list
                                "•".yellow()
                            }
                        };
                        
                        output.push_str(&format!("{}{} {}\n", indent, marker, list_item_content.trim()));
                        in_list_item = false;
                        list_item_content.clear();
                    }
                }
                Event::Start(Tag::Paragraph) => {
                    if in_list_item {
                        // Don't add extra spacing within list items
                    } else if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Event::End(Tag::Paragraph) => {
                    if in_list_item {
                        // Add spacing between paragraphs within list items
                        list_item_content.push('\n');
                    } else {
                        output.push('\n');
                        output.push('\n'); // Extra spacing between paragraphs
                    }
                }
                Event::SoftBreak => {
                    if in_heading {
                        heading_content.push(' ');
                    } else if in_list_item {
                        list_item_content.push(' ');
                    } else {
                        output.push(' ');
                    }
                }
                Event::HardBreak => {
                    if in_heading {
                        heading_content.push('\n');
                    } else if in_list_item {
                        list_item_content.push('\n');
                    } else {
                        output.push('\n');
                    }
                }
                _ => {}
            }
        }
        
        // Clean up any trailing whitespace but preserve intentional spacing
        output.trim_end().to_string()
    }
    
    fn apply_inline_formatting(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // This function is now mainly used for basic markdown in previews
        // The main markdown parser handles most formatting through events
        
        // Bold text (before italic to handle nested cases)
        let bold_regex = Regex::new(r"\*\*(.*?)\*\*").unwrap();
        result = bold_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].bold())
        }).to_string();
        
        // Italic text (single asterisks - bold already processed)
        let italic_regex = Regex::new(r"\*([^*]+)\*").unwrap();
        result = italic_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].italic())
        }).to_string();
        
        result
    }
    
    fn apply_basic_markdown(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // Bold text
        let bold_regex = Regex::new(r"\*\*(.*?)\*\*").unwrap();
        result = bold_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].bold())
        }).to_string();
        
        // Inline code (for basic markdown in previews where full parser isn't used)
        let code_regex = Regex::new(r"`(.*?)`").unwrap();
        result = code_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].white().on_black())
        }).to_string();
        
        result
    }
    
    fn remove_thinking_text(&self, text: &str) -> String {
        // First, apply regex-based cleanup for thinking blocks (most reliable)
        let mut result = text.to_string();
        
        let patterns = vec![
            r"(?s)<thinking>.*?</thinking>",
            r"(?s)<think>.*?</think>", 
            r"(?s)<reasoning>.*?</reasoning>",
            r"(?s)<internal.*?>.*?</internal.*?>",
            r"(?s)```thinking\n.*?\n```",
            r"(?s)```reasoning\n.*?\n```",
        ];
        
        for pattern in patterns {
            if let Ok(regex) = Regex::new(pattern) {
                result = regex.replace_all(&result, "").to_string();
            }
        }
        
        // Then process line by line for any remaining edge cases
        let lines: Vec<&str> = result.lines().collect();
        let mut filtered_lines = Vec::new();
        let mut in_thinking_block = false;
        let mut skip_until_next_header = false;
        
        for line in lines {
            let trimmed_line = line.trim();
            
            // Check for any remaining opening thinking tags (including partial matches)
            if trimmed_line.contains("<thinking>") || 
               trimmed_line.contains("<think>") || 
               trimmed_line.contains("<reasoning>") ||
               trimmed_line.contains("<internal") {
                in_thinking_block = true;
                // If the line also contains the closing tag, don't start the block
                if trimmed_line.contains("</thinking>") || 
                   trimmed_line.contains("</think>") ||
                   trimmed_line.contains("</reasoning>") ||
                   trimmed_line.contains("</internal") {
                    in_thinking_block = false;
                }
                continue;
            }
            
            // Check for closing thinking tags
            if in_thinking_block && (
                trimmed_line.contains("</thinking>") || 
                trimmed_line.contains("</think>") ||
                trimmed_line.contains("</reasoning>") ||
                trimmed_line.contains("</internal")
            ) {
                in_thinking_block = false;
                continue;
            }
            
            // Skip lines while in thinking block
            if in_thinking_block {
                continue;
            }
            
            // Check for thinking headers (case insensitive)
            let lower_line = trimmed_line.to_lowercase();
            let is_thinking_header = (lower_line.starts_with("#") && (
                lower_line.contains("thinking") || 
                lower_line.contains("thoughts") ||
                lower_line.contains("reasoning") ||
                lower_line.contains("internal")
            )) || lower_line == "thinking:" || lower_line == "thoughts:";
            
            if is_thinking_header {
                skip_until_next_header = true;
                continue;
            }
            
            // Check if this is a new header (to stop skipping)
            let is_other_header = trimmed_line.starts_with("#") && !is_thinking_header;
            if is_other_header && skip_until_next_header {
                skip_until_next_header = false;
            }
            
            // Skip lines until we hit a new header
            if skip_until_next_header {
                continue;
            }
            
            // Check for standalone thinking content indicators
            if trimmed_line.starts_with("```thinking") || 
               trimmed_line.starts_with("```reasoning") {
                in_thinking_block = true;
                continue;
            }
            
            if in_thinking_block && trimmed_line == "```" {
                in_thinking_block = false;
                continue;
            }
            
            // Skip empty lines at the beginning
            if trimmed_line.is_empty() && filtered_lines.is_empty() {
                continue;
            }
            
            filtered_lines.push(line);
        }
        
        result = filtered_lines.join("\n");
        
        // Final cleanup of extra whitespace and newlines
        let extra_newlines = Regex::new(r"\n{3,}").unwrap();
        result = extra_newlines.replace_all(&result, "\n\n").to_string();
        
        // Remove leading/trailing whitespace
        result.trim().to_string()
    }
    
    fn highlight_code(&self, code: &str, language: &str) -> String {
        // Try to find the syntax for the language
        let syntax = self.syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| self.syntax_set.find_syntax_by_name(language))
            .or_else(|| {
                // Try common mappings
                match language.to_lowercase().as_str() {
                    "js" => self.syntax_set.find_syntax_by_name("JavaScript"),
                    "ts" => self.syntax_set.find_syntax_by_name("TypeScript"),
                    "py" => self.syntax_set.find_syntax_by_name("Python"),
                    "rs" => self.syntax_set.find_syntax_by_name("Rust"),
                    "cpp" | "c++" => self.syntax_set.find_syntax_by_name("C++"),
                    "sh" | "bash" => self.syntax_set.find_syntax_by_name("Bash"),
                    "json" => self.syntax_set.find_syntax_by_name("JSON"),
                    "xml" => self.syntax_set.find_syntax_by_name("XML"),
                    "yaml" | "yml" => self.syntax_set.find_syntax_by_name("YAML"),
                    "sql" => self.syntax_set.find_syntax_by_name("SQL"),
                    "go" => self.syntax_set.find_syntax_by_name("Go"),
                    "java" => self.syntax_set.find_syntax_by_name("Java"),
                    "c" => self.syntax_set.find_syntax_by_name("C"),
                    "php" => self.syntax_set.find_syntax_by_name("PHP"),
                    "rb" | "ruby" => self.syntax_set.find_syntax_by_name("Ruby"),
                    _ => None,
                }
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        // Use a vibrant theme with good contrast - Monokai is excellent for readability
        let theme = &self.theme_set.themes.get("Monokai")
            .or_else(|| self.theme_set.themes.get("InspiredGitHub"))
            .or_else(|| self.theme_set.themes.get("Solarized (dark)"))
            .unwrap_or_else(|| &self.theme_set.themes["base16-ocean.dark"]);
        
        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut output = String::new();
        
        for line in LinesWithEndings::from(code) {
            let ranges: Vec<(Style, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_else(|_| vec![(Style::default(), line)]);
            
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            output.push_str(&format!(" {}", escaped)); // Add padding
        }
        
        output
    }
} 
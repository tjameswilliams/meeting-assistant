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
use crate::types::{SessionEntry, CodeEntry, ContentAnalysis};

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
    
    pub async fn print_welcome(&self) -> Result<()> {
        execute!(
            stdout(),
            Clear(ClearType::All),
            SetForegroundColor(Color::Cyan)
        )?;
        
        println!("{}", "🤝 Meeting Assistant CLI - Rust Edition".cyan().bold());
        println!("{}", "🚀 Ultra-fast AI assistant for meetings and collaboration".green());
        println!("{}", "=".repeat(50).bright_black());
        println!();
        println!("{}", "📋 Automatically listening & buffering audio (system-wide)".green());
        println!("{}", "🔴 Double-tap 'A' quickly to answer questions or provide context".red());
        println!("{}", "💡 Automatically captures ~15 seconds from buffer!".yellow());
        println!("{}", "🕐 Continuous buffering with auto-restart".blue());
        println!("{}", "💻 Double-tap 'S' quickly to analyze clipboard content (code-aware)".green());
        println!("{}", "🔗 Double-tap 'Q' for combined audio + clipboard analysis".cyan());
        println!("{}", "📸 Double-tap 'W' quickly to capture window + audio analysis (code-aware)".magenta());
        println!("{}", "🛑 Double-tap 'R' quickly to cancel current request".red());
        println!("{}", "📚 Double-tap 'H' to view session history & conversation summary".magenta());
        println!("{}", "🔄 Double-tap 'C' to clear conversation context".yellow());
        println!("{}", "🚪 Ctrl+C to exit".bright_black());
        println!("{}", "=".repeat(50).bright_black());
        println!();
        println!("{}", "🎯 Ready to assist with your meetings!".green().bold());
        println!();
        
        execute!(stdout(), ResetColor)?;
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
        println!("{}", "🟢 Ready for next action... (Double-tap 'A' for audio, 'S' for clipboard, 'Q' for combined, 'W' for screenshot, 'R' to cancel)".green());
        println!();
        Ok(())
    }
    
    pub async fn print_shutdown(&self) -> Result<()> {
        println!();
        println!("{}", "🛑 Stopping Interview Assistant...".red());
        println!("{}", "Thank you for using Interview Assistant CLI! 🚀".cyan().bold());
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
    
    pub async fn stream_response(&self, response: &str) -> Result<()> {
        println!("{}", "🤖 AI Support:".cyan().bold());
        println!("{}", "-".repeat(50).bright_black());
        
        // Process the response with markdown formatting
        let formatted = self.format_markdown(response);
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
    ) -> Result<()> {
        println!();
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
            
            // Show response preview
            let preview = if entry.response.len() > 150 {
                format!("{}...", &entry.response[..150])
            } else {
                entry.response.clone()
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
                    if in_code_block {
                        code_content.push_str(&text);
                    } else if in_list_item {
                        // Collect all text within list items
                        let formatted_text = self.apply_inline_formatting(&text);
                        if just_started_list_item {
                            list_item_content.push_str(&formatted_text);
                            just_started_list_item = false;
                        } else {
                            list_item_content.push_str(&formatted_text);
                        }
                    } else {
                        output.push_str(&self.apply_inline_formatting(&text));
                    }
                }
                Event::Start(Tag::Strong) => {
                    // Strong formatting is handled in apply_inline_formatting
                }
                Event::End(Tag::Strong) => {
                    // Strong formatting is handled in apply_inline_formatting
                }
                Event::Start(Tag::Emphasis) => {
                    // Emphasis formatting is handled in apply_inline_formatting
                }
                Event::End(Tag::Emphasis) => {
                    // Emphasis formatting is handled in apply_inline_formatting
                }
                Event::Start(Tag::Heading(level, _, _)) => {
                    // Add extra spacing before headings if there's already content
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    
                    let level_num = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    let prefix = match level_num {
                        1 => "# ".blue().bold(),
                        2 => "## ".cyan().bold(),
                        3 => "### ".magenta().bold(),
                        _ => "#### ".yellow().bold(),
                    };
                    output.push_str(&format!("{}", prefix));
                }
                Event::End(Tag::Heading(_, _, _)) => {
                    output.push('\n');
                    output.push('\n'); // Extra spacing after headings
                }
                Event::Start(Tag::List(_)) => {
                    list_depth += 1;
                    // Add spacing before lists if there's content
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                }
                Event::End(Tag::List(_)) => {
                    list_depth -= 1;
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
                        output.push_str(&format!("{}{} {}\n", indent, "•".yellow(), list_item_content.trim()));
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
                    if in_list_item {
                        list_item_content.push(' ');
                    } else {
                        output.push(' ');
                    }
                }
                Event::HardBreak => {
                    if in_list_item {
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
        
        // Process in order: code first (to avoid interfering with bold/italic), then bold, then italic
        
        // Inline code (highest priority)
        let code_regex = Regex::new(r"`(.*?)`").unwrap();
        result = code_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].on_black().white())
        }).to_string();
        
        // Bold text (before italic to handle nested cases)
        let bold_regex = Regex::new(r"\*\*(.*?)\*\*").unwrap();
        result = bold_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[1].bold().to_string()
        }).to_string();
        
        // Italic text (single asterisks - bold already processed)
        let italic_regex = Regex::new(r"\*([^*]+)\*").unwrap();
        result = italic_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[1].italic().to_string()
        }).to_string();
        
        result
    }
    
    fn apply_basic_markdown(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // Bold text
        let bold_regex = Regex::new(r"\*\*(.*?)\*\*").unwrap();
        result = bold_regex.replace_all(&result, |caps: &regex::Captures| {
            caps[1].bold().to_string()
        }).to_string();
        
        // Inline code
        let code_regex = Regex::new(r"`(.*?)`").unwrap();
        result = code_regex.replace_all(&result, |caps: &regex::Captures| {
            format!("{}", caps[1].on_black().white())
        }).to_string();
        
        result
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
                    _ => None,
                }
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        
        let theme = &self.theme_set.themes["base16-ocean.dark"];
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
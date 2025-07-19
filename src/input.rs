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

use std::time::Instant;
use std::collections::HashMap;
use anyhow::Result;
use arboard::Clipboard;
use rdev::Key;
use regex::Regex;
use lazy_static::lazy_static;
use crate::types::{AppEvent, KeyState, ContentAnalysis};

const DOUBLE_TAP_WINDOW_MS: u64 = 500;
const CONTINUATION_CHECK_WINDOW_MS: u64 = 100;

#[derive(Debug, Clone)]
pub struct PendingEvent {
    pub event: AppEvent,
    pub timestamp: Instant,
    pub key: Key,
}

pub struct KeyboardHandler {
    key_states: HashMap<String, KeyState>,
    last_event_time: Instant,
    pending_events: Vec<PendingEvent>,
    last_key_press: Instant,
}

impl KeyboardHandler {
    pub fn new() -> Self {
        Self {
            key_states: HashMap::new(),
            last_event_time: Instant::now(),
            pending_events: Vec::new(),
            last_key_press: Instant::now(),
        }
    }
    
    pub fn handle_key_press(&mut self, key: Key) -> Option<AppEvent> {
        let now = Instant::now();
        
        // Track all key presses for continuation detection
        self.last_key_press = now;
        
        // Check if this key press invalidates any pending events
        self.check_pending_events_for_continuation(key, now);
        
        // Debounce rapid key presses
        if now.duration_since(self.last_event_time).as_millis() < 50 {
            return None;
        }
        self.last_event_time = now;
        
        // Handle special key combinations first
        match key {
            Key::KeyH => {
                // We can't easily detect Ctrl+H in rdev, so we'll use a different approach
                // For now, just detect H key double-tap for history
                if self.is_double_tap(key, now) {
                    println!("🔍 Double-tap 'H' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::ShowHistory, key, now);
                    return None; // Don't emit immediately
                }
            }
            Key::KeyR => {
                // Check for double-tap R (cancel) vs Ctrl+R (clear context)
                if self.is_double_tap(key, now) {
                    println!("🛑 Double-tap 'R' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::Cancel, key, now);
                    return None; // Don't emit immediately
                }
            }
            _ => {}
        }
        
        // Handle main action keys
        match key {
            Key::KeyA => {
                if self.is_double_tap(key, now) {
                    println!("🎤 Double-tap 'A' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::AudioCapture, key, now);
                    return None; // Don't emit immediately
                }
            }
            Key::KeyS => {
                if self.is_double_tap(key, now) {
                    println!("📋 Double-tap 'S' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::ClipboardAnalysis, key, now);
                    return None; // Don't emit immediately
                }
            }
            Key::KeyQ => {
                if self.is_double_tap(key, now) {
                    println!("🔗 Double-tap 'Q' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::CombinedMode, key, now);
                    return None; // Don't emit immediately
                }
            }
            Key::KeyW => {
                if self.is_double_tap(key, now) {
                    println!("📸 Double-tap 'W' detected - checking for continuation...");
                    self.add_pending_event(AppEvent::ScreenshotMode, key, now);
                    return None; // Don't emit immediately
                }
            }
            _ => {}
        }
        
        None
    }
    
    /// Check for pending events that are ready to be emitted
    /// This should be called periodically to check if any pending events have passed the continuation window
    pub fn check_pending_events(&mut self) -> Option<AppEvent> {
        let now = Instant::now();
        
        // Find events that have passed the continuation window
        let mut ready_events = Vec::new();
        let mut remaining_events = Vec::new();
        
        for event in self.pending_events.drain(..) {
            let time_since_event = now.duration_since(event.timestamp);
            
            if time_since_event.as_millis() >= CONTINUATION_CHECK_WINDOW_MS as u128 {
                // Event has passed the continuation window
                ready_events.push(event);
            } else {
                // Event is still in the continuation window
                remaining_events.push(event);
            }
        }
        
        // Restore remaining events
        self.pending_events = remaining_events;
        
        // Return the first ready event (if any)
        if let Some(event) = ready_events.first() {
            println!("✅ Hotkey confirmed - no continuation detected");
            Some(event.event.clone())
        } else {
            None
        }
    }
    
    /// Add a pending event that needs to be confirmed
    fn add_pending_event(&mut self, event: AppEvent, key: Key, timestamp: Instant) {
        self.pending_events.push(PendingEvent {
            event,
            timestamp,
            key,
        });
    }
    
    /// Check if a key press invalidates any pending events
    fn check_pending_events_for_continuation(&mut self, key: Key, now: Instant) {
        let mut remaining_events = Vec::new();
        
        for event in self.pending_events.drain(..) {
            let time_since_event = now.duration_since(event.timestamp);
            
            // If this is a different key pressed within the continuation window, cancel the event
            if time_since_event.as_millis() < CONTINUATION_CHECK_WINDOW_MS as u128 {
                let current_key_name = format!("{:?}", key);
                let event_key_name = format!("{:?}", event.key);
                
                if current_key_name != event_key_name {
                    println!("🚫 Hotkey cancelled - continued typing detected ({:?} -> {:?})", event.key, key);
                    // Don't add this event back to remaining_events (effectively cancelling it)
                } else {
                    // Same key pressed again, keep the event
                    remaining_events.push(event);
                }
            } else {
                // Event is outside the continuation window, keep it
                remaining_events.push(event);
            }
        }
        
        self.pending_events = remaining_events;
    }
    
    fn is_double_tap(&mut self, key: Key, now: Instant) -> bool {
        let key_name = format!("{:?}", key);
        let state = self.key_states.entry(key_name).or_insert_with(KeyState::default);
        
        let time_since_last = now.duration_since(state.last_press);
        
        if time_since_last.as_millis() <= DOUBLE_TAP_WINDOW_MS as u128 {
            // This is the second tap - reset and return true
            state.tap_count = 0;
            state.last_press = now;
            true
        } else {
            // This is the first tap or too long since last tap
            state.tap_count = 1;
            state.last_press = now;
            false
        }
    }
}

pub struct ClipboardHandler {
    clipboard: Clipboard,
}

impl ClipboardHandler {
    pub fn new() -> Self {
        Self {
            clipboard: Clipboard::new().expect("Failed to initialize clipboard"),
        }
    }
    
    pub async fn read_clipboard(&mut self) -> Result<Option<String>> {
        match self.clipboard.get_text() {
            Ok(content) => {
                if content.trim().is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(content))
                }
            }
            Err(_) => Ok(None),
        }
    }
    
    pub fn analyze_content_type(&self, content: &str) -> ContentAnalysis {
        let text = content.to_lowercase().trim().to_string();
        
        // Language pattern matching
        let language_patterns = self.get_language_patterns();
        let mut best_match = ContentAnalysis {
            content_type: "text".to_string(),
            language: "text".to_string(),
            confidence: 0.0,
        };
        
        // Check each language
        for (language, patterns) in language_patterns {
            let mut matches = 0;
            for pattern in &patterns {
                if pattern.is_match(&text) {
                    matches += 1;
                }
            }
            
            let confidence = matches as f32 / patterns.len() as f32;
            if confidence > best_match.confidence {
                best_match.language = language.to_string();
                best_match.confidence = confidence;
            }
        }
        
        // Determine content type based on language and patterns
        if best_match.confidence > 0.3 {
            best_match.content_type = match best_match.language.as_str() {
                "json" | "xml" | "yaml" => "data".to_string(),
                "html" | "css" => "markup".to_string(),
                "sql" => "query".to_string(),
                "shell" | "bash" => "script".to_string(),
                _ => "code".to_string(),
            };
        } else {
            // Check for general code indicators
            let code_indicators = ["{", "}", ";", "()", "[]", "//", "/*", "*/", "=", "==", "!=", "&&", "||"];
            let code_score = code_indicators.iter()
                .filter(|&indicator| text.contains(indicator))
                .count();
            
            if code_score >= 3 {
                best_match.content_type = "code".to_string();
                best_match.language = "unknown".to_string();
                best_match.confidence = (code_score as f32 / 10.0).min(0.7);
            } else {
                best_match.content_type = "text".to_string();
            }
        }
        
        best_match
    }
    
    fn get_language_patterns(&self) -> HashMap<&'static str, Vec<Regex>> {
        lazy_static! {
            static ref PATTERNS: HashMap<&'static str, Vec<Regex>> = {
                let mut patterns = HashMap::new();
                
                // JavaScript patterns
                patterns.insert("javascript", vec![
                    Regex::new(r"function\s*\(").unwrap(),
                    Regex::new(r"=>\s*\{").unwrap(),
                    Regex::new(r"const\s+\w+").unwrap(),
                    Regex::new(r"let\s+\w+").unwrap(),
                    Regex::new(r"var\s+\w+").unwrap(),
                    Regex::new(r"console\.log").unwrap(),
                    Regex::new(r"require\(").unwrap(),
                    Regex::new(r"import\s+.*from").unwrap(),
                ]);
                
                // TypeScript patterns
                patterns.insert("typescript", vec![
                    Regex::new(r"interface\s+\w+").unwrap(),
                    Regex::new(r"type\s+\w+\s*=").unwrap(),
                    Regex::new(r":\s*string").unwrap(),
                    Regex::new(r":\s*number").unwrap(),
                    Regex::new(r":\s*boolean").unwrap(),
                    Regex::new(r"<.*>").unwrap(),
                ]);
                
                // Python patterns
                patterns.insert("python", vec![
                    Regex::new(r"def\s+\w+\(").unwrap(),
                    Regex::new(r"import\s+\w+").unwrap(),
                    Regex::new(r"from\s+\w+\s+import").unwrap(),
                    Regex::new(r"if\s+__name__\s*==").unwrap(),
                    Regex::new(r"print\(").unwrap(),
                    Regex::new(r"class\s+\w+:").unwrap(),
                ]);
                
                // Java patterns
                patterns.insert("java", vec![
                    Regex::new(r"public\s+class").unwrap(),
                    Regex::new(r"private\s+\w+").unwrap(),
                    Regex::new(r"public\s+static\s+void\s+main").unwrap(),
                    Regex::new(r"System\.out\.println").unwrap(),
                    Regex::new(r"extends\s+\w+").unwrap(),
                    Regex::new(r"implements\s+\w+").unwrap(),
                ]);
                
                // C++ patterns
                patterns.insert("cpp", vec![
                    Regex::new(r"#include\s*<").unwrap(),
                    Regex::new(r"std::").unwrap(),
                    Regex::new(r"cout\s*<<").unwrap(),
                    Regex::new(r"int\s+main\(").unwrap(),
                    Regex::new(r"namespace\s+\w+").unwrap(),
                    Regex::new(r"using\s+namespace").unwrap(),
                ]);
                
                // HTML patterns
                patterns.insert("html", vec![
                    Regex::new(r"<html").unwrap(),
                    Regex::new(r"<head").unwrap(),
                    Regex::new(r"<body").unwrap(),
                    Regex::new(r"<div").unwrap(),
                    Regex::new(r"<span").unwrap(),
                    Regex::new(r"<script").unwrap(),
                    Regex::new(r"<style").unwrap(),
                ]);
                
                // CSS patterns
                patterns.insert("css", vec![
                    Regex::new(r"\{[^}]*\}").unwrap(),
                    Regex::new(r"\.[a-zA-Z][\w-]*\s*\{").unwrap(),
                    Regex::new(r"#[a-zA-Z][\w-]*\s*\{").unwrap(),
                    Regex::new(r"@media").unwrap(),
                    Regex::new(r"color\s*:").unwrap(),
                    Regex::new(r"font-size\s*:").unwrap(),
                ]);
                
                // SQL patterns
                patterns.insert("sql", vec![
                    Regex::new(r"select\s+.*from").unwrap(),
                    Regex::new(r"insert\s+into").unwrap(),
                    Regex::new(r"update\s+.*set").unwrap(),
                    Regex::new(r"delete\s+from").unwrap(),
                    Regex::new(r"create\s+table").unwrap(),
                    Regex::new(r"alter\s+table").unwrap(),
                ]);
                
                // JSON patterns
                patterns.insert("json", vec![
                    Regex::new(r"^\s*\{").unwrap(),
                    Regex::new(r"^\s*\[").unwrap(),
                    Regex::new(r#""[^"]*"\s*:"#).unwrap(),
                    Regex::new(r"}\s*,\s*\{").unwrap(),
                ]);
                
                // XML patterns
                patterns.insert("xml", vec![
                    Regex::new(r"<\?xml").unwrap(),
                    Regex::new(r"<\w+[^>]*>").unwrap(),
                    Regex::new(r"</\w+>").unwrap(),
                ]);
                
                // YAML patterns
                patterns.insert("yaml", vec![
                    Regex::new(r"^[\w-]+\s*:").unwrap(),
                    Regex::new(r"^\s*-\s+\w+").unwrap(),
                    Regex::new(r"---").unwrap(),
                    Regex::new(r"\.\.\.").unwrap(),
                ]);
                
                // Shell patterns
                patterns.insert("shell", vec![
                    Regex::new(r"#!/").unwrap(),
                    Regex::new(r"\$\w+").unwrap(),
                    Regex::new(r"echo\s+").unwrap(),
                    Regex::new(r"grep\s+").unwrap(),
                    Regex::new(r"awk\s+").unwrap(),
                    Regex::new(r"sed\s+").unwrap(),
                    Regex::new(r"if\s*\[\s*").unwrap(),
                    Regex::new(r"fi$").unwrap(),
                ]);
                
                // PHP patterns
                patterns.insert("php", vec![
                    Regex::new(r"<\?php").unwrap(),
                    Regex::new(r"\$\w+").unwrap(),
                    Regex::new(r"function\s+\w+\(").unwrap(),
                    Regex::new(r"class\s+\w+").unwrap(),
                    Regex::new(r"echo\s+").unwrap(),
                    Regex::new(r"print\s+").unwrap(),
                ]);
                
                // Ruby patterns
                patterns.insert("ruby", vec![
                    Regex::new(r"def\s+\w+").unwrap(),
                    Regex::new(r"class\s+\w+").unwrap(),
                    Regex::new(r"puts\s+").unwrap(),
                    Regex::new(r"require\s+").unwrap(),
                    Regex::new(r"end$").unwrap(),
                ]);
                
                // Go patterns
                patterns.insert("go", vec![
                    Regex::new(r"package\s+\w+").unwrap(),
                    Regex::new(r"func\s+\w+\(").unwrap(),
                    Regex::new(r"import\s+\(").unwrap(),
                    Regex::new(r"fmt\.Print").unwrap(),
                    Regex::new(r"var\s+\w+\s+\w+").unwrap(),
                ]);
                
                // Rust patterns
                patterns.insert("rust", vec![
                    Regex::new(r"fn\s+\w+\(").unwrap(),
                    Regex::new(r"let\s+\w+").unwrap(),
                    Regex::new(r"use\s+\w+").unwrap(),
                    Regex::new(r"struct\s+\w+").unwrap(),
                    Regex::new(r"impl\s+\w+").unwrap(),
                    Regex::new(r"println!").unwrap(),
                ]);
                
                patterns
            };
        }
        
        PATTERNS.clone()
    }
} 
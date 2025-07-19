/*
 * Meeting Assistant CLI - Sentiment Analyzer Plugin Example
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
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::plugin_system::*;
use crate::types::*;

/// Simple sentiment analysis plugin
pub struct SentimentAnalyzerPlugin {
    positive_keywords: Vec<String>,
    negative_keywords: Vec<String>,
    enabled: bool,
}

impl SentimentAnalyzerPlugin {
    pub fn new() -> Self {
        Self {
            positive_keywords: vec![
                "great".to_string(),
                "excellent".to_string(),
                "good".to_string(),
                "amazing".to_string(),
                "fantastic".to_string(),
                "awesome".to_string(),
                "love".to_string(),
                "perfect".to_string(),
                "brilliant".to_string(),
                "outstanding".to_string(),
            ],
            negative_keywords: vec![
                "bad".to_string(),
                "terrible".to_string(),
                "awful".to_string(),
                "horrible".to_string(),
                "hate".to_string(),
                "worst".to_string(),
                "disappointing".to_string(),
                "frustrating".to_string(),
                "annoying".to_string(),
                "broken".to_string(),
            ],
            enabled: true,
        }
    }
    
    fn analyze_sentiment(&self, text: &str) -> SentimentResult {
        let text_lower = text.to_lowercase();
        
        let positive_count = self.positive_keywords.iter()
            .filter(|word| text_lower.contains(word.as_str()))
            .count();
        
        let negative_count = self.negative_keywords.iter()
            .filter(|word| text_lower.contains(word.as_str()))
            .count();
        
        let sentiment = if positive_count > negative_count {
            if positive_count > 2 {
                Sentiment::VeryPositive
            } else {
                Sentiment::Positive
            }
        } else if negative_count > positive_count {
            if negative_count > 2 {
                Sentiment::VeryNegative
            } else {
                Sentiment::Negative
            }
        } else {
            Sentiment::Neutral
        };
        
        let confidence = if positive_count + negative_count > 0 {
            (positive_count + negative_count) as f32 / 10.0
        } else {
            0.0
        }.min(1.0);
        
        SentimentResult {
            sentiment,
            confidence,
            positive_keywords: self.positive_keywords.iter()
                .filter(|word| text_lower.contains(word.as_str()))
                .cloned()
                .collect(),
            negative_keywords: self.negative_keywords.iter()
                .filter(|word| text_lower.contains(word.as_str()))
                .cloned()
                .collect(),
        }
    }
}

#[async_trait]
impl Plugin for SentimentAnalyzerPlugin {
    fn name(&self) -> &str {
        "sentiment_analyzer"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "Analyzes sentiment in meeting conversations and provides emotional context"
    }
    
    fn author(&self) -> &str {
        "Meeting Assistant Team"
    }
    
    async fn initialize(&mut self, context: &PluginContext) -> Result<()> {
        // Load custom keywords from plugin data if available
        let plugin_data = context.plugin_data.read().await;
        if let Some(data) = plugin_data.get("sentiment_analyzer") {
            if let Ok(config) = serde_json::from_value::<SentimentConfig>(data.clone()) {
                self.positive_keywords = config.positive_keywords;
                self.negative_keywords = config.negative_keywords;
                self.enabled = config.enabled;
            }
        }
        
        println!("🎭 Sentiment Analyzer Plugin initialized");
        println!("   Positive keywords: {}", self.positive_keywords.len());
        println!("   Negative keywords: {}", self.negative_keywords.len());
        
        Ok(())
    }
    
    async fn cleanup(&mut self, _context: &PluginContext) -> Result<()> {
        println!("🎭 Sentiment Analyzer Plugin cleaned up");
        Ok(())
    }
    
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginContext,
    ) -> Result<PluginHookResult> {
        if !self.enabled {
            return Ok(PluginHookResult::Continue);
        }
        
        match event {
            PluginEvent::ContentAnalyzed { content, analysis } => {
                // Analyze sentiment of the content
                let sentiment_result = self.analyze_sentiment(content);
                
                // Store the sentiment analysis in plugin data
                let mut plugin_data = context.plugin_data.write().await;
                plugin_data.insert(
                    "last_sentiment".to_string(),
                    serde_json::to_value(&sentiment_result)?,
                );
                
                // If sentiment is very positive or very negative, add it to the analysis
                if matches!(sentiment_result.sentiment, Sentiment::VeryPositive | Sentiment::VeryNegative) {
                    println!("🎭 Strong sentiment detected: {:?} (confidence: {:.2})", 
                        sentiment_result.sentiment, sentiment_result.confidence);
                    
                    // Modify the content analysis to include sentiment
                    let enhanced_data = json!({
                        "original_analysis": analysis,
                        "sentiment": sentiment_result,
                        "enhanced_by": "sentiment_analyzer"
                    });
                    
                    return Ok(PluginHookResult::Modify(enhanced_data));
                }
                
                Ok(PluginHookResult::Continue)
            }
            
            PluginEvent::PromptStreamComplete { response } => {
                // Analyze sentiment of AI responses
                let sentiment_result = self.analyze_sentiment(response);
                
                if sentiment_result.confidence > 0.3 {
                    println!("🎭 AI response sentiment: {:?} (confidence: {:.2})", 
                        sentiment_result.sentiment, sentiment_result.confidence);
                }
                
                Ok(PluginHookResult::Continue)
            }
            
            PluginEvent::SessionHistoryUpdated { entry } => {
                // Track sentiment trends over the session
                let input_sentiment = self.analyze_sentiment(&entry.input);
                let response_sentiment = self.analyze_sentiment(&entry.response);
                
                // Store sentiment trends
                let mut plugin_data = context.plugin_data.write().await;
                let mut trends = plugin_data.get("sentiment_trends")
                    .and_then(|v| serde_json::from_value::<Vec<SentimentTrend>>(v.clone()).ok())
                    .unwrap_or_default();
                
                trends.push(SentimentTrend {
                    timestamp: entry.timestamp,
                    input_sentiment: input_sentiment.sentiment,
                    response_sentiment: response_sentiment.sentiment,
                    confidence: (input_sentiment.confidence + response_sentiment.confidence) / 2.0,
                });
                
                // Keep only last 10 trends
                if trends.len() > 10 {
                    trends.remove(0);
                }
                
                plugin_data.insert(
                    "sentiment_trends".to_string(),
                    serde_json::to_value(&trends)?,
                );
                
                Ok(PluginHookResult::Continue)
            }
            
            _ => Ok(PluginHookResult::Continue),
        }
    }
    
    fn subscribed_events(&self) -> Vec<PluginEvent> {
        vec![
            PluginEvent::ContentAnalyzed { 
                content: String::new(), 
                analysis: ContentAnalysis {
                    content_type: String::new(),
                    language: String::new(),
                    confidence: 0.0,
                }
            },
            PluginEvent::PromptStreamComplete { response: String::new() },
            PluginEvent::SessionHistoryUpdated { 
                entry: SessionEntry {
                    timestamp: chrono::Utc::now(),
                    input: String::new(),
                    response: String::new(),
                    question_type: QuestionType::Audio,
                    confidence: 0.0,
                    key_topics: vec![],
                }
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
                    "description": "Enable/disable sentiment analysis"
                },
                "positive_keywords": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of positive keywords"
                },
                "negative_keywords": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of negative keywords"
                }
            }
        }))
    }
    
    fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        // Validate the configuration
        if let Some(enabled) = config.get("enabled") {
            if !enabled.is_boolean() {
                return Err(anyhow::anyhow!("'enabled' must be a boolean"));
            }
        }
        
        if let Some(positive_keywords) = config.get("positive_keywords") {
            if !positive_keywords.is_array() {
                return Err(anyhow::anyhow!("'positive_keywords' must be an array"));
            }
        }
        
        if let Some(negative_keywords) = config.get("negative_keywords") {
            if !negative_keywords.is_array() {
                return Err(anyhow::anyhow!("'negative_keywords' must be an array"));
            }
        }
        
        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SentimentConfig {
    enabled: bool,
    positive_keywords: Vec<String>,
    negative_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResult {
    pub sentiment: Sentiment,
    pub confidence: f32,
    pub positive_keywords: Vec<String>,
    pub negative_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Sentiment {
    VeryPositive,
    Positive,
    Neutral,
    Negative,
    VeryNegative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentTrend {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub input_sentiment: Sentiment,
    pub response_sentiment: Sentiment,
    pub confidence: f32,
}

// Example plugin command to show sentiment trends
pub fn show_sentiment_trends(context: &PluginContext) -> Result<()> {
    let plugin_data = context.plugin_data.try_read()
        .map_err(|_| anyhow::anyhow!("Failed to read plugin data"))?;
    
    if let Some(trends_data) = plugin_data.get("sentiment_trends") {
        let trends: Vec<SentimentTrend> = serde_json::from_value(trends_data.clone())?;
        
        println!("📊 Sentiment Trends:");
        for trend in trends {
            println!("  {} | Input: {:?} | Response: {:?} | Confidence: {:.2}",
                trend.timestamp.format("%H:%M:%S"),
                trend.input_sentiment,
                trend.response_sentiment,
                trend.confidence
            );
        }
    } else {
        println!("No sentiment trends available");
    }
    
    Ok(())
} 
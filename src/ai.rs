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
use reqwest::Client;
use serde_json::{json, Value};
use futures::StreamExt;
use base64;
use std::fs;
use std::path::PathBuf;
use regex::Regex;
use crate::config::Config;
use crate::types::{
    OpenAIConfig, ContentAnalysis, QuestionAnalysis, 
    get_all_technologies, TECHNOLOGY_ABBREVIATIONS
};

pub struct OpenAIClient {
    client: Client,
    config: OpenAIConfig,
}

impl OpenAIClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let client = Client::new();
        
        Ok(Self {
            client,
            config: config.openai.clone(),
        })
    }
    
    pub async fn transcribe_audio(&self, audio_file: &PathBuf) -> Result<String> {
        let file_data = fs::read(audio_file)
            .context("Failed to read audio file")?;
        
        let file_name = audio_file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav");
        
        let form = reqwest::multipart::Form::new()
            .part("file", 
                reqwest::multipart::Part::bytes(file_data)
                    .file_name(file_name.to_string())
                    .mime_str("audio/wav")?
            )
            .text("model", "whisper-1")
            .text("language", "en")
            .text("response_format", "text");
        
        let response = self.client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("OpenAI Whisper API error: {}", error_text));
        }
        
        let transcript = response.text().await?;
        
        // Note: Don't delete the audio file here as it might be needed by other transcription methods
        // File cleanup should happen at the application level after all transcription attempts
        
        Ok(transcript.trim().to_string())
    }
    
    pub async fn generate_meeting_support(
        &self,
        transcript: &str,
        conversation_context: &str,
    ) -> Result<String> {
        // First classify the content
        let analysis = self.classify_content(transcript).await?;
        
        match analysis.question_type.as_str() {
            "question_or_request" => {
                self.generate_question_response(transcript, &analysis, conversation_context).await
            }
            "discussion_point" => {
                self.generate_discussion_response(transcript, &analysis, conversation_context).await
            }
            _ => {
                self.generate_general_meeting_response(transcript, &analysis, conversation_context).await
            }
        }
    }
    
    pub async fn classify_content(&self, transcript: &str) -> Result<QuestionAnalysis> {
        // Try quick classification first
        let quick_result = self.quick_classify_content(transcript);
        if quick_result.confidence > 0.7 {
            return Ok(quick_result);
        }
        
        // Fallback to AI classification
        let system_prompt = "Meeting content classifier. JSON only.";
        
        let user_prompt = format!(
            r#"Classify meeting content. JSON only:

{{"type": "question_or_request|discussion_point|action_item|general", "strategy": "brief strategy", "confidence": 0.0-1.0, "key_topics": ["topic1"]}}

Types:
- question_or_request: Direct questions, requests for information or clarification
- discussion_point: Topics being discussed, ideas being explored
- action_item: Tasks, decisions, or items requiring follow-up
- general: General conversation, unclear content

"{}""#,
            transcript
        );
        
        let response = self.simple_completion(&system_prompt, &user_prompt, 150).await?;
        
        match serde_json::from_str::<QuestionAnalysis>(&response) {
            Ok(analysis) => Ok(analysis),
            Err(_) => Ok(quick_result), // Fallback to quick classification
        }
    }
    
    fn quick_classify_content(&self, transcript: &str) -> QuestionAnalysis {
        let text = transcript.to_lowercase();
        
        // Question/request keywords
        let question_keywords = [
            "what", "why", "how", "when", "where", "who", "can you", "could you", 
            "would you", "explain", "clarify", "help", "question", "ask"
        ];
        let question_score = question_keywords.iter()
            .filter(|&&keyword| text.contains(keyword))
            .count();
        
        // Discussion keywords
        let discussion_keywords = [
            "think", "believe", "opinion", "consider", "discuss", "talk about",
            "idea", "proposal", "suggest", "recommend", "maybe", "perhaps"
        ];
        let discussion_score = discussion_keywords.iter()
            .filter(|&&keyword| text.contains(keyword))
            .count();
        
        // Action item keywords  
        let action_keywords = [
            "need to", "should", "must", "have to", "action", "task", "todo",
            "follow up", "next step", "deadline", "assign", "responsible"
        ];
        let action_score = action_keywords.iter()
            .filter(|&&keyword| text.contains(keyword))
            .count();
        
        let (question_type, confidence, key_topics) = if question_score >= 1 {
            let confidence = (0.6 + question_score as f32 * 0.1).min(0.9);
            let topics = question_keywords.iter()
                .filter(|&&keyword| text.contains(keyword))
                .take(3)
                .map(|&s| s.to_string())
                .collect();
            ("question_or_request".to_string(), confidence, topics)
        } else if discussion_score >= 1 {
            let confidence = (0.6 + discussion_score as f32 * 0.1).min(0.9);
            let topics = discussion_keywords.iter()
                .filter(|&&keyword| text.contains(keyword))
                .take(3)
                .map(|&s| s.to_string())
                .collect();
            ("discussion_point".to_string(), confidence, topics)
        } else if action_score >= 1 {
            let confidence = (0.6 + action_score as f32 * 0.1).min(0.8);
            let topics = action_keywords.iter()
                .filter(|&&keyword| text.contains(keyword))
                .take(3)
                .map(|&s| s.to_string())
                .collect();
            ("action_item".to_string(), confidence, topics)
        } else {
            ("general".to_string(), 0.5, vec![])
        };
        
        QuestionAnalysis {
            question_type: question_type.clone(),
            strategy: format!("Quick {} classification", question_type),
            confidence,
            key_topics,
        }
    }
    
    async fn generate_question_response(
        &self,
        transcript: &str,
        _analysis: &QuestionAnalysis,
        conversation_context: &str,
    ) -> Result<String> {
        let context_preview = if !conversation_context.is_empty() {
            format!("MEETING CONTEXT: {}", conversation_context.lines().take(2).collect::<Vec<_>>().join(" "))
        } else {
            String::new()
        };
        
        let system_prompt = format!(
            r#"Expert meeting assistant. Someone has asked a question or made a request. Provide helpful, actionable information.

{}

FORMAT:
**UNDERSTANDING:** What they're asking for
**QUICK ANSWER:** Direct response to the question/request
**ADDITIONAL CONTEXT:** Relevant background information
**SUGGESTIONS:** Next steps or related considerations

Focus on:
- Direct, clear answers to questions
- Practical information and context
- Actionable suggestions
- Connecting to broader meeting topics when relevant

Keep responses concise but comprehensive. Use bullet points and clear formatting."#,
            context_preview
        );
        
        let user_prompt = format!(
            r#"Question/Request: "{}"

Provide a helpful response that directly addresses what was asked."#,
            transcript
        );
        
        self.stream_completion(&system_prompt, &user_prompt, 900).await
    }
    
    async fn generate_discussion_response(
        &self,
        transcript: &str,
        _analysis: &QuestionAnalysis,
        conversation_context: &str,
    ) -> Result<String> {
        let context_preview = if !conversation_context.is_empty() {
            format!("MEETING CONTEXT: {}", conversation_context.lines().take(2).collect::<Vec<_>>().join(" "))
        } else {
            String::new()
        };
        
        let system_prompt = format!(
            r#"Expert meeting assistant. Someone has raised a discussion point or topic. Provide context, analysis, and thoughtful input.

{}

FORMAT:
**TOPIC SUMMARY:** What's being discussed
**KEY CONSIDERATIONS:** Important factors to consider
**DIFFERENT PERSPECTIVES:** Various viewpoints on the topic
**POTENTIAL OUTCOMES:** Possible results or implications

Focus on:
- Balanced analysis of the topic
- Multiple perspectives and considerations
- Practical implications
- Questions that might help deepen the discussion

Use clear formatting and bullet points. Be objective and helpful."#,
            context_preview
        );
        
        let user_prompt = format!(r#"Discussion Point: "{}""#, transcript);
        
        self.stream_completion(&system_prompt, &user_prompt, 800).await
    }
    
    async fn generate_general_meeting_response(
        &self,
        transcript: &str,
        _analysis: &QuestionAnalysis,
        conversation_context: &str,
    ) -> Result<String> {
        let context_preview = if !conversation_context.is_empty() {
            format!("MEETING CONTEXT: {}", conversation_context.lines().take(2).collect::<Vec<_>>().join(" "))
        } else {
            String::new()
        };
        
        let system_prompt = format!(
            r#"Expert meeting assistant. Provide general meeting support with clear formatting.

{}

FORMAT:
**CONTEXT:** What was said
**RELEVANT INFO:** Background information that might be helpful
**CONNECTIONS:** How this relates to broader meeting topics
**SUGGESTIONS:** Potential next steps or considerations

Focus on being helpful and contextual."#,
            context_preview
        );
        
        let user_prompt = format!(r#"Meeting Content: "{}""#, transcript);
        
        self.stream_completion(&system_prompt, &user_prompt, 600).await
    }
    
    pub async fn generate_code_analysis(
        &self,
        code: &str,
        analysis: &ContentAnalysis,
        code_context: &str,
    ) -> Result<String> {
        let system_prompt = format!(
            r#"Expert code analyst and meeting assistant. Analyze the provided code with markdown formatting.

ANALYSIS FRAMEWORK:
**WHAT IT DOES:** Clear explanation of the code's purpose
**KEY CONCEPTS:** Important programming concepts demonstrated
**POTENTIAL ISSUES:** Bugs, inefficiencies, or improvements needed
**MEETING CONTEXT:** How this code relates to the discussion at hand
**IMPROVEMENTS:** Specific suggestions for optimization or best practices

Focus on:
- Code correctness and potential bugs
- Performance implications
- Best practices and code quality
- How this code fits into the broader discussion
- Practical recommendations for improvement
{}

Language: {}
Content Type: {}

Keep explanations clear and accessible. Use code formatting for code snippets.

{}"#,
            if !code_context.is_empty() { "- References to previous code when relevant" } else { "" },
            analysis.language,
            analysis.content_type,
            code_context
        );
        
        let user_prompt = format!(
            r#"Analyze this {} {}:

```{}
{}
```"#,
            analysis.language,
            analysis.content_type,
            analysis.language,
            code
        );
        
        self.stream_completion(&system_prompt, &user_prompt, 1600).await
    }
    
    pub async fn generate_code_with_audio_analysis(
        &self,
        transcript: &str,
        code: &str,
        analysis: &ContentAnalysis,
        code_context: &str,
    ) -> Result<String> {
        let system_prompt = format!(
            r#"Expert code analyst and meeting assistant. You have both AUDIO CONTEXT and CODE to work with.

MISSION: Provide helpful code analysis that addresses what was discussed in the audio context.

RESPONSE FRAMEWORK:
**UNDERSTANDING:** What the audio is asking about the code
**CODE ANALYSIS:** Detailed analysis of the code with explanations
**SOLUTIONS/IMPROVEMENTS:** If solving a problem, provide the solution with detailed explanations
**MEETING RELEVANCE:** How this code relates to the broader discussion

FOCUS ON:
- Clear explanations of what the code does
- Inline comments for complex logic or algorithms  
- Explanations of why certain approaches are used
- Performance and optimization considerations
- Best practices and potential improvements
- If it's problematic code, provide fixes with explanations
{}

Language: {}
Content Type: {}

Use proper code formatting with ```{} blocks. Make explanations clear and accessible.

{}"#,
            if !code_context.is_empty() { "- Reference to previous code snippets when relevant" } else { "" },
            analysis.language,
            analysis.content_type,
            analysis.language,
            code_context
        );
        
        let user_prompt = format!(
            r#"AUDIO CONTEXT: "{}"

CODE TO ANALYZE:
```{}
{}
```

Please provide code analysis that addresses what was mentioned in the audio context."#,
            transcript,
            analysis.language,
            code
        );
        
        self.stream_completion(&system_prompt, &user_prompt, 1800).await
    }
    
    pub async fn generate_screenshot_with_audio_analysis(
        &self,
        audio_context: &str,
        screenshot_path: &PathBuf,
    ) -> Result<String> {
        let image_base64 = self.convert_image_to_base64(screenshot_path).await?;
        
        // Clean up screenshot after conversion
        let _ = fs::remove_file(screenshot_path);
        
        let system_prompt = r#"Expert visual analyst and meeting assistant. You have both AUDIO CONTEXT and a SCREENSHOT to analyze.

MISSION: Analyze the screenshot content in the context of what was said in the audio, providing insights that help with understanding or solving what's shown.

RESPONSE FRAMEWORK:
**WHAT I SEE:** Detailed description of visual elements (UI, code, diagrams, text, etc.)
**AUDIO CONTEXT:** How the spoken words relate to what's shown
**KEY INSIGHTS:** Important observations that connect audio and visual
**EXPLANATIONS:** Technical concepts or processes visible in the screenshot
**RECOMMENDATIONS:** Suggested actions or next steps based on the analysis

FOCUS ON:
- UI elements, buttons, menus, and interface components
- Code structure, syntax, and any visible errors or warnings
- Diagrams, charts, or visual representations
- Text content, documentation, or error messages
- System states, process flows, or application behavior
- How the audio question/comment relates to what's visible
- Practical recommendations for addressing what's shown

Be detailed about what you observe visually, then connect it meaningfully to the audio context."#;
        
        let messages = json!([
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": format!("AUDIO CONTEXT: \"{}\"\n\nPlease analyze the screenshot in the context of what was said.", audio_context)
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", image_base64),
                            "detail": "high"
                        }
                    }
                ]
            }
        ]);
        
        self.stream_completion_with_messages(&messages, 2000).await
    }
    
    async fn convert_image_to_base64(&self, image_path: &PathBuf) -> Result<String> {
        let image_data = fs::read(image_path)
            .context("Failed to read screenshot file")?;
        
        use base64::{Engine as _, engine::general_purpose};
        Ok(general_purpose::STANDARD.encode(image_data))
    }
    
    fn extract_technologies_from_question(&self, transcript: &str) -> Vec<String> {
        let text = transcript.to_lowercase();
        let mut found_technologies = std::collections::HashSet::new();
        
        // Get all technologies and sort by length (longest first for better matching)
        let mut all_techs = get_all_technologies();
        all_techs.sort_by(|a, b| b.name.len().cmp(&a.name.len()));
        
        // Find technologies mentioned in the transcript
        for tech in all_techs {
            let regex_pattern = format!(r"\b{}\b", regex::escape(&tech.name));
            if let Ok(regex) = Regex::new(&regex_pattern) {
                if regex.is_match(&text) {
                    found_technologies.insert(tech.name);
                }
            }
        }
        
        // Check for abbreviations
        for (abbrev, full_name) in TECHNOLOGY_ABBREVIATIONS {
            let regex_pattern = format!(r"\b{}\b", regex::escape(abbrev));
            if let Ok(regex) = Regex::new(&regex_pattern) {
                if regex.is_match(&text) {
                    found_technologies.insert(full_name.to_string());
                }
            }
        }
        
        found_technologies.into_iter().take(6).collect()
    }
    
    pub async fn simple_completion(&self, system_prompt: &str, user_prompt: &str, max_tokens: u32) -> Result<String> {
        let request_body = json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "max_tokens": max_tokens,
            "temperature": self.config.temperature,
            "stream": false
        });
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("OpenAI API error: {}", response.status()));
        }
        
        let response_json: Value = response.json().await?;
        
        let content = response_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .unwrap_or("")
            .to_string();
        
        Ok(content)
    }
    
    async fn stream_completion(&self, system_prompt: &str, user_prompt: &str, max_tokens: u32) -> Result<String> {
        let messages = json!([
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]);
        
        self.stream_completion_with_messages(&messages, max_tokens).await
    }
    
    async fn stream_completion_with_messages(&self, messages: &Value, max_tokens: u32) -> Result<String> {
        let request_body = json!({
            "model": self.config.model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": self.config.temperature,
            "stream": true
        });
        
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow::anyhow!("OpenAI API error: {}", response.status()));
        }
        
        let mut stream = response.bytes_stream();
        let mut full_response = String::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            
            // Parse Server-Sent Events format
            for line in chunk_str.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..]; // Remove "data: " prefix
                    
                    if data == "[DONE]" {
                        break;
                    }
                    
                    if let Ok(json_data) = serde_json::from_str::<Value>(data) {
                        if let Some(content) = json_data
                            .get("choices")
                            .and_then(|choices| choices.get(0))
                            .and_then(|choice| choice.get("delta"))
                            .and_then(|delta| delta.get("content"))
                            .and_then(|content| content.as_str())
                        {
                            full_response.push_str(content);
                        }
                    }
                }
            }
        }
        
        Ok(full_response)
    }
} 
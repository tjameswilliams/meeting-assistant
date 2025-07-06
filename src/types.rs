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

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone)]
pub enum AppEvent {
    AudioCapture,
    ClipboardAnalysis,
    CombinedMode,
    ScreenshotMode,
    Cancel,
    ShowHistory,
    ClearContext,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuestionType {
    Audio,
    Code,
    Combined,
    Screenshot,
    PortfolioHistory,
    TechnicalKnowledge,
    Behavioral,
    General,
}

impl fmt::Display for QuestionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuestionType::Audio => write!(f, "audio"),
            QuestionType::Code => write!(f, "code_analysis"),
            QuestionType::Combined => write!(f, "combined"),
            QuestionType::Screenshot => write!(f, "screenshot"),
            QuestionType::PortfolioHistory => write!(f, "portfolio_history"),
            QuestionType::TechnicalKnowledge => write!(f, "technical_knowledge"),
            QuestionType::Behavioral => write!(f, "behavioral"),
            QuestionType::General => write!(f, "general"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub timestamp: DateTime<Utc>,
    pub input: String,
    pub response: String,
    pub question_type: QuestionType,
    pub confidence: f32,
    pub key_topics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    pub timestamp: DateTime<Utc>,
    pub question: String,
    pub question_type: String,
    pub key_topics: Vec<String>,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEntry {
    pub id: usize,
    pub timestamp: DateTime<Utc>,
    pub code: String,
    pub language: String,
    pub analysis_type: String,
    pub description: String,
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct ContentAnalysis {
    pub content_type: String,
    pub language: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionAnalysis {
    pub question_type: String,
    pub strategy: String,
    pub confidence: f32,
    pub key_topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhisperBackend {
    WhisperCpp,
    WhisperBrew,
    FasterWhisper,
    StandardWhisper,
    OpenAIAPI,
}

impl fmt::Display for WhisperBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhisperBackend::WhisperCpp => write!(f, "whisper.cpp"),
            WhisperBackend::WhisperBrew => write!(f, "brew"),
            WhisperBackend::FasterWhisper => write!(f, "faster-whisper"),
            WhisperBackend::StandardWhisper => write!(f, "python"),
            WhisperBackend::OpenAIAPI => write!(f, "openai-api"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemStatus {
    pub audio_ready: bool,
    pub whisper_ready: bool,
    pub whisper_backend: Option<WhisperBackend>,
    pub openai_ready: bool,
}

#[derive(Debug, Clone)]
pub struct KeyState {
    pub last_press: std::time::Instant,
    pub tap_count: usize,
}

impl Default for KeyState {
    fn default() -> Self {
        Self {
            last_press: std::time::Instant::now(),
            tap_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub device_index: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_duration: u64,
    pub capture_duration: u64,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_index: ":7".to_string(), // Default for macOS "Tim's Input"
            sample_rate: 16000,
            channels: 1,
            buffer_duration: 8,
            capture_duration: 15,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            max_tokens: 1800,
            temperature: 0.5,
        }
    }
}

#[derive(Debug)]
pub struct StreamingResponse {
    pub content: String,
    pub is_complete: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Technology {
    pub name: String,
    pub category: String,
}

// Common technology categories and their items
pub const FRONTEND_TECHNOLOGIES: &[&str] = &[
    "react", "vue", "vue.js", "angular", "svelte", "ember", "backbone", "jquery",
    "bootstrap", "tailwind", "material-ui", "ant design", "next.js", "nuxt",
    "gatsby", "astro", "vite", "webpack", "parcel"
];

pub const BACKEND_TECHNOLOGIES: &[&str] = &[
    "node.js", "nodejs", "express", "koa", "fastify", "nest.js", "django",
    "flask", "fastapi", "spring", "spring boot", "laravel", "symfony",
    "ruby on rails", "rails", "phoenix", "gin", "echo", ".net", "asp.net", "core"
];

pub const PROGRAMMING_LANGUAGES: &[&str] = &[
    "javascript", "typescript", "python", "java", "c++", "c#", "go", "rust",
    "php", "ruby", "swift", "kotlin", "scala", "clojure", "elixir", "haskell",
    "dart", "r", "matlab", "perl", "lua"
];

pub const DATABASES: &[&str] = &[
    "postgresql", "postgres", "mysql", "mongodb", "redis", "cassandra",
    "dynamodb", "elasticsearch", "sqlite", "mariadb", "couchdb", "neo4j",
    "influxdb", "graphql", "prisma", "sequelize", "mongoose"
];

pub const CLOUD_TECHNOLOGIES: &[&str] = &[
    "aws", "azure", "google cloud", "gcp", "docker", "kubernetes", "terraform",
    "ansible", "jenkins", "github actions", "gitlab ci", "cloudformation",
    "helm", "istio", "consul", "vault", "nginx", "apache", "load balancer",
    "cdn", "cloudfront", "s3", "ec2", "lambda"
];

pub const DEVOPS_TECHNOLOGIES: &[&str] = &[
    "ci/cd", "continuous integration", "continuous deployment", "microservices",
    "api gateway", "service mesh", "monitoring", "logging", "grafana",
    "prometheus", "elk stack", "datadog", "new relic", "sentry", "git",
    "github", "gitlab", "bitbucket"
];

pub const MOBILE_TECHNOLOGIES: &[&str] = &[
    "react native", "flutter", "ionic", "cordova", "phonegap", "xamarin",
    "native script", "ios", "android", "swift ui", "jetpack compose"
];

pub const TESTING_TECHNOLOGIES: &[&str] = &[
    "jest", "mocha", "cypress", "selenium", "playwright", "vitest",
    "unit testing", "integration testing", "e2e testing", "tdd", "bdd",
    "test driven development", "behavior driven development"
];

pub const DATA_TECHNOLOGIES: &[&str] = &[
    "machine learning", "artificial intelligence", "data science", "pandas",
    "numpy", "scikit-learn", "tensorflow", "pytorch", "jupyter", "apache spark",
    "hadoop", "kafka", "rabbitmq", "etl", "data pipeline", "big data", "analytics"
];

pub const ARCHITECTURE_PATTERNS: &[&str] = &[
    "microservices", "monolith", "serverless", "event driven", "domain driven design",
    "ddd", "clean architecture", "hexagonal", "cqrs", "event sourcing",
    "saga pattern", "circuit breaker", "api first", "rest api", "graphql api",
    "websockets", "grpc"
];

// Common abbreviations and their full forms
pub const TECHNOLOGY_ABBREVIATIONS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("k8s", "kubernetes"),
    ("tf", "terraform"),
    ("pg", "postgresql"),
    ("mongo", "mongodb"),
    ("es", "elasticsearch"),
    ("ml", "machine learning"),
    ("ai", "artificial intelligence"),
    ("api", "rest api"),
    ("db", "database"),
    ("ui", "user interface"),
    ("ux", "user experience"),
];

pub fn get_all_technologies() -> Vec<Technology> {
    let mut technologies = Vec::new();
    
    for &tech in FRONTEND_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "frontend".to_string(),
        });
    }
    
    for &tech in BACKEND_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "backend".to_string(),
        });
    }
    
    for &tech in PROGRAMMING_LANGUAGES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "language".to_string(),
        });
    }
    
    for &tech in DATABASES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "database".to_string(),
        });
    }
    
    for &tech in CLOUD_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "cloud".to_string(),
        });
    }
    
    for &tech in DEVOPS_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "devops".to_string(),
        });
    }
    
    for &tech in MOBILE_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "mobile".to_string(),
        });
    }
    
    for &tech in TESTING_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "testing".to_string(),
        });
    }
    
    for &tech in DATA_TECHNOLOGIES {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "data".to_string(),
        });
    }
    
    for &tech in ARCHITECTURE_PATTERNS {
        technologies.push(Technology {
            name: tech.to_string(),
            category: "architecture".to_string(),
        });
    }
    
    technologies
} 
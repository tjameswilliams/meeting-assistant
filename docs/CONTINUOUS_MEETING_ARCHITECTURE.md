# Continuous Meeting Assistant Architecture

## Overview

Transform the meeting assistant from an on-demand tool to a continuous meeting recorder that automatically transcribes, diarizes, vectorizes, and stores all audio in real-time.

## Core Components

### 1. Continuous Audio Pipeline

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Audio Input   │───▶│   Audio Buffer   │───▶│   Chunking      │
│   (Microphone)  │    │   (Rolling)      │    │   (2-5 seconds) │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                                                         ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Database      │◀───│   Vectorization  │◀───│  Transcription  │
│   Storage       │    │   (OpenAI API)   │    │   (Whisper)     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                                                         ▼
                                               ┌─────────────────┐
                                               │   Diarization   │
                                               │ (Speaker Split) │
                                               └─────────────────┘
```

### 2. Real-time Processing Pipeline

**Audio Capture Thread**

- Continuous 16kHz audio capture
- Rolling buffer with 30-60 second retention
- Chunk extraction every 2-5 seconds with overlap

**Transcription Thread**

- Async Whisper processing of audio chunks
- Queue-based processing to handle backlog
- Confidence scoring and filtering

**Diarization Thread**

- Speaker change detection
- Voice fingerprinting for speaker identification
- Segment boundary detection

**Vectorization Thread**

- OpenAI embeddings generation
- Batch processing for efficiency
- Fallback queue for API failures

**Database Thread**

- Async SQLite operations
- Transaction batching for performance
- Real-time indexing

### 3. New Core Architecture

```rust
pub struct ContinuousMeetingAssistant {
    // Core processing pipeline
    audio_pipeline: Arc<AudioPipeline>,
    transcription_pipeline: Arc<TranscriptionPipeline>,
    diarization_pipeline: Arc<DiarizationPipeline>,
    vectorization_pipeline: Arc<VectorizationPipeline>,
    storage_pipeline: Arc<StoragePipeline>,

    // State management
    current_meeting: Arc<RwLock<Option<MeetingSession>>>,
    speaker_profiles: Arc<RwLock<SpeakerRegistry>>,
    processing_queue: Arc<ProcessingQueue>,

    // Configuration
    config: MeetingConfig,
    cancellation_token: CancellationToken,
}

pub struct AudioPipeline {
    capture: Arc<RwLock<ContinuousAudioCapture>>,
    buffer: Arc<RwLock<RollingAudioBuffer>>,
    chunk_sender: mpsc::UnboundedSender<AudioChunk>,
}

pub struct TranscriptionPipeline {
    whisper_service: Arc<WhisperService>,
    chunk_receiver: mpsc::UnboundedReceiver<AudioChunk>,
    transcript_sender: mpsc::UnboundedSender<TranscriptSegment>,
    processing_queue: Arc<RwLock<VecDeque<AudioChunk>>>,
}

pub struct DiarizationPipeline {
    speaker_detector: Arc<SpeakerDetector>,
    transcript_receiver: mpsc::UnboundedReceiver<TranscriptSegment>,
    diarized_sender: mpsc::UnboundedSender<DiarizedSegment>,
    speaker_profiles: Arc<RwLock<SpeakerRegistry>>,
}

pub struct VectorizationPipeline {
    embeddings_service: Arc<OpenAIEmbeddingsService>,
    segment_receiver: mpsc::UnboundedReceiver<DiarizedSegment>,
    vectorized_sender: mpsc::UnboundedSender<VectorizedSegment>,
    batch_processor: Arc<EmbeddingBatchProcessor>,
}

pub struct StoragePipeline {
    database: Arc<SqliteMeetingStorage>,
    segment_receiver: mpsc::UnboundedReceiver<VectorizedSegment>,
    batch_writer: Arc<BatchDatabaseWriter>,
}
```

## Data Flow Types

```rust
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub id: Uuid,
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: DateTime<Utc>,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub id: Uuid,
    pub audio_chunk_id: Uuid,
    pub text: String,
    pub confidence: f32,
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct DiarizedSegment {
    pub id: Uuid,
    pub transcript_id: Uuid,
    pub text: String,
    pub speaker_id: String,
    pub speaker_confidence: f32,
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub is_speaker_change: bool,
}

#[derive(Debug, Clone)]
pub struct VectorizedSegment {
    pub id: Uuid,
    pub diarized_id: Uuid,
    pub text: String,
    pub speaker_id: String,
    pub embedding: Vec<f32>,
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub metadata: SegmentMetadata,
}

#[derive(Debug, Clone)]
pub struct SegmentMetadata {
    pub word_count: u32,
    pub sentiment: Option<String>,
    pub key_phrases: Vec<String>,
    pub confidence_scores: ConfidenceScores,
}
```

## Implementation Plan

### Phase 1: Continuous Audio Capture

1. **Replace current audio system** with continuous capture
2. **Implement rolling buffer** for efficient memory usage
3. **Add chunking with overlap** for seamless processing
4. **Background thread management** with proper cleanup

### Phase 2: Real-time Transcription

1. **Async Whisper integration** for continuous processing
2. **Queue-based processing** to handle varying speeds
3. **Confidence filtering** to ignore low-quality audio
4. **Error handling and retries** for robust operation

### Phase 3: Speaker Diarization

1. **Voice fingerprinting** for speaker identification
2. **Speaker change detection** using audio features
3. **Speaker registry** for consistent identification
4. **Manual speaker labeling** interface

### Phase 4: Vectorization Pipeline

1. **Batch embedding generation** for efficiency
2. **Queue management** for API rate limiting
3. **Fallback strategies** for API failures
4. **Cost optimization** with smart batching

### Phase 5: Storage Optimization

1. **Batch database writes** for performance
2. **Real-time indexing** for instant search
3. **Data compression** for storage efficiency
4. **Backup and recovery** mechanisms

## Configuration

```toml
[meeting]
# Audio capture settings
audio_chunk_duration = 3.0  # seconds
audio_overlap = 0.5         # seconds
sample_rate = 16000
channels = 1

# Processing settings
transcription_confidence_threshold = 0.7
speaker_change_threshold = 0.8
embedding_batch_size = 10
database_batch_size = 50

# Performance settings
max_processing_queue_size = 100
transcription_timeout = 30.0  # seconds
embedding_timeout = 10.0     # seconds

# Storage settings
database_path = "~/.meeting-assistant/continuous.db"
audio_retention_hours = 24   # Keep raw audio for 24 hours
backup_interval_minutes = 60
```

## New CLI Interface

```bash
# Start continuous meeting recording
meeting-assistant start

# Stop current meeting (but keep processing backlog)
meeting-assistant stop

# Pause/resume processing
meeting-assistant pause
meeting-assistant resume

# Real-time status
meeting-assistant status

# Speaker management
meeting-assistant speakers list
meeting-assistant speakers identify <speaker-id> "John Smith"
meeting-assistant speakers merge <speaker-id-1> <speaker-id-2>

# Advanced search with real-time data
meeting-assistant search "project timeline" --since "1 hour ago"
meeting-assistant search --speaker "John Smith" --timeframe "today"
meeting-assistant search --semantic "budget concerns" --confidence 0.8

# Analytics and insights
meeting-assistant analytics meeting-length
meeting-assistant analytics speaker-time
meeting-assistant analytics topics --since "1 week"
```

## Performance Considerations

### Memory Management

- **Rolling audio buffers** to prevent memory bloat
- **LRU caching** for speaker profiles and embeddings
- **Batch processing** to reduce memory fragmentation
- **Configurable retention** policies

### CPU Usage

- **Thread pool management** for parallel processing
- **Adaptive quality** based on system load
- **Background/foreground** priority management
- **Resource monitoring** and throttling

### Storage Efficiency

- **Audio compression** for long-term storage
- **Embedding quantization** to reduce size
- **Incremental indexing** for search performance
- **Automatic cleanup** of old data

### Network Usage

- **Batch API calls** to reduce requests
- **Smart retry logic** with exponential backoff
- **Local fallbacks** when APIs are unavailable
- **Cost monitoring** for OpenAI usage

## Privacy & Security

### Data Protection

- **Local-first storage** with no cloud dependencies
- **Encrypted database** option for sensitive meetings
- **Automatic data expiration** for privacy compliance
- **Selective recording** with keyword triggers

### Speaker Privacy

- **Opt-in recording** with clear indicators
- **Speaker anonymization** options
- **Data export/deletion** tools for GDPR compliance
- **Meeting access controls** for multi-user scenarios

This architecture transforms the meeting assistant into a comprehensive, real-time meeting intelligence platform while maintaining the privacy and performance benefits of local processing.

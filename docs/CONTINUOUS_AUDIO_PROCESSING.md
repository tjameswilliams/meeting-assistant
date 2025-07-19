# Continuous Audio Processing for Meeting Storage

## Overview

The SQLite Meeting Storage Plugin now supports **continuous audio processing**, which automatically captures, transcribes, and stores all audio without requiring manual hotkey triggers. This transforms the meeting assistant from an on-demand tool into a continuous meeting recording and analysis system.

## How It Works

### Architecture

1. **Audio Buffering**: The main application continuously buffers audio in the background
2. **Continuous Processing Loop**: A background task extracts audio chunks every 15 seconds
3. **Automatic Transcription**: Each audio chunk is automatically transcribed using available services
4. **Plugin Storage**: The SQLite plugin receives transcription events and stores them in the database

### Key Components

#### 1. Main Application (`src/main.rs`)

- **`start_continuous_audio_processing()`**: Background task that runs every 15 seconds
- **Audio Extraction**: Extracts 10-second audio chunks from the buffer
- **Transcription Pipeline**: Uses the same fallback logic as manual triggers
- **Event Firing**: Sends `AudioCaptured` and `TranscriptionComplete` events to plugins

#### 2. SQLite Plugin (`src/plugins/sqlite_meeting_storage.rs`)

- **Continuous Processing State**: Tracks whether continuous processing is enabled
- **Automatic Initialization**: Enables continuous processing by default
- **Event Handling**: Responds to `TranscriptionComplete` events
- **Database Storage**: Stores utterances with timestamps and speaker information

### Configuration

The continuous processing is automatically enabled when the SQLite meeting storage plugin is active. No additional configuration is required.

#### Default Settings

- **Processing Interval**: 15 seconds (audio chunks are extracted every 15 seconds)
- **Chunk Duration**: 10 seconds per audio chunk
- **Transcription Confidence**: 0.8 (slightly lower than manual triggers to account for continuous processing)

## Usage

### Starting the Application

```bash
./meeting-assistant run
```

When the application starts, you should see:

```
📊 SQLite Meeting Storage Plugin initialized
   Database: /Users/username/.meeting-assistant/meetings.db
   Search: Text-based (semantic search coming in future version)
   Continuous processing: enabled
🔄 Starting continuous audio processing for active plugins
```

### Monitoring Continuous Processing

The application will show periodic status updates:

```
📝 Continuous: This is a sample transcription of ongoing audio...
```

### Database Structure

The continuous processing stores data in the same database tables:

#### Meetings Table

- Automatically creates a new meeting on startup
- Tracks start/end times and participant information

#### Utterances Table

- Stores all transcribed audio chunks
- Includes timestamp, speaker ID, content, and confidence
- Supports semantic search when OpenAI API key is configured

#### AI Responses Table

- Stores any AI responses (when manual interactions occur)
- Separate from continuous transcription data

## Benefits

### 1. Complete Meeting Coverage

- Captures everything said during a meeting
- No need to remember to trigger recording
- Perfect for long meetings or continuous discussions

### 2. Searchable Meeting History

```bash
# Search through all meetings
./meeting-assistant meeting search "action items"

# Get all utterances from a specific speaker
./meeting-assistant meeting speaker user

# View meeting summary
./meeting-assistant meeting summary <meeting-id>
```

### 3. Speaker Identification

```bash
# Name speakers for better organization
./meeting-assistant meeting name speaker_1 "Alice Johnson"
./meeting-assistant meeting name speaker_2 "Bob Smith"
```

### 4. Integration with Manual Features

- Continuous processing runs alongside manual hotkey features
- Manual transcriptions are also stored in the database
- Both automatic and manual interactions are preserved

## Performance Considerations

### Resource Usage

- **CPU**: Minimal overhead for background processing
- **Memory**: Audio buffer maintains ~60 seconds of audio
- **Disk**: Transcriptions are stored efficiently in SQLite
- **Network**: Uses local Whisper when available, OpenAI as fallback

### Audio Quality

- Automatically skips silent periods (energy threshold: 0.001)
- Only processes audio chunks with meaningful content
- Maintains high transcription quality through fallback system

## Privacy and Security

### Local Processing

- Prefers local Whisper installations over cloud services
- Database stored locally in `~/.meeting-assistant/meetings.db`
- No data transmitted unless OpenAI fallback is used

### Data Control

- Complete control over meeting data
- Easy backup and export through SQLite
- Can disable OpenAI fallback in configuration

## Troubleshooting

### Audio Not Being Captured

1. Check audio device configuration in `.env` file
2. Verify FFmpeg is installed and accessible
3. Check system audio permissions

### Transcription Failures

1. Ensure local Whisper is installed (whisper.cpp, faster-whisper, etc.)
2. Verify OpenAI API key if using cloud fallback
3. Check audio quality and microphone settings

### Database Issues

1. Verify write permissions to `~/.meeting-assistant/` directory
2. Check available disk space
3. Inspect database with `sqlite3 ~/.meeting-assistant/meetings.db`

## Testing

Use the provided test script to verify functionality:

```bash
./tests/test_continuous_meeting.sh
```

This script will:

1. Build the application
2. Create a test environment
3. Run the meeting assistant
4. Show database statistics after completion

## Future Enhancements

### Planned Features

- **Speaker Diarization**: Automatic speaker identification
- **Real-time Sentiment Analysis**: Emotion and tone detection
- **Meeting Summaries**: Automatic summary generation
- **Export Formats**: PDF, Word, and text export options
- **API Integration**: REST API for external integrations

### Configuration Options

- Adjustable processing intervals
- Configurable audio chunk sizes
- Custom silence detection thresholds
- Selective plugin activation

## API Reference

### Meeting Commands

```bash
# Start a new meeting with custom title
./meeting-assistant meeting start --name "Team Standup 2024-01-15"

# End current meeting
./meeting-assistant meeting end

# Search meetings
./meeting-assistant meeting search "budget discussion" --limit 5

# List recent meetings
./meeting-assistant meeting list --limit 10
```

### Database Schema

#### Meetings

```sql
CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    title TEXT,
    summary TEXT,
    participants TEXT, -- JSON array
    total_utterances INTEGER DEFAULT 0,
    duration_minutes INTEGER
);
```

#### Utterances

```sql
CREATE TABLE utterances (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    speaker_id TEXT NOT NULL,
    speaker_name TEXT,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    confidence REAL DEFAULT 0.0,
    embedding BLOB, -- For semantic search
    word_count INTEGER DEFAULT 0,
    sentiment TEXT
);
```

## Contributing

This feature is part of the Meeting Assistant's plugin system. To contribute:

1. Fork the repository
2. Create a feature branch
3. Implement your changes
4. Add tests for new functionality
5. Submit a pull request

The continuous processing system is designed to be extensible, allowing for additional plugins that can process the same audio stream for different purposes (sentiment analysis, keyword extraction, etc.).

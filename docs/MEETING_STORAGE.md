# Meeting Storage Plugin

The SQLite Meeting Storage plugin provides persistent storage for meeting transcripts with speaker diarization and advanced search capabilities.

## Features

### ✅ Implemented

- **Automatic Meeting Storage**: Meetings are automatically saved to a local SQLite database
- **Real-time Integration**: Seamlessly captures audio transcripts and AI responses in real-time
- **Speaker Diarization**: Track individual speakers throughout conversations
- **Speaker Naming**: Assign names to speakers for better organization
- **Dual Search Modes**:
  - **Text Search**: Fast keyword-based search through transcripts
  - **Semantic Search**: Vector-based similarity search using OpenAI embeddings
- **Meeting Management**: Start/end meetings, view summaries, and track participants
- **Event-driven Architecture**: Responds to plugin events for live data capture

### 🔄 Vector Embeddings

- **OpenAI Integration**: Uses OpenAI's `text-embedding-3-small` model for generating embeddings
- **Automatic Fallback**: Falls back to text search when OpenAI API key is not available
- **Cosine Similarity**: Uses cosine similarity for semantic matching
- **Efficient Storage**: Embeddings are serialized and stored as binary data in SQLite

## Database Structure

The plugin creates four main tables:

1. **meetings**: Stores meeting metadata

   - `id`: Unique meeting identifier (UUID)
   - `started_at`, `ended_at`: Meeting time boundaries
   - `title`, `summary`: Optional descriptive fields
   - `participants`: JSON array of speaker IDs
   - `total_utterances`: Count of speech segments
   - `duration_minutes`: Meeting length

2. **utterances**: Stores individual speech segments from audio transcription

   - `id`: Unique utterance identifier (UUID)
   - `meeting_id`: Links to parent meeting
   - `speaker_id`: Identifies the speaker
   - `speaker_name`: Optional human-readable name
   - `content`: The actual transcript text
   - `timestamp`: When the utterance occurred
   - `confidence`: Transcription confidence score
   - `embedding`: Vector embedding for semantic search (binary)
   - `word_count`: Number of words in the utterance
   - `sentiment`: Optional sentiment analysis result

3. **ai_responses**: Stores AI/LLM responses separately from human utterances

   - `id`: Unique response identifier (UUID)
   - `meeting_id`: Links to parent meeting
   - `content`: The AI response text
   - `timestamp`: When the response was generated
   - `model`: LLM model used (optional)
   - `prompt_context`: Context that prompted the response (optional)
   - `response_type`: Type of response (e.g., 'llm_response', 'code_analysis')
   - `token_count`: Number of tokens in the response
   - `generation_time_ms`: Time taken to generate response (optional)

4. **speakers**: Tracks speaker metadata per meeting
   - `id`: Speaker identifier
   - `name`: Optional assigned name
   - `meeting_id`: Links to specific meeting
   - `first_appearance`, `last_appearance`: Activity timestamps
   - `total_utterances`, `total_words`: Speaking statistics

## CLI Commands

### Meeting Management

```bash
# Start a new meeting
meeting-assistant meeting start

# End the current meeting
meeting-assistant meeting end

# List recent meetings
meeting-assistant meeting list --limit 10
```

### Speaker Management

```bash
# Name a speaker
meeting-assistant meeting name speaker_1 "Alice Johnson"

# Get all utterances from a specific speaker
meeting-assistant meeting speaker speaker_1
meeting-assistant meeting speaker speaker_1 --meeting-id <uuid>
```

### Search & Retrieval

```bash
# Search through all transcripts
meeting-assistant meeting search "discuss the new feature"
meeting-assistant meeting search "budget planning" --limit 5

# Get meeting summary
meeting-assistant meeting summary <meeting-id>
```

## Configuration

### Environment Variables

```bash
# Required for semantic search (optional for text search)
export OPENAI_API_KEY="your-api-key-here"

# The plugin will automatically detect the API key and enable semantic search
```

### Database Location

- Default: `~/.meeting-assistant/meetings.db`
- Automatically created with proper permissions
- SQLite database with foreign key constraints enabled

## Real-time Integration

The plugin automatically captures meeting data through the event system:

### Plugin Events Handled

- **ApplicationStartup**: Starts a new meeting session
- **ApplicationShutdown**: Ends the current meeting session
- **TranscriptionComplete**: Stores all transcribed utterances from audio capture
- **PromptStreamComplete**: Stores AI responses in separate table
- **ContentAnalyzed**: Stores analyzed content as utterances
- **Custom**: Handles custom meeting management commands

### Automatic Data Flow

1. User speaks → Audio captured → Transcribed → **TranscriptionComplete event fired**
2. Plugin stores utterance in `utterances` table with embedding
3. User interacts with AI → AI responds → **PromptStreamComplete event fired**
4. Plugin stores AI response in `ai_responses` table
5. **All data immediately available for search and analysis**

### Key Improvement

The plugin now correctly captures **ALL audio transcriptions**, not just when the user interacts with the LLM. This means:

- Every spoken word is recorded and stored
- AI responses are stored separately for better organization
- Complete meeting transcripts are preserved
- Search works across all spoken content, not just AI interactions

## Search Capabilities

### Text Search

- Fast keyword matching using SQL LIKE queries
- Available even without OpenAI API key
- Good for exact phrase matching

### Semantic Search (with OpenAI API key)

- Vector similarity using OpenAI embeddings
- Understands context and meaning
- Finds conceptually related content
- Returns results ranked by semantic similarity

## Performance Characteristics

### Search Performance

- **Text Search**: Sub-millisecond for typical queries
- **Semantic Search**: ~100-200ms including API call
- **Database Size**: Minimal overhead (embeddings ~1-3KB per utterance)

### Memory Usage

- Minimal memory footprint
- Embeddings generated on-demand and cached in database
- SQLite handles efficient query optimization

## Usage Examples

### Typical Workflow

1. Start meeting: `meeting-assistant meeting start`
2. Use the main application for audio capture and AI interaction
3. Plugin automatically stores everything in real-time
4. Search later: `meeting-assistant meeting search "budget discussion"`
5. Name speakers: `meeting-assistant meeting name speaker_1 "John Smith"`

### Search Examples

```bash
# Find discussions about technical topics
meeting-assistant meeting search "API design patterns"

# Find emotional moments (with sentiment analysis)
meeting-assistant meeting search "excited about the project"

# Find specific decisions
meeting-assistant meeting search "decided to proceed with option B"
```

## Technical Implementation

### Architecture

- **Event-driven**: Responds to application events in real-time
- **Async-first**: All database operations are non-blocking
- **Error-resilient**: Graceful degradation when API calls fail
- **Type-safe**: Full Rust type safety with proper error handling

### Dependencies

- `sqlx`: Async SQL database operations
- `reqwest`: HTTP client for OpenAI API
- `bincode`: Efficient binary serialization for embeddings
- `serde_json`: JSON handling for API requests/responses

### Future Enhancements

- **Local Embeddings**: Support for offline embedding models
- **Advanced Analytics**: Meeting patterns and insights
- **Export Formats**: PDF, DOCX, and markdown export
- **Integration APIs**: Webhook support for external systems
- **Multi-language Support**: Embeddings for non-English content

## Troubleshooting

### Common Issues

1. **No semantic search available**

   - Solution: Set `OPENAI_API_KEY` environment variable
   - Fallback: Text search will still work

2. **Database permissions error**

   - Solution: Check `~/.meeting-assistant/` directory permissions
   - Creates directory automatically if needed

3. **Embeddings API errors**

   - Check API key validity and rate limits
   - Plugin gracefully falls back to text search

4. **Search returns no results**
   - Check if meeting data has been captured
   - Try text search first: simple keyword queries

### Debug Information

```bash
# Check database contents directly
sqlite3 ~/.meeting-assistant/meetings.db ".schema"
sqlite3 ~/.meeting-assistant/meetings.db "SELECT COUNT(*) FROM meetings;"
sqlite3 ~/.meeting-assistant/meetings.db "SELECT COUNT(*) FROM utterances;"

# Check for embeddings
sqlite3 ~/.meeting-assistant/meetings.db "SELECT COUNT(*) FROM utterances WHERE embedding IS NOT NULL;"
```

The SQLite Meeting Storage plugin represents a comprehensive solution for meeting data persistence, combining the reliability of local SQLite storage with the power of modern AI embeddings for intelligent search and retrieval.

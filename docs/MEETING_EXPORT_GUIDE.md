# Meeting Export and Display Guide

## Overview

The Meeting Assistant now provides comprehensive functionality to reconstruct, display, and export meeting transcripts in multiple formats. This allows you to review past meetings, generate reports, and share meeting summaries with team members.

## Features

### 🎯 **Core Functionality**

- **Meeting Reconstruction**: Combine all utterances and AI responses chronologically
- **Multiple Formats**: Plain text, Markdown, JSON, and detailed reporting
- **Export Options**: Save to files with auto-generated or custom names
- **Timeline Integration**: Merge human speech and AI interactions in proper sequence
- **Speaker Attribution**: Display named speakers or fall back to speaker IDs
- **Statistics**: Word counts, confidence scores, and meeting analytics

### 📊 **Available Formats**

#### 1. **Plain Text** (`plain` or `text`)

Simple, readable format suitable for basic viewing and sharing.

```
MEETING TRANSCRIPT
==================

Title: Weekly Team Standup
Meeting ID: 550e8400-e29b-41d4-a716-446655440000
Started: 2024-01-15 14:00:00 UTC
Ended: 2024-01-15 15:00:00 UTC
Duration: 60 minutes
Participants: Alice Johnson, Bob Smith, Charlie Brown
Total Utterances: 5

TRANSCRIPT
=========

[14:00:00] Alice Johnson: Good morning everyone! Let's start our weekly standup.
[14:05:00] Bob Smith: Hi Alice! I've completed the authentication module this week.
[14:10:00] Charlie Brown: Great work Bob! I'm still working on the database migration scripts.
```

#### 2. **Markdown** (`markdown` or `md`)

Rich format with headers, emphasis, and proper structure for documentation platforms.

```markdown
# Meeting Transcript

**Title:** Weekly Team Standup

**Meeting ID:** `550e8400-e29b-41d4-a716-446655440000`

**Started:** 2024-01-15 14:00:00 UTC

**Duration:** 60 minutes

## Transcript

**[14:00:00] Alice Johnson:** Good morning everyone! Let's start our weekly standup.

**[14:05:00] Bob Smith:** Hi Alice! I've completed the authentication module this week.

**[14:10:00] AI Assistant:**

Based on the discussion, the team is making good progress...

---
```

#### 3. **JSON** (`json`)

Structured data format for programmatic processing and integration with other tools.

```json
{
  "meeting": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "Weekly Team Standup",
    "started_at": "2024-01-15T14:00:00Z",
    "participants": ["Alice Johnson", "Bob Smith", "Charlie Brown"],
    "total_utterances": 5,
    "duration_minutes": 60
  },
  "utterances": [
    {
      "speaker_name": "Alice Johnson",
      "content": "Good morning everyone! Let's start our weekly standup.",
      "timestamp": "2024-01-15T14:00:00Z",
      "confidence": 0.95,
      "word_count": 9
    }
  ],
  "exported_at": "2024-01-15T15:30:00Z"
}
```

#### 4. **Detailed** (`detailed`)

Comprehensive format with statistics, metadata, and detailed information.

```
DETAILED MEETING TRANSCRIPT
============================

Title: Weekly Team Standup
Meeting ID: 550e8400-e29b-41d4-a716-446655440000
Started: 2024-01-15 14:00:00 UTC
Duration: 60 minutes
Total Utterances: 5
AI Responses: 1

STATISTICS
==========
Total Words: 47
Average Confidence: 0.92

SPEAKER STATISTICS
==================
Alice Johnson: 2 utterances, 21 words
Bob Smith: 2 utterances, 25 words
Charlie Brown: 1 utterances, 11 words

DETAILED TRANSCRIPT
===================

[14:00:00] Alice Johnson (confidence: 0.95, words: 9):
  Good morning everyone! Let's start our weekly standup.

[14:05:00] Bob Smith (confidence: 0.92, words: 9):
  Hi Alice! I've completed the authentication module this week.
```

## Command Line Interface

### List Meetings

View all available meetings with basic information:

```bash
./meeting-assistant meeting list --limit 10
```

**Example Output:**

```
Found 3 meetings:

1. Weekly Team Standup (2024-01-15 14:00:00)
   ID: 550e8400-e29b-41d4-a716-446655440000
   Duration: 60 minutes
   Utterances: 5
   Participants: Alice Johnson, Bob Smith, Charlie Brown

2. Project Planning Session (2024-01-14 10:00:00)
   ID: 550e8400-e29b-41d4-a716-446655440001
   Duration: 90 minutes
   Utterances: 12
   Participants: Alice Johnson, David Wilson
```

### Display Meeting

Show a meeting transcript in the terminal:

```bash
# Display in plain text (default)
./meeting-assistant meeting display <meeting-id>

# Display in specific format
./meeting-assistant meeting display <meeting-id> --format markdown
./meeting-assistant meeting display <meeting-id> --format detailed
./meeting-assistant meeting display <meeting-id> --format json
```

### Export Meeting

Save a meeting transcript to a file:

```bash
# Export with auto-generated filename
./meeting-assistant meeting export <meeting-id> --format markdown

# Export to specific file
./meeting-assistant meeting export <meeting-id> --format plain --output "team_standup.txt"

# Export in JSON format for data processing
./meeting-assistant meeting export <meeting-id> --format json --output "meeting_data.json"
```

**Auto-Generated Filenames:**

- Format: `{title}_{timestamp}.{extension}`
- Example: `Weekly_Team_Standup_20240115_140000.md`
- Location: `~/.meeting-assistant/exports/`

## Integration Examples

### Meeting Review Workflow

```bash
# 1. List recent meetings
./meeting-assistant meeting list --limit 5

# 2. Display a specific meeting
./meeting-assistant meeting display 550e8400-e29b-41d4-a716-446655440000 --format detailed

# 3. Export for sharing
./meeting-assistant meeting export 550e8400-e29b-41d4-a716-446655440000 --format markdown --output "standup_summary.md"

# 4. Search for specific topics
./meeting-assistant meeting search "authentication" --limit 5
```

### Team Reporting

```bash
# Export all recent meetings for analysis
for meeting_id in $(./meeting-assistant meeting list --limit 10 | grep "ID:" | cut -d' ' -f4); do
    ./meeting-assistant meeting export $meeting_id --format json --output "reports/meeting_${meeting_id}.json"
done
```

### Documentation Generation

```bash
# Generate markdown documentation for a sprint review
./meeting-assistant meeting export <sprint-review-id> --format markdown --output "sprint_review_notes.md"

# Create detailed analysis report
./meeting-assistant meeting export <meeting-id> --format detailed --output "detailed_analysis.txt"
```

## Advanced Features

### Speaker Management

Name speakers for better readability:

```bash
# Identify speakers by name
./meeting-assistant meeting name user1 "Alice Johnson"
./meeting-assistant meeting name user2 "Bob Smith"

# View speaker-specific utterances
./meeting-assistant meeting speaker user1
```

### Search and Filter

Find specific content across meetings:

```bash
# Search for topics
./meeting-assistant meeting search "budget discussion" --limit 5

# Find action items
./meeting-assistant meeting search "action item" --limit 10

# Look for decisions
./meeting-assistant meeting search "decided" --limit 3
```

### Meeting Management

Control meeting sessions:

```bash
# Start a new meeting with title
./meeting-assistant meeting start --name "Q1 Planning Session"

# End current meeting
./meeting-assistant meeting end

# Get meeting summary
./meeting-assistant meeting summary <meeting-id>
```

## File Organization

### Default Export Locations

```
~/.meeting-assistant/
├── meetings.db           # SQLite database
├── exports/              # Exported files
│   ├── Weekly_Team_Standup_20240115_140000.md
│   ├── Project_Planning_Session_20240114_100000.txt
│   └── Q1_Planning_Session_20240113_090000.json
└── temp/                 # Temporary audio files
```

### Custom Export Paths

```bash
# Export to specific directory
./meeting-assistant meeting export <id> --format markdown --output "/path/to/reports/meeting.md"

# Export to current directory
./meeting-assistant meeting export <id> --format plain --output "./meeting_notes.txt"
```

## Data Processing

### JSON Structure for Integration

The JSON export format provides structured data for integration with other tools:

```json
{
  "meeting": {
    "id": "uuid",
    "title": "string",
    "started_at": "ISO8601 timestamp",
    "ended_at": "ISO8601 timestamp",
    "duration_minutes": number,
    "participants": ["string"],
    "total_utterances": number
  },
  "utterances": [
    {
      "id": "uuid",
      "speaker_id": "string",
      "speaker_name": "string|null",
      "content": "string",
      "timestamp": "ISO8601 timestamp",
      "confidence": number,
      "word_count": number,
      "sentiment": "string|null"
    }
  ],
  "ai_responses": [
    {
      "id": "uuid",
      "content": "string",
      "timestamp": "ISO8601 timestamp",
      "response_type": "string",
      "token_count": number
    }
  ],
  "exported_at": "ISO8601 timestamp"
}
```

## Performance Considerations

### Large Meetings

For meetings with many utterances:

- **Plain text**: Fastest to generate and view
- **Markdown**: Good balance of features and performance
- **Detailed**: Most comprehensive but slower for large meetings
- **JSON**: Efficient for programmatic processing

### Storage Requirements

Typical file sizes:

- **Plain text**: ~1KB per minute of meeting
- **Markdown**: ~1.5KB per minute of meeting
- **JSON**: ~2-3KB per minute of meeting
- **Detailed**: ~2KB per minute of meeting

## Testing

Use the test script to verify functionality:

```bash
./tests/test_meeting_export.sh
```

This script will:

1. Create a test environment
2. Generate sample meeting data
3. Test all export formats
4. Demonstrate CLI commands
5. Show file outputs

## Troubleshooting

### Common Issues

**Meeting not found:**

```bash
# Check if meeting exists
./meeting-assistant meeting list

# Verify meeting ID format (should be UUID)
```

**Export permission errors:**

```bash
# Check directory permissions
ls -la ~/.meeting-assistant/exports/

# Create exports directory if missing
mkdir -p ~/.meeting-assistant/exports/
```

**Database connection errors:**

```bash
# Verify database exists
ls -la ~/.meeting-assistant/meetings.db

# Check database integrity
sqlite3 ~/.meeting-assistant/meetings.db "PRAGMA integrity_check;"
```

### Format-Specific Issues

**Markdown rendering:**

- Ensure proper markdown viewer/editor
- Check for special characters in content

**JSON parsing:**

- Validate JSON structure with `jq` or similar tools
- Check for encoding issues with non-ASCII characters

**File size concerns:**

- Use `--format plain` for large meetings
- Consider splitting large exports by time ranges

## Future Enhancements

### Planned Features

- **PDF Export**: Direct PDF generation with formatting
- **HTML Export**: Rich web-friendly format
- **Excel Export**: Spreadsheet format for analysis
- **Summary Generation**: AI-powered meeting summaries
- **Template Support**: Custom export templates
- **Filtering Options**: Export specific time ranges or speakers
- **Batch Operations**: Process multiple meetings simultaneously

### Integration Possibilities

- **Calendar Integration**: Link exports to calendar events
- **Slack/Teams**: Direct sharing to communication platforms
- **Project Management**: Export to Jira, Trello, etc.
- **Analytics**: Integrate with business intelligence tools
- **Archive Systems**: Automated archival and retention

The meeting export and display system provides a robust foundation for meeting documentation and analysis, enabling teams to maintain comprehensive records of their discussions and decisions.

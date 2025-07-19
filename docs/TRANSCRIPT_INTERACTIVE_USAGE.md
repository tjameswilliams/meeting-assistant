# Transcript Interactive Plugin - Usage Guide

The Transcript Interactive Plugin allows you to analyze existing meeting transcripts using AI with multiple output format options for easy copy-pasting into various applications.

## Features

- **Interactive transcript selection** from your transcripts directory
- **Multiple output formats** for different applications:
  - **Markdown**: Standard format for GitHub, technical docs
  - **HTML**: Rich text format for web editors
  - **Plain Text**: Clean text for universal compatibility
  - **Outlook/Teams Optimized**: Simple formatting perfect for business communications

## Quick Start

1. Run the plugin through the main application
2. Select a transcript from the list
3. Choose your preferred output format
4. Enter your analysis question
5. Copy the formatted response to your target application

## Output Format Options

### 1. Markdown Format

- **Best for**: GitHub, technical documentation, developer tools
- **Features**: Full markdown syntax with headers, bullets, code blocks
- **Example use**: Creating technical summaries, documentation

### 2. HTML Format

- **Best for**: Rich text editors, web applications, content management systems
- **Features**: Clean HTML tags (`<h2>`, `<p>`, `<ul>`, `<strong>`, `<em>`)
- **Example use**: Website content, blog posts, HTML emails

### 3. Plain Text Format

- **Best for**: Universal compatibility, simple text editors, command line
- **Features**: Clean text without any special formatting
- **Example use**: SMS, basic email, simple notes

### 4. Outlook/Teams Optimized Format

- **Best for**: Microsoft Outlook, Microsoft Teams, business communications
- **Features**: Simple formatting that works well in business applications
  - `**bold text**` for emphasis
  - `-` for bullet points
  - Numbered lists with `1.`, `2.`, etc.
  - Clean paragraph breaks
- **Example use**: Meeting summaries, status updates, team communications

## Configuration Options

You can configure the plugin behavior through the plugin system:

```json
{
  "enabled": true,
  "transcripts_dir": "/path/to/transcripts",
  "max_display_count": 20,
  "default_output_format": "OutlookTeams",
  "always_ask_format": true
}
```

### Configuration Fields

- **`enabled`**: Enable/disable the plugin (default: `true`)
- **`transcripts_dir`**: Custom directory for transcripts (default: auto-detected)
- **`max_display_count`**: Maximum transcripts shown in list (default: `20`)
- **`default_output_format`**: Default format when not asking user
  - Options: `"Markdown"`, `"Html"`, `"PlainText"`, `"OutlookTeams"`
  - Default: `"Markdown"`
- **`always_ask_format`**: Always prompt for format selection (default: `true`)
  - If `false`, uses `default_output_format` automatically

## Usage Examples

### Business Meeting Summary for Teams

1. Select format: **Outlook/Teams Optimized**
2. Question: "Create a summary of key decisions and action items"
3. Result: Clean, copy-pasteable text perfect for Teams channels

### Technical Documentation

1. Select format: **Markdown**
2. Question: "Extract technical requirements and create a specification"
3. Result: Properly formatted markdown ready for GitHub or technical docs

### Email Communication

1. Select format: **HTML** or **Plain Text**
2. Question: "Summarize the meeting for stakeholders who weren't present"
3. Result: Professional summary ready for email

## Copy-Paste Tips

### For Outlook/Teams

- The optimized format uses simple formatting that Outlook and Teams recognize
- `**text**` becomes bold when pasted
- Bullet points with `-` work correctly
- Paragraph breaks are preserved

### For Rich Text Editors

- HTML format works well in most rich text editors
- Some applications may require "Paste Special" → "HTML"

### For Universal Compatibility

- Plain text format works everywhere
- No formatting is lost because none is applied
- Perfect for applications with limited formatting support

## Transcript Format Support

The plugin automatically detects and handles multiple transcript formats:

- **Plain Text**: Standard meeting transcript files
- **ElevenLabs JSON**: API response format from ElevenLabs
- **STT Plugin JSON**: Output from Speech-to-Text plugins
- **Unknown**: Generic text files

## Troubleshooting

### No Transcripts Found

- Check that transcripts exist in `~/.meeting-assistant/transcripts/` or your configured directory
- Ensure transcript files have appropriate extensions (`.txt`, `.json`)
- Verify file naming includes "transcript" or has recognized format

### Format Not Working as Expected

- Some applications may require "Paste Special" for HTML format
- Try different format options if one doesn't work in your target application
- Plain Text format is the most universally compatible

### Configuration Issues

- Use the exact format names: `"Markdown"`, `"Html"`, `"PlainText"`, `"OutlookTeams"`
- Check JSON syntax in configuration
- Restart the application after configuration changes

## Integration Examples

### Teams Workflow

```
1. Analyze transcript → Outlook/Teams format
2. Copy result
3. Paste directly into Teams chat or channel
4. Formatting preserved, ready to share
```

### Documentation Workflow

```
1. Analyze transcript → Markdown format
2. Copy result
3. Paste into GitHub issue, wiki, or docs
4. Full markdown formatting intact
```

### Email Workflow

```
1. Analyze transcript → HTML format
2. Copy result
3. Paste into email composer
4. Rich formatting automatically applied
```

The enhanced Transcript Interactive Plugin makes it easy to get AI-powered insights from your meeting transcripts in exactly the format you need for your workflow.

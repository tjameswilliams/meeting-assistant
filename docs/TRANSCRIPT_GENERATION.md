# Transcript Generation Feature 📝

## Overview

The Meeting Assistant CLI now includes an intelligent transcript generation feature that automatically prompts users to generate meeting transcripts when they exit the application (Ctrl+C). This feature leverages the advanced diarization plugin to create speaker-attributed transcripts from the audio buffer.

## How It Works

### Automatic Detection

- When you press Ctrl+C to exit the application, the system automatically checks if the advanced diarization plugin is enabled
- If the plugin is available, you'll see a prompt asking if you want to generate a transcript

### User Interaction

1. **Plugin Detection**: The system shows: `🎯 Advanced Diarization Plugin is enabled`
2. **User Prompt**: You'll be asked: `📝 Would you like to generate a transcript for this meeting? (y/n):`
3. **Choice**: Answer 'y' to generate or 'n' to skip

### Transcript Generation Process

If you choose to generate a transcript:

1. The system extracts the last 60 seconds of audio from the buffer
2. The advanced diarization plugin (Whisper + PyAnnote) processes the audio
3. A formatted transcript is displayed with speaker attribution
4. Summary statistics are shown (total speakers, segments, duration)

## Example Output

```
🎯 Advanced Diarization Plugin is enabled
📝 Would you like to generate a transcript for this meeting? (y/n): y
📝 Generating transcript from meeting audio...

📄 Meeting Transcript:
==================================================
Speaker_0: Welcome everyone to today's meeting. Let's start by reviewing the agenda.
Speaker_1: Thanks for organizing this. I have a few questions about the project timeline.
Speaker_0: Great, let's address those. What specific aspects would you like to discuss?
Speaker_1: I'm particularly concerned about the integration phase and resource allocation.
==================================================

👥 Total speakers: 2
💬 Total segments: 4
⏱️  Total duration: 25.3s
✅ Transcript generated successfully!
```

## Requirements

### Plugin Dependencies

- **Advanced Diarization Plugin**: Must be enabled for the feature to work
- **Whisper + PyAnnote**: The plugin uses OpenAI Whisper for transcription and PyAnnote for speaker diarization
- **Audio Buffer**: Recent audio data must be available in the buffer

### System Dependencies

- **Python 3.7+**: Required for the PyAnnote backend
- **FFmpeg**: For audio processing
- **HuggingFace Token**: Optional, for downloading PyAnnote models

## Configuration

### Plugin Configuration

The advanced diarization plugin can be configured through the plugin system:

```json
{
  "enabled": true,
  "whisper_model_size": "base",
  "whisper_language": "auto",
  "pyannote_model_path": "pyannote/speaker-diarization-3.1",
  "max_speakers": 10,
  "speaker_threshold": 0.7
}
```

### Audio Buffer Settings

Ensure your audio buffer is configured to capture sufficient data:

```env
BUFFER_DURATION=8
CAPTURE_DURATION=15
```

## Troubleshooting

### No Transcript Prompt

If you don't see the transcript prompt when exiting:

1. Check if the advanced diarization plugin is enabled
2. Verify plugin registration in the logs
3. Ensure the plugin loaded successfully during startup

### Force Exit Too Early

If you see "🚫 Force exiting..." before you can respond:

- **This has been fixed!** The timeout is now 30 seconds instead of 2
- This gives plenty of time for transcript generation and user interaction
- You should now have sufficient time to respond to the prompt and wait for processing

### No Audio Data Available

If you see "No audio data available for transcript generation":

1. Check if audio buffering is working
2. Verify your audio device configuration
3. Ensure sufficient audio has been captured during the session
4. **Improved handling**: The system now tries multiple fallback approaches:
   - First attempts 60 seconds of audio
   - Falls back to 30 seconds if no data found
   - Provides clear feedback about what it's trying

### Audio File Issues

If you see "Audio file is empty" or "Cannot access audio file":

1. **New validation**: The system now validates audio files before processing
2. Check that the audio capture system is working properly
3. Verify file permissions in the temp directory
4. Look for the audio file size information displayed during processing

### Transcript Generation Fails

If the transcript generation fails:

1. Check the Python dependencies (see `scripts/whisper_pyannote_helper.py`)
2. Verify FFmpeg is installed and accessible
3. Check if HuggingFace token is configured (if using PyAnnote models)
4. Review the logs for detailed error messages
5. **Enhanced diagnostics**: The system now provides helpful diagnostic information:
   - Audio file size and location
   - Specific error context and suggestions
   - Troubleshooting hints for common issues

## Technical Implementation

### Architecture

- **Event-Driven**: Uses the plugin system's event architecture
- **Asynchronous**: Non-blocking transcript generation
- **Graceful Fallback**: Continues shutdown if transcript generation fails

### Plugin Integration

- **AudioCaptured Event**: Triggers the diarization plugin
- **PluginHookResult**: Receives transcript data from the plugin
- **Data Format**: JSON format with segments, speakers, and metadata

### User Experience

- **Non-Intrusive**: Only prompts when plugin is available
- **Quick Decision**: Simple y/n prompt
- **Informative**: Shows progress and results
- **Graceful**: Handles errors without blocking shutdown

## Testing

### Manual Testing

Use the provided test script:

```bash
./tests/test_transcript_generation.sh
```

### Test Scenarios

1. **Plugin Enabled**: Should show transcript prompt
2. **Plugin Disabled**: Should skip transcript generation
3. **No Audio Data**: Should show appropriate warning
4. **Generation Success**: Should display formatted transcript
5. **Generation Failure**: Should show error and continue shutdown

## Future Enhancements

### Planned Features

- **Save to File**: Option to save transcript to a file
- **Export Formats**: Support for different output formats (JSON, TXT, PDF)
- **Meeting Summary**: AI-generated summary of key points
- **Speaker Names**: Ability to assign names to speakers
- **Timestamp Precision**: More detailed timestamp information

### Integration Opportunities

- **Meeting Storage**: Automatic saving to meeting database
- **Cloud Export**: Upload to cloud storage services
- **Email Integration**: Send transcript via email
- **Calendar Integration**: Attach transcript to meeting events

## Best Practices

### For Users

1. **Let Audio Buffer**: Allow a few seconds of audio to be captured before exiting
2. **Quiet Environment**: Better results with clear audio
3. **Multiple Speakers**: Works best with 2-10 speakers
4. **Regular Exits**: Use Ctrl+C rather than force-killing the application

### For Developers

1. **Error Handling**: Always handle plugin failures gracefully
2. **User Feedback**: Provide clear status messages
3. **Performance**: Keep transcript generation time reasonable
4. **Resource Cleanup**: Ensure proper cleanup of temporary files

## API Reference

### New Methods

- `ask_yes_no(question: &str) -> Result<bool>`: Prompts user for y/n input
- `is_advanced_diarization_enabled() -> bool`: Checks if plugin is available
- `generate_transcript() -> Result<()>`: Generates and displays transcript

### Plugin Events

- **AudioCaptured**: Triggered with audio file path
- **PluginHookResult::Replace**: Returns transcript data

### Data Structures

```rust
// Transcript segment format
{
    "segments": [
        {
            "start_time": 0.0,
            "end_time": 5.2,
            "speaker_id": "Speaker_0",
            "text": "Welcome everyone to today's meeting.",
            "confidence": 0.95
        }
    ],
    "total_speakers": 2,
    "total_segments": 4,
    "total_duration": 25.3
}
```

This feature enhances the Meeting Assistant CLI by providing valuable meeting documentation capabilities while maintaining the application's focus on user experience and technical excellence.

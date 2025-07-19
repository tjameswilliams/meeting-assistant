# Meeting Assistant - Interactive Transcript Generator

A user-friendly command-line tool for generating transcripts from meeting audio files with multiple diarization provider options.

## 🎯 Features

- **📁 Auto-discovery**: Automatically finds audio files from common meeting locations
- **🔧 Multiple Providers**: Support for ElevenLabs, Whisper+PyAnnote, Local Whisper, and OpenAI
- **👥 Speaker Identification**: Advanced speaker diarization with multiple quality levels
- **📄 Multiple Formats**: Generates both human-readable and JSON transcript formats
- **⚡ Easy Setup**: Interactive provider selection with real-time availability checking
- **💾 Organized Output**: Saves transcripts to organized folders with timestamps

## 🚀 Quick Start

### 1. Run the Interactive Script

```bash
./transcript_generator.sh
```

### 2. Follow the Interactive Prompts

The script will:

1. **Show available audio files** from common locations
2. **Let you select** which meeting to transcribe
3. **Display available providers** with their setup status
4. **Generate the transcript** using your chosen provider
5. **Save results** to the `transcripts/` folder

### 3. Test Setup (Optional)

```bash
./tests/test_transcript_generator.sh
```

This will create sample audio files and test the setup.

## 📂 Audio File Discovery

The script automatically searches for audio files in these locations:

- `~/.meeting-assistant/meetings/`
- `~/.meeting-assistant/temp/`
- `./meetings/`
- `./temp/`
- `./recordings/`
- `~/Downloads/`

**Supported formats:** wav, mp3, m4a, flac, aac, ogg

## 🔧 Provider Options

### 1. ElevenLabs Scribe v1 (Recommended)

- **Quality:** ⭐⭐⭐⭐⭐ (Highest)
- **Setup:** Requires `ELEVENLABS_API_KEY` environment variable
- **Features:** Up to 32 speakers, 99 languages, audio events
- **Speed:** Fast (cloud-based)

### 2. Whisper + PyAnnote (Full Local)

- **Quality:** ⭐⭐⭐⭐ (High)
- **Setup:** Requires Python with `whisper` and `pyannote.audio`
- **Features:** Full local processing, good speaker separation
- **Speed:** Slower (local processing)

### 3. Whisper + Smart Detection (Light Local)

- **Quality:** ⭐⭐⭐ (Good)
- **Setup:** Requires Python with `whisper` only
- **Features:** Intelligent speaker detection without PyAnnote
- **Speed:** Medium (local processing)

### 4. Local Whisper

- **Quality:** ⭐⭐⭐ (Good)
- **Setup:** Requires `whisper` command or `whisper-cpp`
- **Features:** Fast local transcription, basic speaker detection
- **Speed:** Fast (local processing)

### 5. OpenAI Whisper API

- **Quality:** ⭐⭐⭐ (Good)
- **Setup:** Requires `OPENAI_API_KEY` environment variable
- **Features:** Cloud-based transcription, no speaker identification
- **Speed:** Fast (cloud-based)

## ⚙️ Configuration

### Environment Variables

Set these in your `.env` file or environment:

```bash
# For ElevenLabs (recommended)
ELEVENLABS_API_KEY=your_elevenlabs_api_key_here

# For OpenAI Whisper API
OPENAI_API_KEY=your_openai_api_key_here

# For PyAnnote (if using local processing)
HUGGINGFACE_HUB_TOKEN=your_huggingface_token_here
```

### Python Dependencies

For local processing options:

```bash
# Basic Whisper support
pip install openai-whisper

# Full PyAnnote support (optional)
pip install pyannote.audio

# For JSON processing
pip install json
```

## 📄 Output Formats

### Human-Readable Transcript

```
Meeting Transcript
=================

Provider: ElevenLabs Scribe v1
Generated: 2024-01-15 14:30:22 UTC
Duration: 15.5 minutes
Speakers: Speaker_1, Speaker_2, Speaker_3

[00:00] Speaker_1: Welcome everyone to today's meeting.
[00:15] Speaker_2: Thank you. I'd like to start with the quarterly results.
[02:30] Speaker_3: The numbers look very promising this quarter.
```

### JSON Format

```json
{
  "provider": "elevenlabs",
  "segments": [
    {
      "start_time": 0.0,
      "end_time": 15.2,
      "speaker_id": "Speaker_1",
      "text": "Welcome everyone to today's meeting.",
      "confidence": 0.95
    }
  ],
  "speakers": ["Speaker_1", "Speaker_2", "Speaker_3"],
  "total_duration": 930.5,
  "generated_at": "2024-01-15T14:30:22Z"
}
```

## 🔍 Example Usage

### Basic Usage

```bash
# Run the interactive script
./transcript_generator.sh

# The script will show you something like:
# 📁 Found 3 recent meeting audio files:
#
# 1. team_standup_2024-01-15.wav
#    📅 Modified: 2024-01-15 09:30:15 UTC
#    📊 Size: 25.3 MB, Duration: 15:30
#    📍 Path: /Users/you/.meeting-assistant/meetings/team_standup_2024-01-15.wav
#
# Select audio file to transcribe (1-3): 1
#
# 🔧 Available diarization providers:
#
# 1. elevenlabs ✅ Ready
#    📝 Cloud-based, highest quality diarization with up to 32 speakers
#
# 2. whisper_only ✅ Ready
#    📝 Local processing with OpenAI Whisper and intelligent speaker detection
#
# Select provider (1-2): 1
```

### Advanced Usage

You can also create your own scripts that use the transcript generator:

```bash
#!/bin/bash
# Custom script to batch process all meetings

for audio_file in ~/.meeting-assistant/meetings/*.wav; do
    if [[ -f "$audio_file" ]]; then
        echo "Processing: $audio_file"

        # Set provider preference
        export SELECTED_PROVIDER="elevenlabs"

        # Process the file
        ./transcript_generator.sh --batch --file "$audio_file"
    fi
done
```

## 🛠️ Troubleshooting

### Common Issues

**No audio files found:**

- Check that you have audio files in the expected locations
- Verify file formats are supported (wav, mp3, m4a, flac, aac, ogg)
- Run `./tests/test_transcript_generator.sh` to create sample files

**Provider not available:**

- ElevenLabs: Check your API key in the `.env` file
- PyAnnote: Install with `pip install pyannote.audio`
- Whisper: Install with `pip install openai-whisper`

**Transcription failed:**

- Check your internet connection for cloud providers
- Verify API keys are valid and have sufficient quota
- Ensure audio files are not corrupted

**Permission errors:**

- Make sure the script is executable: `chmod +x transcript_generator.sh`
- Check write permissions in the transcripts directory

### Debug Mode

Run with debug information:

```bash
bash -x ./transcript_generator.sh
```

### Manual Testing

Test individual components:

```bash
# Test ElevenLabs API
curl -H "xi-api-key: $ELEVENLABS_API_KEY" https://api.elevenlabs.io/v1/user

# Test Whisper
python3 -c "import whisper; print('Whisper OK')"

# Test PyAnnote
python3 -c "import pyannote.audio; print('PyAnnote OK')"
```

## 📁 File Structure

After running the script, you'll have:

```
meeting-assistant/
├── transcript_generator.sh          # Main script
├── tests/
│   └── test_transcript_generator.sh     # Test script
├── transcripts/                     # Generated transcripts
│   ├── meeting_20240115_143022_transcript.txt
│   ├── meeting_20240115_143022_transcript.json
│   └── ...
├── test_audio/                      # Test audio files
│   └── test_meeting.wav
└── recordings/                      # Your meeting recordings
    └── ...
```

## 🔗 Integration

The transcript generator can be integrated into your workflow:

### With Git Hooks

```bash
# .git/hooks/post-receive
#!/bin/bash
if [[ -f "new_meeting.wav" ]]; then
    ./transcript_generator.sh --batch --file "new_meeting.wav"
fi
```

### With Cron Jobs

```bash
# Transcribe any new meetings daily at 6 AM
0 6 * * * cd /path/to/meeting-assistant && ./transcript_generator.sh --batch --all-new
```

### With Other Tools

```bash
# Upload transcripts to cloud storage
./transcript_generator.sh && rsync -av transcripts/ user@server:/backups/transcripts/
```

## 🤝 Contributing

To add new providers or improve the script:

1. Fork the repository
2. Add your provider function to `transcript_generator.sh`
3. Update the provider detection logic
4. Test with various audio files
5. Submit a pull request

## 📄 License

This project is licensed under the Creative Commons Attribution-NonCommercial 4.0 International License. See the main project LICENSE file for details.

## 🆘 Support

For issues or questions:

1. Check this README and troubleshooting section
2. Run the test script to verify setup
3. Check the main project documentation
4. Open an issue on GitHub

---

**Happy transcribing!** 🎙️✨

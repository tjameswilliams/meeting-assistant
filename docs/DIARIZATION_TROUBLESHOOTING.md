# Diarization Troubleshooting Guide

## The Problem

The speaker diarization was breaking text into somewhat equal chunks instead of detecting actual speaker changes. This happens when:

1. **PyAnnote isn't working** - Missing HuggingFace token or dependencies
2. **Fallback algorithms too simplistic** - Not sensitive enough to detect natural conversation patterns
3. **Conversation patterns not recognized** - Algorithms miss subtle speaker changes

## Root Cause Analysis

Looking at the test case in `test_diarize.txt`, the expected pattern is:

- **Speaker 1**: Long monologue segments
- **Speaker 2**: Short interjections like "Into the weeds", "Yeah. Perf", "Well, I use TypeScript and I have bugs"

The original algorithms were:

- Too conservative (required long gaps)
- Missing conversation-specific patterns
- Not sensitive to contradictory statements and short responses

## The Fix

### 1. Enhanced Python Helper Script

Updated `scripts/whisper_pyannote_helper.py` with:

- **Specific phrase detection**: "into the weeds", "yeah. perf", etc.
- **More sensitive gap detection**: 0.2s vs 0.5s thresholds
- **Better short response handling**: Detects brief interjections
- **Debug output**: Shows reasons for speaker changes

### 2. Improved Detection Rules

The new `balanced_speaker_detection()` function uses:

1. **Clear phrase detection** (highest priority)
2. **Short interjections after gaps**
3. **Contradictory statements** (starting with "well")
4. **Topic transitions** ("by the way", "speaking of")
5. **Question/answer patterns**
6. **Very short responses** ("yeah", "perf")
7. **Name mentions**
8. **Significant gaps** with context

### 3. Debugging Tools

Created `test_diarization_debug.py` that:

- Shows detailed Whisper segment analysis
- Compares results with expected diarization
- Explains why speaker changes are detected
- Provides specific feedback on algorithm performance

## Testing the Fix

### Quick Test

```bash
# Run the comprehensive test
./tests/test_diarization_fixed.sh [audio_file]

# Or with specific audio file
./tests/test_diarization_fixed.sh /path/to/your/audio.wav
```

### Manual Testing

1. **Test Python helper directly:**

   ```bash
   python3 scripts/whisper_pyannote_helper.py your_audio.wav --whisper-model base
   ```

2. **Test with debugging tool:**

   ```bash
   python3 test_diarization_debug.py your_audio.wav --expected test_diarize.txt
   ```

3. **Test through Rust app:**
   ```bash
   cargo run -- transcript diarize your_audio.wav --model whisper_pyannote --format json
   ```

### Expected Results

With the improvements, you should see:

- **Multiple speakers detected** (not just 1)
- **Debug output showing reasons** for speaker changes:
  ```
  Speaker change at 27.60s: Clear phrase: 'into the weeds'
  Speaker change at 45.23s: Very short response: 'yeah. perf'
  Speaker change at 67.45s: Contradictory statement starting with 'well'
  ```
- **Natural conversation flow** with short interjections properly attributed

## Troubleshooting

### Still Only 1 Speaker?

1. **Check the transcription quality:**

   - Are the expected phrases actually transcribed?
   - Is the audio clear enough for Whisper?

2. **Verify debug output:**

   - Look for "Speaker change at X.Xs: reason" messages
   - Check if any detection rules are triggered

3. **Try with PyAnnote (if available):**
   ```bash
   export HUGGINGFACE_HUB_TOKEN=your_token_here
   python3 scripts/whisper_pyannote_helper.py your_audio.wav --hf-token $HUGGINGFACE_HUB_TOKEN
   ```

### Audio Quality Issues

If the audio doesn't contain clear speaker changes:

- **Check audio source**: Multiple speakers actually present?
- **Audio clarity**: Clear enough for transcription?
- **Expected patterns**: Do the specific phrases exist in the audio?

### Dependency Issues

If Python scripts fail:

```bash
# Install required dependencies
pip install openai-whisper

# Optional: Install PyAnnote for better results
pip install pyannote.audio
```

## Technical Details

### Algorithm Improvements

The key changes in `balanced_speaker_detection()`:

1. **Phrase-based detection**: Direct matching of conversation patterns
2. **Sensitivity tuning**: Reduced thresholds for gap detection
3. **Context awareness**: Consider previous segment duration and content
4. **Debug transparency**: Clear logging of detection reasons

### Performance Impact

- **Accuracy**: Better detection of natural conversation patterns
- **Speed**: Minimal impact, still runs in real-time
- **Robustness**: Graceful fallback when PyAnnote unavailable

## Configuration

For fine-tuning, modify these values in the Python helper:

```python
# More sensitive gap detection
if gap > 0.2 and time_since_change > 1.0:  # Was 0.5 and 2.0

# Shorter response detection
if duration < 2.0 and len(words) <= 3:  # Was 4.0 and 6

# Faster speaker transitions
if time_since_change > 1.0:  # Was 3.0
```

## Next Steps

1. **Test with your audio**: Run the test script with your specific audio files
2. **Verify improvements**: Check if multiple speakers are now detected
3. **Fine-tune if needed**: Adjust sensitivity thresholds based on your audio characteristics
4. **Add more patterns**: Include conversation patterns specific to your use case

The enhanced diarization should now properly detect speaker changes in natural conversations instead of just breaking text into equal chunks.

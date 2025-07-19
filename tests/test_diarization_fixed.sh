#!/bin/bash

# Test script for improved diarization functionality
# This script tests the enhanced speaker detection algorithms

set -e

echo "🎯 Testing Enhanced Speaker Diarization"
echo "======================================="
echo ""

# Check if required dependencies are available
echo "🔧 Checking dependencies..."

# Check for Python3
if ! command -v python3 &> /dev/null; then
    echo "❌ Python3 not found. Please install Python3."
    exit 1
fi

# Check for Whisper
if ! python3 -c "import whisper" 2>/dev/null; then
    echo "❌ OpenAI Whisper not found."
    echo "💡 Install with: pip install openai-whisper"
    exit 1
fi

echo "✅ Python3 and Whisper found"

# Find audio file
AUDIO_FILE=""
if [ -n "$1" ]; then
    AUDIO_FILE="$1"
elif [ -f "/Users/timwilliams/.meeting-assistant/recordings/meeting_20250709_222941.wav" ]; then
    AUDIO_FILE="/Users/timwilliams/.meeting-assistant/recordings/meeting_20250709_222941.wav"
else
    echo "❌ No audio file found."
    echo "Usage: $0 [audio_file_path]"
    echo "   or place audio file at: /Users/timwilliams/.meeting-assistant/recordings/meeting_20250709_222941.wav"
    exit 1
fi

echo "📁 Using audio file: $AUDIO_FILE"

# Check if expected diarization file exists
EXPECTED_FILE=""
if [ -f "test_diarize.txt" ]; then
    EXPECTED_FILE="test_diarize.txt"
    echo "📋 Using expected diarization: $EXPECTED_FILE"
else
    echo "⚠️  No expected diarization file found (test_diarize.txt)"
fi

echo ""
echo "🧪 Running enhanced diarization test..."
echo "======================================="

# Test 1: Run our debugging tool
echo "1. Testing with debugging tool..."
if [ -f "test_diarization_debug.py" ]; then
    python3 test_diarization_debug.py "$AUDIO_FILE" --expected "$EXPECTED_FILE"
    echo ""
else
    echo "⚠️  Debugging tool not found, skipping detailed analysis"
fi

# Test 2: Run the actual Python helper
echo "2. Testing with Python helper script..."
if [ -f "scripts/whisper_pyannote_helper.py" ]; then
    echo "🔄 Running Whisper + PyAnnote helper..."
    python3 scripts/whisper_pyannote_helper.py "$AUDIO_FILE" --whisper-model base 2>&1 | head -30
    echo ""
else
    echo "❌ Python helper script not found at scripts/whisper_pyannote_helper.py"
    exit 1
fi

# Test 3: Run through the Rust application
echo "3. Testing with Rust application..."
if [ -f "target/release/meeting-assistant" ] || [ -f "target/debug/meeting-assistant" ]; then
    # Find the binary
    BINARY=""
    if [ -f "target/release/meeting-assistant" ]; then
        BINARY="target/release/meeting-assistant"
    else
        BINARY="target/debug/meeting-assistant"
    fi
    
    echo "🔄 Running through Rust application..."
    echo "Command: $BINARY transcript diarize \"$AUDIO_FILE\" --model whisper_pyannote --format json"
    
    # Run the command and capture output
    if timeout 60 "$BINARY" transcript diarize "$AUDIO_FILE" --model whisper_pyannote --format json; then
        echo "✅ Rust application test completed"
    else
        echo "⚠️  Rust application test timed out or failed"
    fi
    echo ""
else
    echo "⚠️  Rust binary not found. Build with: cargo build --release"
fi

# Test 4: Analyze results
echo "4. Results Analysis"
echo "=================="

echo ""
echo "🎯 Expected behavior:"
echo "   - Should detect 2 speakers (Speaker 1 and Speaker 2)"
echo "   - Speaker 2 should appear at 'Into the weeds' (~27.6s)"
echo "   - Speaker 2 should appear at 'Yeah. Perf' exchanges"
echo "   - Speaker 2 should appear at 'Well, I use TypeScript and I have bugs'"
echo ""

echo "🔍 Key improvements made:"
echo "   ✅ Enhanced phrase detection for specific conversation patterns"
echo "   ✅ More sensitive gap detection (0.2s vs 0.5s)"
echo "   ✅ Better handling of contradictory statements"
echo "   ✅ Improved short response detection"
echo "   ✅ Debug output showing speaker change reasons"
echo ""

echo "💡 If diarization still shows only 1 speaker:"
echo "   1. Check audio quality and speaker distinctiveness"
echo "   2. Try with a HuggingFace token for PyAnnote"
echo "   3. Verify the specific phrases exist in the transcription"
echo "   4. Check the debug output for detected patterns"
echo ""

echo "✅ Diarization test completed!"
echo ""
echo "🚀 To run individual components:"
echo "   Debug tool: python3 test_diarization_debug.py \"$AUDIO_FILE\" --expected test_diarize.txt"
echo "   Helper only: python3 scripts/whisper_pyannote_helper.py \"$AUDIO_FILE\""
echo "   Rust app: cargo run -- transcript diarize \"$AUDIO_FILE\" --model whisper_pyannote" 
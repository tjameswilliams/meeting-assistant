#!/bin/bash

# Test script for transcript generation feature
# This script tests the new transcript generation functionality that prompts the user during shutdown

echo "🧪 Testing Transcript Generation Feature"
echo "=========================================="
echo

# Build the application first
echo "🔧 Building Meeting Assistant..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful!"
echo

# Create a test environment
echo "🎯 Setting up test environment..."

# Check if .env file exists
if [ ! -f ../.env ]; then
    echo "⚠️  No ../.env file found. Creating a minimal test configuration..."
    cat > ../.env << EOF
# Test configuration for transcript generation
OPENAI_API_KEY=test_key_for_transcript_generation
OPENAI_MODEL=gpt-4o-mini
AUDIO_DEVICE=:0
BUFFER_DURATION=8
CAPTURE_DURATION=15
EOF
fi

echo "✅ Environment configured"
echo

# Start the application with instructions
echo "📝 Starting Meeting Assistant CLI..."
echo "Instructions:"
echo "1. The application will start and show the welcome screen"
echo "2. Wait a few seconds for audio buffering to initialize"
echo "3. Optionally, speak some words or play audio to generate content for transcription"
echo "4. Press Ctrl+C to initiate shutdown"
echo "5. If the advanced diarization plugin is enabled, you should see:"
echo "   - '🎯 Advanced Diarization Plugin is enabled'"
echo "   - 'This can generate a speaker-attributed transcript from recent audio'"
echo "   - '📝 Would you like to generate a transcript for this meeting? (y/n):'"
echo "6. Answer 'y' to test transcript generation or 'n' to skip"
echo "7. If you answer 'y', the system will:"
echo "   - Extract recent audio from the buffer"
echo "   - Show audio file information"
echo "   - Process the audio through Whisper + PyAnnote"
echo "   - Display a formatted transcript with speaker attribution"
echo
echo "Expected improvements in this version:"
echo "✅ Extended timeout (30 seconds instead of 2) for transcript generation"
echo "✅ Better error handling and user feedback"
echo "✅ Audio file validation before processing"
echo "✅ Fallback to shorter audio duration if needed"
echo "✅ Detailed troubleshooting information on failures"
echo
echo "Press Enter to start the application..."
read

echo "🚀 Starting Meeting Assistant CLI..."
echo "   Use Ctrl+C to test the improved transcript generation feature"
echo

# Run the application
./target/release/meeting-assistant

echo
echo "🎉 Test completed!"
echo
echo "Expected behavior summary:"
echo "✅ On Ctrl+C, you should see a transcript generation prompt if plugin is enabled"
echo "✅ 30-second timeout allows sufficient time for user input and processing"
echo "✅ Audio file validation prevents processing of empty/invalid files"
echo "✅ Clear feedback about what's happening during transcript generation"
echo "✅ Helpful error messages if transcript generation fails"
echo "✅ Application exits gracefully regardless of transcript success/failure"
echo
echo "If you see issues:"
echo "• Check that advanced diarization plugin is properly registered"
echo "• Ensure audio buffer captured some data before shutdown"
echo "• Verify Python/PyAnnote dependencies are installed"
echo "• Review logs for detailed error information" 
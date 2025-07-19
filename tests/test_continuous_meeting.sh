#!/bin/bash

echo "🧪 Testing Continuous Meeting Audio Processing"
echo "=============================================="
echo ""

# Check if the binary exists
if [ ! -f "target/debug/meeting-assistant" ]; then
    echo "❌ Building meeting-assistant binary..."
    cargo build --bin meeting-assistant
    if [ $? -ne 0 ]; then
        echo "❌ Build failed!"
        exit 1
    fi
fi

# Create test environment
echo "📁 Setting up test environment..."
TEST_DIR="test_continuous_audio"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Create a test .env file
cat > ../.env << 'EOF'
OPENAI_API_KEY=your_openai_api_key_here
OPENAI_MODEL=gpt-4o-mini
OPENAI_MAX_TOKENS=1800
AUDIO_DEVICE=:0
BUFFER_DURATION=8
CAPTURE_DURATION=15
MEETING_STORAGE_DB_PATH=test_meetings.db
EOF

echo "📝 Created test configuration"

# Create a simple test audio file (white noise for 5 seconds)
echo "🎵 Creating test audio file..."
if command -v ffmpeg &> /dev/null; then
    ffmpeg -f lavfi -i anullsrc=channel_layout=mono:sample_rate=16000 -t 5 test_audio.wav -y > /dev/null 2>&1
    echo "✅ Created test audio file: test_audio.wav"
else
    echo "⚠️  FFmpeg not found - audio tests may fail"
fi

echo ""
echo "🚀 Starting meeting assistant with continuous processing..."
echo "   The SQLite meeting storage plugin should automatically"
echo "   process all audio without manual hotkey triggers."
echo ""
echo "🔧 To test:"
echo "   1. The app will start with continuous audio processing"
echo "   2. Check the console output for 'Continuous processing: enabled'"
echo "   3. Look for periodic 'Continuous audio processing tick' messages"
echo "   4. Any audio captured will be automatically transcribed and stored"
echo ""
echo "📊 Database will be created at: $(pwd)/test_meetings.db"
echo "🗂️  Check the database after running to see stored utterances"
echo ""
echo "▶️  Starting application... (Press Ctrl+C to stop)"
echo ""

# Run the meeting assistant
../../target/debug/meeting-assistant run

echo ""
echo "🏁 Test completed"
echo ""

# Check if database was created
if [ -f "test_meetings.db" ]; then
    echo "✅ Meeting database created successfully!"
    
    # If sqlite3 is available, show some basic stats
    if command -v sqlite3 &> /dev/null; then
        echo ""
        echo "📊 Database Statistics:"
        echo "   Meetings: $(sqlite3 test_meetings.db 'SELECT COUNT(*) FROM meetings;' 2>/dev/null || echo 'N/A')"
        echo "   Utterances: $(sqlite3 test_meetings.db 'SELECT COUNT(*) FROM utterances;' 2>/dev/null || echo 'N/A')"
        echo "   AI Responses: $(sqlite3 test_meetings.db 'SELECT COUNT(*) FROM ai_responses;' 2>/dev/null || echo 'N/A')"
        echo ""
        echo "🔍 Recent utterances:"
        sqlite3 test_meetings.db "SELECT datetime(timestamp) as time, speaker_id, substr(content, 1, 80) as content FROM utterances ORDER BY timestamp DESC LIMIT 5;" 2>/dev/null || echo "   No utterances found"
    fi
else
    echo "❌ Meeting database was not created"
fi

echo ""
echo "🧹 Cleanup: Remove test directory? (y/N)"
read -r response
if [[ "$response" =~ ^[Yy]$ ]]; then
    cd ..
    rm -rf "$TEST_DIR"
    echo "✅ Test directory cleaned up"
else
    echo "📁 Test directory preserved: $TEST_DIR"
fi 
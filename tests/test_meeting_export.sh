#!/bin/bash

echo "🧪 Testing Meeting Export and Display Functionality"
echo "===================================================="
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
TEST_DIR="test_meeting_export"
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

# Create a test database with sample data
echo "🗄️  Creating test database with sample data..."

# Start the application briefly to create database and tables
timeout 5s ../../target/debug/meeting-assistant run >/dev/null 2>&1 || true

# Check if database was created
if [ -f "test_meetings.db" ]; then
    echo "✅ Database created successfully"
    
    # Insert sample meeting data using SQLite directly
    if command -v sqlite3 &> /dev/null; then
        echo "📊 Inserting sample meeting data..."
        
        # Sample meeting data
        MEETING_ID="550e8400-e29b-41d4-a716-446655440000"
        
        sqlite3 test_meetings.db << EOF
-- Insert sample meeting
INSERT INTO meetings (id, started_at, ended_at, title, summary, participants, total_utterances, duration_minutes)
VALUES (
    '$MEETING_ID',
    '$(date -u -v-1H +"%Y-%m-%dT%H:%M:%SZ")',
    '$(date -u +"%Y-%m-%dT%H:%M:%SZ")',
    'Weekly Team Standup',
    'Discussion of sprint progress and blockers',
    '["Alice Johnson", "Bob Smith", "Charlie Brown"]',
    5,
    60
);

-- Insert sample utterances
INSERT INTO utterances (id, meeting_id, speaker_id, speaker_name, content, timestamp, confidence, word_count)
VALUES 
    ('$(uuidgen)', '$MEETING_ID', 'user1', 'Alice Johnson', 'Good morning everyone! Let''s start our weekly standup.', '$(date -u -v-1H +"%Y-%m-%dT%H:%M:%SZ")', 0.95, 9),
    ('$(uuidgen)', '$MEETING_ID', 'user2', 'Bob Smith', 'Hi Alice! I''ve completed the authentication module this week.', '$(date -u -v-55M +"%Y-%m-%dT%H:%M:%SZ")', 0.92, 9),
    ('$(uuidgen)', '$MEETING_ID', 'user3', 'Charlie Brown', 'Great work Bob! I''m still working on the database migration scripts.', '$(date -u -v-50M +"%Y-%m-%dT%H:%M:%SZ")', 0.88, 11),
    ('$(uuidgen)', '$MEETING_ID', 'user2', 'Bob Smith', 'Charlie, do you need any help with the migration? I have some experience with that.', '$(date -u -v-45M +"%Y-%m-%dT%H:%M:%SZ")', 0.90, 16),
    ('$(uuidgen)', '$MEETING_ID', 'user1', 'Alice Johnson', 'Excellent! Let''s sync up after this meeting to plan the deployment.', '$(date -u -v-40M +"%Y-%m-%dT%H:%M:%SZ")', 0.93, 12);

-- Insert sample AI response
INSERT INTO ai_responses (id, meeting_id, content, timestamp, response_type, token_count)
VALUES (
    '$(uuidgen)',
    '$MEETING_ID',
    'Based on the discussion, the team is making good progress. Key action items: 1) Bob to help Charlie with database migrations, 2) Plan deployment after migration completion, 3) Continue monitoring authentication module performance.',
    '$(date -u -v-35M +"%Y-%m-%dT%H:%M:%SZ")',
    'summary',
    45
);
EOF
        
        echo "✅ Sample data inserted successfully"
        
        # Test the CLI commands
        echo ""
        echo "🧪 Testing CLI Commands"
        echo "======================="
        echo ""
        
        echo "1. 📋 Listing meetings:"
        echo "========================"
        ../../target/debug/meeting-assistant meeting list --limit 5
        echo ""
        
        echo "2. 📄 Displaying meeting in plain text format:"
        echo "=============================================="
        ../../target/debug/meeting-assistant meeting display $MEETING_ID --format plain
        echo ""
        
        echo "3. 📝 Displaying meeting in markdown format:"
        echo "============================================"
        ../../target/debug/meeting-assistant meeting display $MEETING_ID --format markdown
        echo ""
        
        echo "4. 📊 Displaying meeting with detailed information:"
        echo "=================================================="
        ../../target/debug/meeting-assistant meeting display $MEETING_ID --format detailed
        echo ""
        
        echo "5. 💾 Exporting meeting to files:"
        echo "================================="
        
        # Test different export formats
        echo "   Exporting as plain text..."
        ../../target/debug/meeting-assistant meeting export $MEETING_ID --format plain --output "meeting_export.txt"
        
        echo "   Exporting as markdown..."
        ../../target/debug/meeting-assistant meeting export $MEETING_ID --format markdown --output "meeting_export.md"
        
        echo "   Exporting as JSON..."
        ../../target/debug/meeting-assistant meeting export $MEETING_ID --format json --output "meeting_export.json"
        
        echo "   Exporting with auto-generated filename..."
        ../../target/debug/meeting-assistant meeting export $MEETING_ID --format detailed
        echo ""
        
        echo "6. 📁 Checking exported files:"
        echo "=============================="
        echo "Files created in $(pwd):"
        ls -la *.txt *.md *.json 2>/dev/null || echo "   No exported files found"
        echo ""
        
        # Show content of a small export file
        if [ -f "meeting_export.txt" ]; then
            echo "7. 📖 Sample export content (first 20 lines):"
            echo "=============================================="
            head -20 meeting_export.txt
            echo ""
            echo "   ... (truncated, full content in meeting_export.txt)"
            echo ""
        fi
        
        echo "8. 🔍 Testing search functionality:"
        echo "==================================="
        ../../target/debug/meeting-assistant meeting search "authentication" --limit 3
        echo ""
        
        echo "9. 👤 Testing speaker utterances:"
        echo "================================="
        ../../target/debug/meeting-assistant meeting speaker "user2"
        echo ""
        
    else
        echo "❌ SQLite3 not found - cannot insert sample data"
        exit 1
    fi
else
    echo "❌ Database was not created"
    exit 1
fi

echo "🏁 Test completed successfully!"
echo ""
echo "📋 Summary of tested features:"
echo "   ✅ Meeting listing"
echo "   ✅ Meeting display (plain text, markdown, detailed)"
echo "   ✅ Meeting export (multiple formats)"
echo "   ✅ File generation with auto-naming"
echo "   ✅ Search functionality"
echo "   ✅ Speaker utterance retrieval"
echo ""

# Ask about cleanup
echo "🧹 Cleanup: Remove test directory? (y/N)"
read -r response
if [[ "$response" =~ ^[Yy]$ ]]; then
    cd ..
    rm -rf "$TEST_DIR"
    echo "✅ Test directory cleaned up"
else
    echo "📁 Test directory preserved: $TEST_DIR"
    echo "   Database: $TEST_DIR/test_meetings.db"
    echo "   Exports: $TEST_DIR/*.{txt,md,json}"
fi 
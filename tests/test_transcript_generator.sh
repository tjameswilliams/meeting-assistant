#!/bin/bash

# Test script for the Meeting Assistant Interactive Transcript Generator

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${CYAN}${BOLD}🧪 Testing Meeting Assistant Transcript Generator${NC}"
echo -e "${CYAN}===============================================${NC}"
echo

# Check if we have any audio files to test with
echo -e "${BLUE}📁 Checking for test audio files...${NC}"

# Common locations to check
test_locations=(
    "$HOME/.meeting-assistant/meetings"
    "$HOME/.meeting-assistant/temp" 
    "./meetings"
    "./temp"
    "./recordings"
    "./test_audio"
)

# Create test directories if they don't exist
mkdir -p test_audio
mkdir -p recordings

found_files=()
for location in "${test_locations[@]}"; do
    if [[ -d "$location" ]]; then
        for ext in wav mp3 m4a flac aac ogg; do
            while IFS= read -r -d '' file; do
                found_files+=("$file")
            done < <(find "$location" -name "*.$ext" -type f -print0 2>/dev/null)
        done
    fi
done

if [[ ${#found_files[@]} -eq 0 ]]; then
    echo -e "${YELLOW}⚠️  No audio files found. Creating a sample audio file for testing...${NC}"
    echo
    
    # Create a simple test audio file using text-to-speech if available
    if command -v say >/dev/null 2>&1; then
        echo -e "${BLUE}🎵 Creating test audio with macOS text-to-speech...${NC}"
        say "Hello, this is a test meeting recording. Speaker one is talking about the quarterly results. Now speaker two will respond about the marketing strategy. Thank you for listening to this test audio." -o test_audio/test_meeting.aiff
        
        # Convert to wav if ffmpeg is available
        if command -v ffmpeg >/dev/null 2>&1; then
            ffmpeg -i test_audio/test_meeting.aiff -y test_audio/test_meeting.wav 2>/dev/null
            rm test_audio/test_meeting.aiff
            echo -e "${GREEN}✅ Created test audio file: test_audio/test_meeting.wav${NC}"
        else
            echo -e "${GREEN}✅ Created test audio file: test_audio/test_meeting.aiff${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️  No text-to-speech available. Please add audio files to test with.${NC}"
        echo
        echo "You can add audio files to any of these locations:"
        for location in "${test_locations[@]}"; do
            echo "  • $location"
        done
        echo
        echo "Supported formats: wav, mp3, m4a, flac, aac, ogg"
        exit 1
    fi
else
    echo -e "${GREEN}✅ Found ${#found_files[@]} audio files for testing${NC}"
fi

echo

# Test the script with environment variables
echo -e "${BLUE}🔧 Testing provider detection...${NC}"

# Test with different environment configurations
echo -e "${CYAN}Testing with no API keys:${NC}"
unset ELEVENLABS_API_KEY
unset OPENAI_API_KEY
./transcript_generator.sh --check-providers 2>/dev/null || echo "Provider check completed"

echo
echo -e "${CYAN}Testing with mock API keys:${NC}"
export ELEVENLABS_API_KEY="test_key_123"
export OPENAI_API_KEY="test_key_456"
./transcript_generator.sh --check-providers 2>/dev/null || echo "Provider check completed"

echo
echo -e "${BLUE}🚀 Ready to test transcript generation!${NC}"
echo
echo "To run the interactive transcript generator:"
echo -e "${BOLD}./transcript_generator.sh${NC}"
echo
echo "The script will:"
echo "1. 📁 Show available audio files"
echo "2. 🔧 Let you select a diarization provider"
echo "3. 📄 Generate a transcript"
echo "4. 💾 Save the results to the 'transcripts' folder"
echo
echo -e "${GREEN}✅ Test setup complete!${NC}" 
#!/bin/bash

# Meeting Assistant CLI - Cleanup Utility
# Safely cleans up background processes and temporary files

set -e

echo "🧹 Meeting Assistant Cleanup Utility"
echo "===================================="
echo

# Function to safely kill processes
safe_kill() {
    local pid=$1
    if kill -0 "$pid" 2>/dev/null; then
        echo "   Stopping process $pid..."
        kill "$pid" 2>/dev/null
        
        # Wait a moment for graceful shutdown
        sleep 1
        
        # Force kill if still running
        if kill -0 "$pid" 2>/dev/null; then
            echo "   Force killing process $pid..."
            kill -9 "$pid" 2>/dev/null
        fi
    fi
}

# Stop FFmpeg processes
echo "🔊 Stopping FFmpeg processes..."
FFMPEG_PIDS=$(pgrep -f "ffmpeg.*avfoundation" 2>/dev/null || true)

if [ -z "$FFMPEG_PIDS" ]; then
    echo "✅ No FFmpeg processes found"
else
    echo "🛑 Found FFmpeg processes: $FFMPEG_PIDS"
    
    for pid in $FFMPEG_PIDS; do
        safe_kill "$pid"
    done
    
    echo "✅ FFmpeg processes stopped"
fi

# Stop meeting-assistant processes
echo "🤝 Stopping meeting-assistant processes..."
ASSISTANT_PIDS=$(pgrep -f "meeting-assistant" 2>/dev/null || true)

if [ -z "$ASSISTANT_PIDS" ]; then
    echo "✅ No meeting-assistant processes found"
else
    echo "🛑 Found meeting-assistant processes: $ASSISTANT_PIDS"
    
    for pid in $ASSISTANT_PIDS; do
        echo "   Killing process $pid..."
        kill $pid 2>/dev/null
        
        # Wait a moment, then force kill if still running
        sleep 1
        if kill -0 $pid 2>/dev/null; then
            echo "   Force killing process $pid..."
            kill -9 $pid 2>/dev/null
        fi
    done
    
    echo "✅ Meeting-assistant processes stopped"
fi

# Clean up temporary files
echo "🧹 Cleaning up temporary files..."

TEMP_DIR="$HOME/.meeting-assistant/temp"
if [ -d "$TEMP_DIR" ]; then
    # Count files before cleanup
    FILE_COUNT=$(find "$TEMP_DIR" -name "*.wav" -o -name "*.txt" -o -name "*.png" | wc -l)
    
    if [ $FILE_COUNT -gt 0 ]; then
        echo "   Found $FILE_COUNT temporary files to clean up"
        
        # Remove audio buffer files
        find "$TEMP_DIR" -name "buffer_*.wav" -mtime +0 -delete 2>/dev/null
        
        # Remove captured audio files older than 1 hour
        find "$TEMP_DIR" -name "captured_*.wav" -mmin +60 -delete 2>/dev/null
        
        # Remove screenshot files older than 1 hour
        find "$TEMP_DIR" -name "screenshot_*.png" -mmin +60 -delete 2>/dev/null
        
        # Remove transcript files
        find "$TEMP_DIR" -name "*.txt" -delete 2>/dev/null
        
        echo "✅ Temporary files cleaned up"
    else
        echo "✅ No temporary files to clean up"
    fi
else
    echo "✅ No temporary directory found"
fi

# Clean up system temp files that might be left behind
echo "🧹 Cleaning up system temp files..."
SYSTEM_TEMP_FILES=$(find /tmp -name "*meeting*" -o -name "*whisper*" -o -name "*captured_*" -o -name "*buffer_*" 2>/dev/null | wc -l)

if [ $SYSTEM_TEMP_FILES -gt 0 ]; then
    echo "   Found $SYSTEM_TEMP_FILES system temp files"
    find /tmp -name "*meeting*" -delete 2>/dev/null
    find /tmp -name "*whisper*" -delete 2>/dev/null
    find /tmp -name "*captured_*" -delete 2>/dev/null
    find /tmp -name "*buffer_*" -delete 2>/dev/null
    echo "✅ System temp files cleaned up"
else
    echo "✅ No system temp files to clean up"
fi

echo ""
echo "🎉 Cleanup complete!"
echo ""
echo "💡 Usage tips:"
echo "   • Run this script if the app crashes or leaves processes running"
echo "   • Safe to run anytime - won't affect other applications"
echo "   • Creates this script as executable: chmod +x cleanup.sh"
echo "" 
#!/bin/bash

# Meeting Assistant CLI - Rust Edition Build Script

set -e

echo "🦀 Building Meeting Assistant CLI - Rust Edition..."
echo

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo is not installed!"
    echo "Install from: https://rustup.rs/"
    exit 1
fi

echo "✅ Rust version: $(rustc --version)"
echo

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo "⚠️  No .env file found. Creating example..."
    cat > .env.example << 'EOF'
# Meeting Assistant CLI - Rust Edition Configuration

# Required - OpenAI API Key
OPENAI_API_KEY=your_openai_api_key_here

# Optional - OpenAI Settings
OPENAI_MODEL=gpt-4o-mini
OPENAI_MAX_TOKENS=1800
OPENAI_TEMPERATURE=0.5

# Optional - Audio Settings
AUDIO_DEVICE=":7"  # macOS audio device index
AUDIO_SAMPLE_RATE=16000
AUDIO_CHANNELS=1
BUFFER_DURATION=8  # seconds
CAPTURE_DURATION=15  # seconds

# Optional - Timing Configuration
DOUBLE_TAP_WINDOW_MS=500
DEBOUNCE_MS=50
MAX_RECORDING_TIME=30000

# Optional - Temporary Directory
# TEMP_DIR=/custom/temp/path
EOF
    echo "📝 Created .env.example - copy to .env and configure"
    echo
fi

# Build mode selection
BUILD_MODE=${1:-release}

if [ "$BUILD_MODE" = "debug" ]; then
    echo "🔨 Building in debug mode (faster compilation)..."
    cargo build
    BINARY_PATH="target/debug/meeting-assistant"
else
    echo "🚀 Building in release mode (optimized)..."
    cargo build --release
    BINARY_PATH="target/release/meeting-assistant"
fi

echo
echo "✅ Build complete!"
echo

# Check binary size and display info
if [ -f "$BINARY_PATH" ]; then
    BINARY_SIZE=$(ls -lh "$BINARY_PATH" | awk '{print $5}')
    echo "📁 Binary: $BINARY_PATH ($BINARY_SIZE)"
    echo "🎯 Ready to run: ./$BINARY_PATH"
    echo
    echo "🔧 Available commands:"
    echo "   ./$BINARY_PATH          # Run the assistant"
    echo "   ./$BINARY_PATH status   # Check system status"
    echo "   ./$BINARY_PATH setup    # Interactive setup"
    echo "   ./$BINARY_PATH --help   # Show help"
else
    echo "❌ Build failed - binary not found"
    exit 1
fi

echo
echo "📚 Next steps:"
echo "1. Copy .env.example to .env and configure"
echo "2. Run setup: ./$BINARY_PATH setup"
echo "3. Start the assistant: ./$BINARY_PATH" 
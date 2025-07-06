#!/bin/bash
# Meeting Assistant - Start Script

# Check if built
if [[ ! -f "target/release/meeting-assistant" ]]; then
    echo "❌ Application not built. Run: cargo build --release"
    exit 1
fi

# Check configuration
if [[ ! -f ".env" ]]; then
    echo "❌ No .env configuration file found."
    echo "Run setup.sh to create one."
    exit 1
fi

# Start the application
echo "🚀 Starting Meeting Assistant..."
./target/release/meeting-assistant

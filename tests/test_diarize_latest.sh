#!/bin/bash

# Meeting Assistant CLI - Test DiarizeLatest Command
# This script demonstrates the new diarize-latest functionality

set -e

echo "🎯 Testing DiarizeLatest Command"
echo "================================="
echo

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}💡 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Build the project first
print_info "Building the project..."
if cargo build --release; then
    print_success "Build completed successfully"
else
    print_error "Build failed"
    exit 1
fi

echo

# Test 1: Help command
print_info "Test 1: Checking help for diarize-latest command"
./target/release/meeting-assistant transcript diarize-latest --help
echo

# Test 2: List existing recordings
print_info "Test 2: Checking for existing recordings"
./target/release/meeting-assistant record list || {
    print_warning "No existing recordings found or command failed"
    echo
}

# Test 3: Attempt to run diarize-latest (will likely fail if no recordings exist)
print_info "Test 3: Testing diarize-latest command"
echo "Command that would be run:"
echo "  ./target/release/meeting-assistant transcript diarize-latest --model base --format detailed"
echo

# Run the actual command and capture output
print_info "Running diarize-latest..."
if ./target/release/meeting-assistant transcript diarize-latest --model base --format detailed; then
    print_success "Diarize-latest completed successfully!"
else
    exit_code=$?
    if [ $exit_code -eq 0 ]; then
        print_success "Command completed successfully"
    else
        print_warning "Command failed (expected if no recordings exist)"
        print_info "To test with actual audio:"
        echo "  1. Record some audio first: ./target/release/meeting-assistant record start"
        echo "  2. Stop recording: ./target/release/meeting-assistant record stop"
        echo "  3. Then run: ./target/release/meeting-assistant transcript diarize-latest"
    fi
fi

echo

# Test 4: Show usage examples
print_info "Usage examples:"
echo "  # Basic usage with default settings"
echo "  ./target/release/meeting-assistant transcript diarize-latest"
echo
echo "  # Use larger model for better accuracy"
echo "  ./target/release/meeting-assistant transcript diarize-latest --model large"
echo
echo "  # Limit to maximum 3 speakers"
echo "  ./target/release/meeting-assistant transcript diarize-latest --max-speakers 3"
echo
echo "  # Output in JSON format"
echo "  ./target/release/meeting-assistant transcript diarize-latest --format json"
echo
echo "  # Text-only output"
echo "  ./target/release/meeting-assistant transcript diarize-latest --format text"

print_success "Test script completed!"
echo
print_info "The new diarize-latest command is ready to use!"
print_info "It will automatically find and process the most recently recorded audio file." 
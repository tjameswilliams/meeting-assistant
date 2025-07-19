#!/bin/bash

# Meeting Assistant - Interactive Transcript Generator
# This script provides a user-friendly interface for transcribing meeting audios

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Audio extensions to search for
AUDIO_EXTENSIONS="wav mp3 m4a flac aac ogg"

# Print colored output
print_header() {
    echo -e "${CYAN}${BOLD}🎙️  Meeting Assistant - Interactive Transcript Generator${NC}"
    echo -e "${CYAN}========================================================${NC}"
    echo
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

print_info() {
    echo -e "${BLUE}💡 $1${NC}"
}

print_step() {
    echo -e "${PURPLE}🚀 $1${NC}"
}

# Function to check if we're in the right directory
check_directory() {
    if [[ ! -f "Cargo.toml" ]]; then
        print_error "Please run this script from the meeting-assistant project root directory"
        exit 1
    fi
}

# Function to find meeting audio files
find_meeting_audios() {
    local search_path="$HOME/.meeting-assistant/recordings"
    
    if [[ ! -d "$search_path" ]]; then
        return
    fi
    
    # Find all audio files and sort by modification time (newest first)
    local found_files=()
    for ext in $AUDIO_EXTENSIONS; do
        while IFS= read -r -d '' file; do
            found_files+=("$file")
        done < <(find "$search_path" -maxdepth 1 -name "*.$ext" -type f -print0 2>/dev/null)
    done
    
    # Sort by modification time (newest first) and take only the first 5
    if [[ ${#found_files[@]} -gt 0 ]]; then
        printf '%s\n' "${found_files[@]}" | while read -r file; do
            printf '%s\t%s\n' "$(stat -c %Y "$file" 2>/dev/null || stat -f %m "$file" 2>/dev/null || echo 0)" "$file"
        done | sort -nr | head -5 | cut -f2-
    fi
}

# Function to get audio duration
get_audio_duration() {
    local audio_file="$1"
    if command -v ffprobe >/dev/null 2>&1; then
        ffprobe -v quiet -show_entries format=duration -of csv=p=0 "$audio_file" 2>/dev/null | awk '{
            duration = $1
            minutes = int(duration / 60)
            seconds = int(duration % 60)
            printf "%d:%02d", minutes, seconds
        }'
    else
        echo "Unknown"
    fi
}

# Function to get file size in MB
get_file_size_mb() {
    local file="$1"
    if [[ -f "$file" ]]; then
        local size_bytes
        if [[ "$OSTYPE" == "darwin"* ]]; then
            size_bytes=$(stat -f%z "$file")
        else
            size_bytes=$(stat -c%s "$file")
        fi
        echo "scale=1; $size_bytes / 1048576" | bc
    else
        echo "0"
    fi
}

# Function to display and select audio file
select_audio_file() {
    local audio_files=("$@")
    
    if [[ ${#audio_files[@]} -eq 0 ]]; then
        print_warning "No meeting audio files found."
        echo "Looking in common locations:"
        echo "  • ~/.meeting-assistant/meetings/"
        echo "  • ~/.meeting-assistant/temp/"
        echo "  • ./meetings/"
        echo "  • ./temp/"
        echo "  • ./recordings/"
        echo "  • ~/Downloads/"
        echo
        echo "Make sure you have recorded some meetings first!"
        exit 1
    fi
    
    print_step "Found ${#audio_files[@]} recent meeting audio files:"
    echo
    
    for i in "${!audio_files[@]}"; do
        local file="${audio_files[$i]}"
        local basename=$(basename "$file")
        local size_mb=$(get_file_size_mb "$file")
        local duration=$(get_audio_duration "$file")
        local modified=$(stat -c %y "$file" 2>/dev/null || stat -f %Sm "$file" 2>/dev/null || echo "Unknown")
        
        echo -e "${GREEN}$((i+1)).${BOLD} $basename${NC}"
        echo -e "   ${BLUE}📅 Modified: $modified${NC}"
        echo -e "   ${BLUE}📊 Size: ${size_mb} MB, Duration: $duration${NC}"
        echo -e "   ${BLUE}📍 Path: $file${NC}"
        echo
    done
    
    while true; do
        echo -ne "${YELLOW}Select audio file to transcribe (1-${#audio_files[@]}): ${NC}"
        read -r choice
        
        if [[ "$choice" =~ ^[0-9]+$ ]] && [[ "$choice" -ge 1 ]] && [[ "$choice" -le "${#audio_files[@]}" ]]; then
            SELECTED_AUDIO="${audio_files[$((choice-1))]}"
            break
        else
            print_error "Invalid choice. Please enter a number between 1 and ${#audio_files[@]}."
        fi
    done
}

# Global arrays for providers
declare -a AVAILABLE_PROVIDERS=()
declare -a PROVIDER_DESCRIPTIONS=()
declare -a PROVIDER_STATUSES=()

# Function to check available providers
check_providers() {
    # Clear existing arrays
    AVAILABLE_PROVIDERS=()
    PROVIDER_DESCRIPTIONS=()
    PROVIDER_STATUSES=()
    
    # Check ElevenLabs
    if [[ -n "${ELEVENLABS_API_KEY:-}" ]]; then
        AVAILABLE_PROVIDERS+=("elevenlabs")
        PROVIDER_DESCRIPTIONS+=("ElevenLabs Scribe v1 - Cloud-based, highest quality diarization")
        PROVIDER_STATUSES+=("✅ Ready")
    else
        AVAILABLE_PROVIDERS+=("elevenlabs")
        PROVIDER_DESCRIPTIONS+=("ElevenLabs Scribe v1 - Cloud-based, highest quality diarization")
        PROVIDER_STATUSES+=("⚠️  Requires API key")
    fi
    
    # Check Whisper + PyAnnote
    if python3 -c "import whisper" 2>/dev/null; then
        if python3 -c "import pyannote.audio" 2>/dev/null; then
            AVAILABLE_PROVIDERS+=("whisper_pyannote")
            PROVIDER_DESCRIPTIONS+=("Whisper + PyAnnote - Local processing, full diarization")
            PROVIDER_STATUSES+=("✅ Ready")
        else
            AVAILABLE_PROVIDERS+=("whisper_only")
            PROVIDER_DESCRIPTIONS+=("Whisper + Smart Detection - Local processing, intelligent speaker detection")
            PROVIDER_STATUSES+=("✅ Ready")
        fi
    fi
    
    # Check if we have any local transcription
    if command -v whisper >/dev/null 2>&1 || command -v whisper-cpp >/dev/null 2>&1; then
        AVAILABLE_PROVIDERS+=("local")
        PROVIDER_DESCRIPTIONS+=("Local Whisper - Fast local transcription")
        PROVIDER_STATUSES+=("✅ Ready")
    fi
    
    # Always add OpenAI option
    if [[ -n "${OPENAI_API_KEY:-}" ]]; then
        AVAILABLE_PROVIDERS+=("openai")
        PROVIDER_DESCRIPTIONS+=("OpenAI Whisper API - Cloud-based transcription")
        PROVIDER_STATUSES+=("✅ Ready")
    else
        AVAILABLE_PROVIDERS+=("openai")
        PROVIDER_DESCRIPTIONS+=("OpenAI Whisper API - Cloud-based transcription")
        PROVIDER_STATUSES+=("⚠️  Requires API key")
    fi
    
    # Debug output
    print_info "Found ${#AVAILABLE_PROVIDERS[@]} providers: ${AVAILABLE_PROVIDERS[*]}"
}

# Function to select provider
select_provider() {
    # Check if we have any providers
    if [[ -z "${AVAILABLE_PROVIDERS:-}" ]] || [[ ${#AVAILABLE_PROVIDERS[@]} -eq 0 ]]; then
        print_error "No providers available. Please check your setup."
        exit 1
    fi
    
    print_step "Available diarization providers:"
    echo
    
    for i in "${!AVAILABLE_PROVIDERS[@]}"; do
        echo -e "${GREEN}$((i+1)).${BOLD} ${AVAILABLE_PROVIDERS[$i]}${NC} ${PROVIDER_STATUSES[$i]}"
        echo -e "   ${BLUE}📝 ${PROVIDER_DESCRIPTIONS[$i]}${NC}"
        echo
    done
    
    while true; do
        echo -ne "${YELLOW}Select provider (1-${#AVAILABLE_PROVIDERS[@]}): ${NC}"
        read -r choice
        
        if [[ "$choice" =~ ^[0-9]+$ ]] && [[ "$choice" -ge 1 ]] && [[ "$choice" -le "${#AVAILABLE_PROVIDERS[@]}" ]]; then
            SELECTED_PROVIDER="${AVAILABLE_PROVIDERS[$((choice-1))]}"
            SELECTED_PROVIDER_DESC="${PROVIDER_DESCRIPTIONS[$((choice-1))]}"
            
            # Check if provider requires setup
            if [[ "${PROVIDER_STATUSES[$((choice-1))]}" == *"Requires"* ]]; then
                print_warning "This provider requires additional setup."
                echo -ne "Do you want to continue anyway? (y/n): "
                read -r confirm
                if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
                    continue
                fi
            fi
            break
        else
            print_error "Invalid choice. Please enter a number between 1 and ${#AVAILABLE_PROVIDERS[@]}."
        fi
    done
}

# Function to generate transcript using the selected provider
generate_transcript() {
    local audio_file="$1"
    local provider="$2"
    
    print_step "Generating transcript..."
    echo -e "${BLUE}📄 Transcribing: $(basename "$audio_file")${NC}"
    echo -e "${BLUE}🔧 Using provider: $provider${NC}"
    echo
    
    # Create transcripts directory in production location
    local transcripts_dir="$HOME/.meeting-assistant/transcripts"
    mkdir -p "$transcripts_dir"
    
    # Generate unique filename
    local audio_name=$(basename "$audio_file" | sed 's/\.[^.]*$//')
    local timestamp=$(date +"%Y%m%d_%H%M%S")
    local output_file="$transcripts_dir/${audio_name}_${timestamp}_transcript.txt"
    local json_file="$transcripts_dir/${audio_name}_${timestamp}_transcript.json"
    
    case "$provider" in
        "elevenlabs")
            generate_elevenlabs_transcript "$audio_file" "$output_file" "$json_file"
            ;;
        "whisper_pyannote")
            generate_whisper_pyannote_transcript "$audio_file" "$output_file" "$json_file"
            ;;
        "whisper_only")
            generate_whisper_only_transcript "$audio_file" "$output_file" "$json_file"
            ;;
        "local")
            generate_local_transcript "$audio_file" "$output_file" "$json_file"
            ;;
        "openai")
            generate_openai_transcript "$audio_file" "$output_file" "$json_file"
            ;;
        *)
            print_error "Unknown provider: $provider"
            exit 1
            ;;
    esac
    
    TRANSCRIPT_FILE="$output_file"
    JSON_FILE="$json_file"
}

# Function to generate ElevenLabs transcript
generate_elevenlabs_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local json_file="$3"
    
    print_info "Sending audio to ElevenLabs API..."
    
    # Check if API key is available
    if [[ -z "${ELEVENLABS_API_KEY:-}" ]]; then
        print_error "ElevenLabs API key not found. Please set ELEVENLABS_API_KEY in .env file."
        generate_fallback_transcript "$audio_file" "$output_file" "ElevenLabs Scribe v1 (API key missing)"
        return 1
    fi
    
    # Direct API call to ElevenLabs
    print_info "Making direct API call to ElevenLabs..."
    
    # Create a temporary file for the API response
    local temp_response=$(mktemp)
    
    # Make the API call
    local api_response=$(curl -s -X POST "https://api.elevenlabs.io/v1/speech-to-text" \
        -H "xi-api-key: ${ELEVENLABS_API_KEY}" \
        -F "file=@${audio_file}" \
        -F "model_id=scribe_v1" \
        -w "%{http_code}" \
        -o "$temp_response")
    
    # Check if the API call was successful
    if [[ "$api_response" =~ ^2[0-9][0-9]$ ]]; then
        print_success "ElevenLabs API call successful!"
        
                 # Parse the response and create transcript
         if python3 -c "
import json
import sys

try:
    with open('$temp_response', 'r') as f:
        data = json.load(f)
    
    # Save raw JSON
    with open('$json_file', 'w') as f:
        json.dump(data, f, indent=2)
    
    # Create human-readable transcript
    with open('$output_file', 'w') as f:
        f.write('Meeting Transcript\\n')
        f.write('=================\\n\\n')
        f.write('Provider: ElevenLabs Scribe v1\\n')
        f.write('Generated: $(date)\\n')
        f.write('Language: {}\\n'.format(data.get('language_code', 'Unknown')))
        f.write('Confidence: {:.1%}\\n\\n'.format(data.get('language_probability', 0)))
        
        # Process words/segments
        words = data.get('words', [])
        if words:
            current_speaker = 'Speaker'
            current_text = []
            current_start = 0
            
            for word in words:
                word_type = word.get('type', 'word')
                speaker = word.get('speaker_id')
                if speaker is None:
                    speaker = 'Speaker'
                text = word.get('text', '')
                start_time = word.get('start', 0)
                
                # Only process word types, skip spacing and events
                if word_type == 'word':
                    if not current_text:
                        current_start = start_time
                    current_text.append(text)
            
            # Write the transcript
            if current_text:
                minutes = int(current_start // 60)
                seconds = int(current_start % 60)
                f.write('[{:02d}:{:02d}] {}: {}\\n'.format(
                    minutes, seconds, current_speaker, ' '.join(current_text)
                ))
        else:
            # Fallback to simple text
            f.write(data.get('text', 'No transcription available'))
            
    print('Transcript generated successfully')
except Exception as e:
    print(f'Error parsing ElevenLabs response: {e}', file=sys.stderr)
    sys.exit(1)
"; then
            print_success "Transcript processing completed successfully"
        else
            print_error "Failed to process ElevenLabs response"
            generate_fallback_transcript "$audio_file" "$output_file" "ElevenLabs Scribe v1 (processing error)"
        fi
    else
        print_error "ElevenLabs API call failed with HTTP code: $api_response"
        
        # Try to show error message from response
        if [[ -f "$temp_response" ]]; then
            local error_msg=$(cat "$temp_response" | head -200)
            print_error "API Error: $error_msg"
        fi
        
        generate_fallback_transcript "$audio_file" "$output_file" "ElevenLabs Scribe v1 (API error)"
    fi
    
    # Clean up
    rm -f "$temp_response"
}

# Function to generate Whisper+PyAnnote transcript
generate_whisper_pyannote_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local json_file="$3"
    
    print_info "Processing with Whisper + PyAnnote..."
    
    if [[ -f "scripts/whisper_pyannote_helper.py" ]]; then
        if python3 scripts/whisper_pyannote_helper.py "$audio_file" --whisper-model base --pyannote-model pyannote/speaker-diarization-3.1 --output-json > "$json_file" 2>/dev/null; then
            # Convert JSON to readable format
            python3 -c "
import json
import sys

try:
    with open('$json_file', 'r') as f:
        data = json.load(f)
    
    with open('$output_file', 'w') as f:
        f.write('Meeting Transcript\\n')
        f.write('=================\\n\\n')
        f.write(f'Provider: Whisper + PyAnnote\\n')
        f.write(f'Generated: $(date)\\n\\n')
        
        if 'segments' in data:
            for segment in data['segments']:
                start_time = segment.get('start_time', 0)
                speaker = segment.get('speaker_id', 'Unknown')
                text = segment.get('text', '')
                
                minutes = int(start_time // 60)
                seconds = int(start_time % 60)
                
                f.write(f'[{minutes:02d}:{seconds:02d}] {speaker}: {text}\\n')
        else:
            f.write('No segments found in the response\\n')
            
    print('Transcript generated successfully')
except Exception as e:
    print(f'Error: {e}', file=sys.stderr)
    sys.exit(1)
"
        else
            print_error "Failed to generate Whisper+PyAnnote transcript"
            generate_fallback_transcript "$audio_file" "$output_file" "Whisper + PyAnnote"
        fi
    else
        print_warning "Python helper script not found."
        generate_fallback_transcript "$audio_file" "$output_file" "Whisper + PyAnnote"
    fi
}

# Function to generate Whisper-only transcript
generate_whisper_only_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local json_file="$3"
    
    print_info "Processing with Whisper + Smart Detection..."
    
    if python3 -c "import whisper" 2>/dev/null; then
        python3 -c "
import whisper
import json
import sys

try:
    model = whisper.load_model('base')
    result = model.transcribe('$audio_file')
    
    # Create simple diarization based on pauses
    segments = []
    current_speaker = 'Speaker 1'
    speaker_count = 1
    
    for segment in result['segments']:
        # Simple speaker change detection based on long pauses
        if segment['start'] > 0 and segments and (segment['start'] - segments[-1]['end_time']) > 2.0:
            speaker_count += 1
            current_speaker = f'Speaker {speaker_count}'
        
        segments.append({
            'start_time': segment['start'],
            'end_time': segment['end'],
            'speaker_id': current_speaker,
            'text': segment['text'].strip(),
            'confidence': 0.8
        })
    
    # Save JSON
    with open('$json_file', 'w') as f:
        json.dump({'segments': segments}, f, indent=2)
    
    # Save readable format
    with open('$output_file', 'w') as f:
        f.write('Meeting Transcript\\n')
        f.write('=================\\n\\n')
        f.write('Provider: Whisper + Smart Detection\\n')
        f.write('Generated: $(date)\\n\\n')
        
        for segment in segments:
            start_time = segment['start_time']
            speaker = segment['speaker_id']
            text = segment['text']
            
            minutes = int(start_time // 60)
            seconds = int(start_time % 60)
            
            f.write(f'[{minutes:02d}:{seconds:02d}] {speaker}: {text}\\n')
    
    print('Transcript generated successfully')
except Exception as e:
    print(f'Error: {e}', file=sys.stderr)
    sys.exit(1)
"
    else
        print_error "Whisper not available"
        generate_fallback_transcript "$audio_file" "$output_file" "Whisper + Smart Detection"
    fi
}

# Function to generate local transcript
generate_local_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local json_file="$3"
    
    print_info "Processing with local Whisper..."
    
    if command -v whisper >/dev/null 2>&1; then
        whisper "$audio_file" --output_dir "$(dirname "$output_file")" --output_format txt --model base
        
        # Find the generated file and rename it
        local generated_file=$(find "$(dirname "$output_file")" -name "*.txt" -newer "$audio_file" | head -1)
        if [[ -f "$generated_file" ]]; then
            mv "$generated_file" "$output_file"
            
            # Add header to the file
            local temp_file=$(mktemp)
            echo "Meeting Transcript" > "$temp_file"
            echo "=================" >> "$temp_file"
            echo "" >> "$temp_file"
            echo "Provider: Local Whisper" >> "$temp_file"
            echo "Generated: $(date)" >> "$temp_file"
            echo "" >> "$temp_file"
            cat "$output_file" >> "$temp_file"
            mv "$temp_file" "$output_file"
        else
            generate_fallback_transcript "$audio_file" "$output_file" "Local Whisper"
        fi
    else
        print_error "Local Whisper not available"
        generate_fallback_transcript "$audio_file" "$output_file" "Local Whisper"
    fi
}

# Function to generate OpenAI transcript
generate_openai_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local json_file="$3"
    
    print_info "Processing with OpenAI Whisper API..."
    
    if [[ -n "${OPENAI_API_KEY:-}" ]]; then
        # Use curl to call OpenAI API
        local response=$(curl -s -X POST "https://api.openai.com/v1/audio/transcriptions" \
            -H "Authorization: Bearer $OPENAI_API_KEY" \
            -H "Content-Type: multipart/form-data" \
            -F "file=@$audio_file" \
            -F "model=whisper-1" \
            -F "response_format=json")
        
        if [[ $? -eq 0 ]] && [[ -n "$response" ]]; then
            # Extract text from response
            local transcript_text=$(echo "$response" | python3 -c "
import json
import sys
try:
    data = json.load(sys.stdin)
    print(data.get('text', ''))
except:
    print('Error parsing response')
")
            
            # Create output file
            cat > "$output_file" << EOF
Meeting Transcript
=================

Provider: OpenAI Whisper API
Generated: $(date)

$transcript_text
EOF
            
            # Create simple JSON
            echo "{\"transcript\": \"$transcript_text\"}" > "$json_file"
        else
            print_error "Failed to call OpenAI API"
            generate_fallback_transcript "$audio_file" "$output_file" "OpenAI Whisper API"
        fi
    else
        print_error "OpenAI API key not configured"
        generate_fallback_transcript "$audio_file" "$output_file" "OpenAI Whisper API"
    fi
}

# Function to generate fallback transcript
generate_fallback_transcript() {
    local audio_file="$1"
    local output_file="$2"
    local provider="$3"
    
    cat > "$output_file" << EOF
Meeting Transcript
=================

Provider: $provider
Generated: $(date)
Source: $(basename "$audio_file")

Error: Unable to process audio file.
This could be due to:
- Missing dependencies
- Invalid API keys
- Network connectivity issues
- Unsupported audio format

Please check your configuration and try again.
EOF
}

# Function to display transcript
display_transcript() {
    local transcript_file="$1"
    
    if [[ -f "$transcript_file" ]]; then
        echo
        print_step "Generated Transcript:"
        echo -e "${CYAN}========================${NC}"
        echo
        
        # Display the transcript with syntax highlighting
        if command -v bat >/dev/null 2>&1; then
            bat --style=plain --color=always "$transcript_file"
        else
            cat "$transcript_file"
        fi
        
        echo
        print_success "Transcript saved to: $transcript_file"
        if [[ -f "$JSON_FILE" ]]; then
            print_success "JSON version saved to: $JSON_FILE"
        fi
    else
        print_error "Transcript file not found: $transcript_file"
    fi
}

# Main execution
main() {
    print_header
    
    # Check dependencies
    check_directory
    
    # Load environment variables FIRST
    if [[ -f ".env" ]]; then
        print_info "Loading environment variables from .env file..."
        source .env
        print_info "Loaded ELEVENLABS_API_KEY: ${ELEVENLABS_API_KEY:+SET}"
        print_info "Loaded OPENAI_API_KEY: ${OPENAI_API_KEY:+SET}"
    else
        print_warning "No .env file found. Some providers may not be available."
    fi
    
    # Find audio files
    audio_files=($(find_meeting_audios))
    
    # Select audio file
    select_audio_file "${audio_files[@]}"
    
    # Check available providers
    check_providers
    
    # Select provider
    select_provider
    
    # Generate transcript
    generate_transcript "$SELECTED_AUDIO" "$SELECTED_PROVIDER"
    
    # Display transcript
    display_transcript "$TRANSCRIPT_FILE"
    
    print_success "Transcript generation complete!"
}

# Check for required dependencies
check_dependencies() {
    local missing_deps=()
    
    if ! command -v bc >/dev/null 2>&1; then
        missing_deps+=("bc")
    fi
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        print_info "Please install them and try again."
        exit 1
    fi
}

# Run main function
check_dependencies
main "$@" 
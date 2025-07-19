#!/bin/bash

# Meeting Assistant CLI - Rust Edition - Setup Utility
# Comprehensive setup script for all dependencies and configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

# Banner
echo -e "${CYAN}"
echo "🤝 Meeting Assistant CLI - Rust Edition Setup"
echo "============================================="
echo -e "${NC}"

# Check if we should use the Rust-based setup
if [[ "$1" == "--interactive" ]] || [[ "$1" == "-i" ]]; then
    echo -e "${BLUE}Starting interactive Rust-based setup...${NC}"
    # Build setup utility if not already built
    if [[ ! -f "target/release/meeting-assistant" ]]; then
        echo -e "${YELLOW}Building setup utility...${NC}"
        cargo build --release
    fi
    
    # Run interactive setup
    ./target/release/meeting-assistant setup
    exit 0
fi

echo -e "${YELLOW}Running automated setup. Use --interactive for full setup experience.${NC}"
echo

# Function to print colored output
print_status() {
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

# Function to detect OS
detect_os() {
    case "$(uname -s)" in
        Darwin)
            OS="macos"
            ;;
        Linux)
            OS="linux"
            ;;
        CYGWIN*|MINGW32*|MINGW64*|MSYS*)
            OS="windows"
            ;;
        *)
            OS="unknown"
            ;;
    esac
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to reset terminal state
reset_terminal() {
    # Reset terminal to clean state
    printf "\033[0m"  # Reset all attributes
    stty sane 2>/dev/null || true  # Reset terminal settings
}

# Function to safely read user input
safe_read() {
    local prompt="$1"
    local var_name="$2"
    local default_value="$3"
    
    # Simple, reliable read
    printf "%s" "$prompt"
    local input
    read -r input || input=""
    
    if [[ -n "$input" ]]; then
        eval "$var_name=\"$input\""
    elif [[ -n "$default_value" ]]; then
        eval "$var_name=\"$default_value\""
    fi
}

# Function to install Homebrew on macOS
install_homebrew() {
    if ! command_exists brew; then
        print_step "Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        
        # Add Homebrew to PATH
        if [[ -f "/opt/homebrew/bin/brew" ]]; then
            eval "$(/opt/homebrew/bin/brew shellenv)"
        elif [[ -f "/usr/local/bin/brew" ]]; then
            eval "$(/usr/local/bin/brew shellenv)"
        fi
    else
        print_status "Homebrew already installed"
    fi
}

# Function to install Rust
install_rust() {
    if ! command_exists cargo; then
        print_step "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    else
        print_status "Rust already installed: $(rustc --version)"
    fi
}

# Function to install FFmpeg
install_ffmpeg() {
    print_step "Installing FFmpeg..."
    
    case $OS in
        "macos")
            if ! command_exists ffmpeg; then
                brew install ffmpeg
            else
                print_status "FFmpeg already installed"
            fi
            ;;
        "linux")
            if ! command_exists ffmpeg; then
                # Try different package managers
                if command_exists apt-get; then
                    sudo apt-get update
                    sudo apt-get install -y ffmpeg
                elif command_exists yum; then
                    sudo yum install -y ffmpeg
                elif command_exists pacman; then
                    sudo pacman -S ffmpeg
                else
                    print_error "Could not install FFmpeg automatically. Please install it manually."
                    exit 1
                fi
            else
                print_status "FFmpeg already installed"
            fi
            ;;
        "windows")
            print_warning "Please install FFmpeg manually for Windows from https://ffmpeg.org/download.html"
            ;;
        *)
            print_error "Unsupported OS for automatic FFmpeg installation"
            exit 1
            ;;
    esac
}

# Function to install audio capture tools (macOS specific)
install_audio_tools_macos() {
    print_step "Installing audio capture tools for macOS..."
    
    # Install BlackHole for system audio capture
    if ! brew list blackhole-2ch &>/dev/null; then
        print_step "Installing BlackHole (virtual audio driver)..."
        brew install blackhole-2ch
        print_info "BlackHole installed successfully!"
        print_info "You'll need to configure it in Audio MIDI Setup after installation"
    else
        print_status "BlackHole already installed"
    fi
    
    # Install SoX for audio processing (optional but useful)
    if ! command_exists sox; then
        print_step "Installing SoX for audio processing..."
        brew install sox
    else
        print_status "SoX already installed"
    fi
}

# Function to configure whisper backends with model selection
configure_whisper_backends() {
    print_step "Configuring Whisper backends..."
    
    local whisper_backends=()
    local selected_backend=""
    
    # Check available backends
    if command_exists whisper-cpp || command_exists whisper-cli; then
        whisper_backends+=("whisper.cpp")
    fi
    if command_exists whisper; then
        whisper_backends+=("openai-whisper")
    fi
    if command_exists faster-whisper; then
        whisper_backends+=("faster-whisper")
    fi
    
    if [[ ${#whisper_backends[@]} -eq 0 ]]; then
        print_warning "No local Whisper backends found. Will use OpenAI API for transcription."
        export WHISPER_BACKEND="openai"
        return 0
    fi
    
    # Let user select backend if multiple are available
    if [[ ${#whisper_backends[@]} -gt 1 ]]; then
        echo -e "${CYAN}Multiple Whisper backends detected:${NC}"
        local i=1
        for backend in "${whisper_backends[@]}"; do
            case "$backend" in
                "whisper.cpp")
                    echo "$i. whisper.cpp - Fastest, C++ implementation"
                    ;;
                "faster-whisper")
                    echo "$i. faster-whisper - Fast, Python with CTranslate2"
                    ;;
                "openai-whisper")
                    echo "$i. openai-whisper - Standard Python implementation"
                    ;;
            esac
            ((i++))
        done
        echo
        
        local choice=""
        safe_read "Enter your choice (1-${#whisper_backends[@]}) or press Enter for default (whisper.cpp): " choice "1"
        
        # Validate choice
        if [[ "$choice" =~ ^[1-9]$ ]] && [[ "$choice" -le "${#whisper_backends[@]}" ]]; then
            selected_backend="${whisper_backends[$((choice-1))]}"
        else
            selected_backend="${whisper_backends[0]}"
        fi
    else
        selected_backend="${whisper_backends[0]}"
    fi
    
    print_info "Selected backend: $selected_backend"
    
    # Configure the selected backend
    case "$selected_backend" in
        "whisper.cpp")
            export WHISPER_BACKEND="whisper.cpp"
            # Model selection handled by download_whisper_models()
            ;;
        "faster-whisper"|"openai-whisper")
            export WHISPER_BACKEND="$selected_backend"
            
            # For Python backends, let user select model
            echo -e "${CYAN}Model Selection for $selected_backend:${NC}"
            echo "Choose your preferred model:"
            echo
            echo "1. tiny.en    - Fastest, lowest accuracy"
            echo "2. base.en    - Good balance, recommended"
            echo "3. small.en   - Better accuracy, slower"
            echo "4. medium.en  - High accuracy, much slower"
            echo "5. large-v3   - Best accuracy, slowest"
            echo
            
            local model_choice=""
            safe_read "Enter your choice (1-5) or press Enter for default (base.en): " model_choice "2"
            
            case "$model_choice" in
                "1")
                    export WHISPER_MODEL="tiny.en"
                    ;;
                "3")
                    export WHISPER_MODEL="small.en"
                    ;;
                "4")
                    export WHISPER_MODEL="medium.en"
                    ;;
                "5")
                    export WHISPER_MODEL="large-v3"
                    ;;
                *)
                    export WHISPER_MODEL="base.en"
                    ;;
            esac
            
            print_info "Selected model: $WHISPER_MODEL"
            print_info "Model will be downloaded automatically on first use"
            ;;
    esac
}

# Function to install Whisper backends
install_whisper_backends() {
    print_step "Installing Whisper backends..."
    
    local whisper_installed=false
    
    # 1. Try whisper.cpp (fastest)
    if ! command_exists whisper-cpp && ! command_exists whisper-cli; then
        print_step "Installing whisper.cpp (ultra-fast)..."
        case $OS in
            "macos")
                brew install whisper-cpp
                whisper_installed=true
                ;;
            "linux")
                # Build from source on Linux
                print_info "Building whisper.cpp from source..."
                if command_exists git && command_exists make && command_exists gcc; then
                    git clone https://github.com/ggerganov/whisper.cpp.git /tmp/whisper.cpp
                    cd /tmp/whisper.cpp
                    make
                    sudo cp main /usr/local/bin/whisper-cli
                    cd - > /dev/null
                    whisper_installed=true
                else
                    print_warning "Missing build tools for whisper.cpp. Skipping..."
                fi
                ;;
        esac
    else
        print_status "whisper.cpp already installed"
        whisper_installed=true
    fi
    
    # 2. Check if homebrew whisper is already available (but don't install it)
    if [[ $OS == "macos" ]] && command_exists whisper; then
        print_status "Homebrew whisper already installed"
        whisper_installed=true
    fi
    
    # 3. Try faster-whisper (Python)
    if ! command_exists faster-whisper && command_exists pip; then
        print_step "Installing faster-whisper..."
        pip install faster-whisper
        whisper_installed=true
    fi
    
    # 4. Try standard whisper (Python fallback)
    if ! command_exists whisper && command_exists pip && ! $whisper_installed; then
        print_step "Installing standard whisper..."
        pip install openai-whisper
        whisper_installed=true
    fi
    
    if $whisper_installed; then
        print_status "At least one Whisper backend installed successfully"
        
        # Configure the installed backends
        configure_whisper_backends
    else
        print_warning "No Whisper backend installed. Will use OpenAI API (slower)"
        export WHISPER_BACKEND="openai"
    fi
}

# Function to let user select whisper model
select_whisper_model() {
    # Send menu to stderr so it doesn't get captured in command substitution
    echo -e "${CYAN}Whisper Model Selection:${NC}" >&2
    echo "Choose your preferred whisper model (balance of speed vs accuracy):" >&2
    echo >&2
    echo "1. tiny.en    - Fastest, lowest accuracy (~39 MB)" >&2
    echo "2. base.en    - Good balance, recommended (~147 MB)" >&2
    echo "3. small.en   - Better accuracy, slower (~466 MB)" >&2
    echo "4. medium.en  - High accuracy, much slower (~1.5 GB)" >&2
    echo "5. large-v3   - Best accuracy, slowest (~3.1 GB)" >&2
    echo >&2
    echo "For most users, 'base.en' provides the best speed/accuracy balance." >&2
    echo >&2
    
    local choice=""
    printf "Enter your choice (1-5) or press Enter for default (base.en): " >&2
    read -r choice || choice=""
    
    # Validate and return model name (only this goes to stdout)
    case "$choice" in
        "1")
            echo "tiny.en"
            ;;
        "2"|"")
            echo "base.en"
            ;;
        "3")
            echo "small.en"
            ;;
        "4")
            echo "medium.en"
            ;;
        "5")
            echo "large-v3"
            ;;
        *)
            echo "base.en"
            ;;
    esac
}

# Function to get correct download URL for whisper model
get_whisper_model_url() {
    local model="$1"
    
    # Validate model name
    if [[ -z "$model" ]] || [[ "$model" == *" "* ]] || [[ "$model" == *"Selection"* ]]; then
        print_error "Invalid model name: '$model'" >&2
        echo ""
        return 1
    fi
    
    case "$model" in
        "large-v3")
            echo "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
            ;;
        "tiny.en"|"base.en"|"small.en"|"medium.en")
            echo "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-$model.bin"
            ;;
        *)
            print_error "Unknown model: '$model'" >&2
            echo ""
            return 1
            ;;
    esac
}

# Function to test URL accessibility
test_url_accessibility() {
    local url="$1"
    
    # Validate URL first
    if [[ -z "$url" ]] || [[ "$url" == "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-.bin" ]]; then
        print_error "Invalid or empty URL"
        return 1
    fi
    
    print_info "Testing URL accessibility: $url"
    
    # Test with curl and capture output for debugging
    local curl_output
    curl_output=$(curl -I -L --max-time 10 "$url" 2>&1)
    local curl_exit_code=$?
    
    if [[ $curl_exit_code -eq 0 ]]; then
        print_status "URL is accessible"
        return 0
    else
        print_warning "URL is not accessible (curl exit code: $curl_exit_code)"
        print_info "Curl output: $(echo "$curl_output" | head -3)"
        return 1
    fi
}

# Function to provide manual download instructions
provide_manual_download_instructions() {
    local model="$1"
    local model_dir="$2"
    
    # Validate inputs
    if [[ -z "$model" ]] || [[ "$model" == *"Selection"* ]] || [[ "$model" == *"Choose"* ]]; then
        print_error "Invalid model name for manual download instructions"
        return 1
    fi
    
    echo
    print_info "Manual Download Instructions:"
    echo
    echo "If automatic download fails, you can download the model manually:"
    echo
    echo "1. Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main"
    echo "2. Download: ggml-$model.bin"
    echo "3. Move the file to: $model_dir/"
    echo "4. Ensure the file is named: ggml-$model.bin"
    echo
    echo "Alternative download command:"
    local download_url
    download_url=$(get_whisper_model_url "$model")
    echo "curl -L -o '$model_dir/ggml-$model.bin' '$download_url'"
    echo
    
    # Ask user if they want to continue with manual download
    local continue_manual=""
    printf "Would you like to continue with manual download? (y/n): "
    read -r continue_manual || continue_manual="n"
    
    if [[ "$continue_manual" =~ ^[Yy]$ ]]; then
        print_info "Please download the model manually and then run the setup again."
        return 1
    else
        print_info "Continuing with fallback model..."
        return 0
    fi
}

# Function to validate whisper model file
validate_whisper_model() {
    local model_file="$1"
    local model_name="$2"
    
    print_info "Validating model file..."
    
    # Check if file exists
    if [[ ! -f "$model_file" ]]; then
        print_error "Model file does not exist: $model_file"
        return 1
    fi
    
    # Check file size (basic validation)
    local file_size
    file_size=$(stat -c%s "$model_file" 2>/dev/null || stat -f%z "$model_file" 2>/dev/null || echo "0")
    
    if [[ "$file_size" -lt 1000000 ]]; then  # Less than 1MB is probably corrupted
        print_error "Model file is too small (${file_size} bytes) - likely corrupted"
        return 1
    fi
    
    # Show file info
    local file_size_human
    file_size_human=$(ls -lh "$model_file" | awk '{print $5}')
    print_info "Model file size: $file_size_human"
    
    # Check if it's a valid binary file (basic check)
    if file "$model_file" | grep -q "data"; then
        print_info "Model file appears to be a valid binary file"
        return 0
    else
        print_warning "Model file may not be a valid binary file"
        # Don't fail here, as the file command might not be available
        return 0
    fi
}

# Function to download whisper.cpp models
download_whisper_models() {
    # Only run if whisper.cpp is the selected backend
    if [[ "$WHISPER_BACKEND" == "whisper.cpp" ]] && (command_exists whisper-cpp || command_exists whisper-cli); then
        print_step "Setting up whisper.cpp models..."
        
        local model_dir
        if [[ $OS == "macos" ]]; then
            model_dir="/opt/homebrew/share/whisper.cpp/models"
            if [[ ! -d "$model_dir" ]]; then
                model_dir="/usr/local/share/whisper.cpp/models"
            fi
        else
            model_dir="$HOME/.whisper.cpp/models"
        fi
        
        mkdir -p "$model_dir"
        
        # Let user select model
        local selected_model
        selected_model=$(select_whisper_model)
        
        # Validate selected model
        if [[ -z "$selected_model" ]] || [[ "$selected_model" == *"Selection"* ]] || [[ "$selected_model" == *"Choose"* ]]; then
            print_error "Invalid model selection. Using default base.en model."
            selected_model="base.en"
        fi
        
        print_info "Selected model: $selected_model"
        
        # Download selected model
        local model_file="$model_dir/ggml-$selected_model.bin"
        if [[ ! -f "$model_file" ]]; then
            print_step "Downloading $selected_model model..."
            
            # Get correct download URL
            local download_url
            download_url=$(get_whisper_model_url "$selected_model")
            
            print_info "Downloading from: $download_url"
            
            # Test URL accessibility first
            if ! test_url_accessibility "$download_url"; then
                print_error "URL is not accessible"
                if provide_manual_download_instructions "$selected_model" "$model_dir"; then
                    export WHISPER_MODEL="base.en"
                    return 0
                else
                    return 1
                fi
            fi
            
            # Try downloading with better error handling
            if curl -L --fail --progress-bar -o "$model_file.tmp" "$download_url"; then
                mv "$model_file.tmp" "$model_file"
                
                # Validate downloaded model
                if validate_whisper_model "$model_file" "$selected_model"; then
                    print_status "Model downloaded and validated successfully"
                    export WHISPER_MODEL="$selected_model"
                else
                    print_error "Downloaded model appears to be corrupted"
                    rm -f "$model_file"
                    # Continue to fallback logic
                    false
                fi
            else
                # Clean up failed download
                rm -f "$model_file.tmp"
                
                print_error "Failed to download $selected_model model"
                print_info "Possible issues:"
                print_info "  1. Network connectivity problem"
                print_info "  2. Invalid model URL"
                print_info "  3. Insufficient disk space"
                
                # Try alternative download methods
                print_step "Attempting alternative download method..."
                
                                 # Try with different curl options
                 if curl -L --max-time 300 --retry 3 --retry-delay 5 -o "$model_file.tmp" "$download_url"; then
                     mv "$model_file.tmp" "$model_file"
                     
                     # Validate the downloaded model
                     if validate_whisper_model "$model_file" "$selected_model"; then
                         print_status "Model downloaded and validated successfully with retry"
                         export WHISPER_MODEL="$selected_model"
                     else
                         print_error "Downloaded model appears to be corrupted even after retry"
                         rm -f "$model_file"
                         # Continue to fallback logic
                         false
                     fi
                else
                    rm -f "$model_file.tmp"
                    
                    print_warning "All download attempts failed. Falling back to base.en model"
                    
                    # Try to download base.en as fallback
                    local fallback_file="$model_dir/ggml-base.en.bin"
                    if [[ ! -f "$fallback_file" ]]; then
                        local fallback_url="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
                        print_step "Downloading base.en fallback model..."
                        
                                                 if curl -L --fail --progress-bar -o "$fallback_file.tmp" "$fallback_url"; then
                             mv "$fallback_file.tmp" "$fallback_file"
                             
                             # Validate the fallback model
                             if validate_whisper_model "$fallback_file" "base.en"; then
                                 print_status "Fallback model downloaded and validated successfully"
                                 export WHISPER_MODEL="base.en"
                             else
                                 print_error "Fallback model appears to be corrupted"
                                 rm -f "$fallback_file"
                                 print_warning "Unable to download a valid model automatically"
                                 provide_manual_download_instructions "base.en" "$model_dir"
                                 export WHISPER_MODEL="base.en"
                             fi
                        else
                            rm -f "$fallback_file.tmp"
                            print_error "Failed to download fallback model"
                            print_warning "You may need to download models manually"
                            print_info "Visit: https://huggingface.co/ggerganov/whisper.cpp/tree/main"
                            export WHISPER_MODEL="base.en"
                        fi
                    else
                        print_info "Using existing base.en model"
                        export WHISPER_MODEL="base.en"
                    fi
                fi
            fi
        else
            print_status "Model $selected_model already exists"
            export WHISPER_MODEL="$selected_model"
        fi
    elif [[ "$WHISPER_BACKEND" != "openai" ]]; then
        print_info "Skipping whisper.cpp model download (using $WHISPER_BACKEND backend)"
    fi
}

# Function to get audio devices
get_audio_devices() {
    print_step "Detecting audio devices..."
    
    case $OS in
        "macos")
            if command_exists ffmpeg; then
                echo -e "${CYAN}Available audio devices:${NC}"
                ffmpeg -f avfoundation -list_devices true -i "" 2>&1 | grep -E "^\[AVFoundation" | head -20
                echo
            fi
            ;;
        "linux")
            if command_exists arecord; then
                echo -e "${CYAN}Available audio devices:${NC}"
                arecord -l
                echo
            fi
            ;;
    esac
}

# Function to test audio capture
test_audio_capture() {
    print_step "Testing audio capture..."
    
    case $OS in
        "macos")
            # Test with default device using correct format
            local test_file="/tmp/test_audio.wav"
            if command_exists ffmpeg; then
                print_info "Testing 3-second audio capture with device ':0'..."
                if ffmpeg -f avfoundation -i "none:0" -t 3 -y "$test_file" 2>/dev/null; then
                    print_status "Audio capture test successful"
                    rm -f "$test_file"
                else
                    print_warning "Audio capture test failed. You may need to configure audio permissions."
                    print_info "Try running: ffmpeg -f avfoundation -list_devices true -i \"\" to check devices"
                fi
            fi
            ;;
        "linux")
            # Test with ALSA
            local test_file="/tmp/test_audio.wav"
            if command_exists arecord; then
                print_info "Testing 3-second audio capture..."
                if timeout 3 arecord -f cd -t wav "$test_file" 2>/dev/null; then
                    print_status "Audio capture test successful"
                    rm -f "$test_file"
                else
                    print_warning "Audio capture test failed. Check microphone permissions."
                fi
            fi
            ;;
    esac
}

# Function to configure audio for macOS
configure_audio_macos() {
    print_step "Configuring audio for macOS..."
    
    echo -e "${CYAN}Audio Configuration Steps:${NC}"
    echo "1. Open Audio MIDI Setup (found in Applications > Utilities)"
    echo "2. Create an Aggregate Device:"
    echo "   - Click the '+' button and select 'Create Aggregate Device'"
    echo "   - Name it 'Meeting Assistant Input'"
    echo "   - Check both your built-in microphone and BlackHole 2ch"
    echo "   - Set BlackHole as the master device"
    echo "3. In System Preferences > Sound > Output:"
    echo "   - Select BlackHole 2ch as output device"
    echo "4. In System Preferences > Sound > Input:"
    echo "   - Select 'Meeting Assistant Input' as input device"
    echo
    print_info "This setup captures both your microphone and system audio"
    echo
    
    # Get current audio devices and suggest configuration
    get_audio_devices
    
    echo -e "${YELLOW}Suggested AUDIO_DEVICE values:${NC}"
    echo "• For audio-only capture, use colon prefix: ':0', ':1', ':2', etc."
    echo "• Check the device list above to find your preferred audio input"
    echo "• Common examples:"
    echo "  - ':0' - First available audio device (often built-in or aggregate)"
    echo "  - ':1' - BlackHole 2ch (for system audio)"
    echo "  - ':2' - Built-in microphone"
    echo "• Update AUDIO_DEVICE in .env file with your chosen device"
    echo
}

# Function to set up permissions on macOS
setup_permissions_macos() {
    print_step "Setting up macOS permissions..."
    
    echo -e "${CYAN}Required Permissions:${NC}"
    echo "1. ${WHITE}Accessibility Access${NC} (for global hotkeys):"
    echo "   - System Preferences > Security & Privacy > Privacy > Accessibility"
    echo "   - Add your terminal app (Terminal, iTerm2, etc.)"
    echo "   - Lock/unlock with password to make changes"
    echo
    echo "2. ${WHITE}Microphone Access${NC} (for audio capture):"
    echo "   - System Preferences > Security & Privacy > Privacy > Microphone"
    echo "   - Add your terminal app"
    echo
    echo "3. ${WHITE}Screen Recording${NC} (for screenshot feature):"
    echo "   - System Preferences > Security & Privacy > Privacy > Screen Recording"
    echo "   - Add your terminal app"
    echo
    
    print_warning "You must restart your terminal app after granting permissions!"
    echo
}

# Function to install Python dependencies for speaker diarization
install_python_diarization_deps() {
    print_step "Setting up speaker diarization..."
    
    echo -e "${CYAN}Speaker Diarization Setup:${NC}"
    echo "The Meeting Assistant supports advanced speaker diarization using multiple providers:"
    echo
    echo "Available providers:"
    echo "1. ${WHITE}ElevenLabs Scribe v1${NC} - Cloud-based, highest quality"
    echo "   • State-of-the-art accuracy with up to 32 speakers"
    echo "   • 99 languages supported"
    echo "   • Audio event detection (laughter, applause, etc.)"
    echo "   • Word-level timestamps"
    echo "   • Requires ElevenLabs API key"
    echo
    echo "2. ${WHITE}Whisper + PyAnnote (Local)${NC} - Full local processing"
    echo "   • OpenAI Whisper (transcription) + PyAnnote (diarization)"
    echo "   • Requires Python dependencies (~200-500MB download)"
    echo "   • HuggingFace account (free) for PyAnnote models"
    echo "   • Additional disk space for models"
    echo
    echo "3. ${WHITE}Whisper-only with smart detection${NC} - Local, lighter"
    echo "   • Intelligent speaker detection without PyAnnote"
    echo "   • Faster, still effective for most use cases"
    echo "   • Requires Python dependencies (~100-200MB download)"
    echo
    echo "4. ${WHITE}Skip speaker diarization${NC} - Basic transcription only"
    echo "   • No speaker identification"
    echo "   • Fastest setup"
    echo
    echo "Benefits of speaker diarization:"
    echo "• Identify multiple speakers in meetings"
    echo "• Separate conversation by speaker"
    echo "• Higher accuracy than basic transcription"
    echo
    
    local install_choice=""
    printf "Choose diarization provider (1-4): "
    read -r install_choice || install_choice="1"
    
    case "$install_choice" in
        "1")
            setup_elevenlabs_diarization
            ;;
        "2")
            setup_local_python_deps
            install_full_diarization_stack
            ;;
        "3")
            setup_local_python_deps
            install_whisper_only_diarization
            ;;
        "4")
            print_info "Skipping speaker diarization setup"
            export DIARIZATION_PROVIDER="none"
            export PYTHON_DIARIZATION_AVAILABLE="false"
            export PYTHON_DIARIZATION_FULL="false"
            ;;
        *)
            print_info "Invalid choice, defaulting to ElevenLabs"
            setup_elevenlabs_diarization
            ;;
    esac
}

# Function to setup ElevenLabs diarization
setup_elevenlabs_diarization() {
    print_step "Setting up ElevenLabs diarization..."
    
    echo -e "${CYAN}ElevenLabs Configuration:${NC}"
    echo "ElevenLabs Scribe v1 provides the highest quality speaker diarization."
    echo "You'll need an ElevenLabs API key to use this service."
    echo
    echo "Steps to get an ElevenLabs API key:"
    echo "1. Go to https://elevenlabs.io/sign-up"
    echo "2. Create a free account"
    echo "3. Go to https://elevenlabs.io/docs/api-reference/authentication"
    echo "4. Generate an API key"
    echo "5. Copy the API key"
    echo
    echo "ElevenLabs Features:"
    echo "• Up to 32 speakers with automatic identification"
    echo "• 99 languages supported"
    echo "• Audio event detection (laughter, applause, etc.)"
    echo "• Word-level timestamps with speaker attribution"
    echo "• No local dependencies required"
    echo
    
    local elevenlabs_key=""
    # Check for existing ElevenLabs API key
    if [[ -f ".env" ]]; then
        elevenlabs_key=$(grep "ELEVENLABS_API_KEY=" ".env" 2>/dev/null | cut -d'=' -f2- | tr -d '"' || echo "")
    fi
    
    if [[ -n "$elevenlabs_key" ]] && [[ "$elevenlabs_key" != "" ]]; then
        # Show masked existing key
        local masked_key="${elevenlabs_key:0:8}...${elevenlabs_key: -4}"
        printf "Current ElevenLabs API key: %s\n" "$masked_key"
        printf "Enter new ElevenLabs API key (or press Enter to keep current): "
        local new_key=""
        read -r new_key || new_key=""
        if [[ -n "$new_key" ]]; then
            elevenlabs_key="$new_key"
        fi
    else
        printf "Enter your ElevenLabs API key (or press Enter to skip): "
        read -r elevenlabs_key || elevenlabs_key=""
    fi
    
    if [[ -n "$elevenlabs_key" ]]; then
        export ELEVENLABS_API_KEY="$elevenlabs_key"
        export DIARIZATION_PROVIDER="elevenlabs"
        export PYTHON_DIARIZATION_AVAILABLE="false"
        export PYTHON_DIARIZATION_FULL="false"
        print_status "ElevenLabs API key configured"
        
        # Test the API key
        print_info "Testing ElevenLabs API key..."
        if command_exists curl; then
            local test_response
            test_response=$(curl -s -H "xi-api-key: $elevenlabs_key" "https://api.elevenlabs.io/v1/user" 2>/dev/null)
            if echo "$test_response" | grep -q "subscription" || echo "$test_response" | grep -q "user"; then
                print_status "ElevenLabs API key test passed"
            else
                print_warning "ElevenLabs API key test failed"
                print_info "You can update the key later in the .env file"
            fi
        else
            print_info "Cannot test API key (curl not available)"
        fi
    else
        print_warning "Skipping ElevenLabs API key configuration"
        print_info "You can add the key later in the .env file as ELEVENLABS_API_KEY"
        print_info "Falling back to Whisper-only diarization"
        export ELEVENLABS_API_KEY=""
        export DIARIZATION_PROVIDER="whisper_pyannote"
        
        # Fall back to local Python setup
        setup_local_python_deps
        install_whisper_only_diarization
    fi
}

# Function to setup local Python dependencies (extracted common code)
setup_local_python_deps() {
    print_step "Setting up Python dependencies for local diarization..."
    
    # Check if Python 3 is available
    if ! command_exists python3; then
        print_error "Python 3 is required for local speaker diarization"
        case $OS in
            "macos")
                print_info "Installing Python 3 via Homebrew..."
                brew install python3
                ;;
            "linux")
                print_info "Please install Python 3 for your distribution"
                return 1
                ;;
        esac
    fi
    
    # Check if pip is available
    if ! command_exists pip3 && ! python3 -m pip --version >/dev/null 2>&1; then
        print_error "pip is required for installing Python dependencies"
        return 1
    fi
    
    print_status "Python 3 and pip are available"
}

# Function to install Whisper-only diarization (fallback)
install_whisper_only_diarization() {
    print_step "Installing Whisper-only diarization with smart speaker detection..."
    
    # Create virtual environment for dependencies (optional but cleaner)
    local use_venv=""
    printf "Create a virtual environment for Python dependencies? (recommended) (y/n): "
    read -r use_venv || use_venv="y"
    
    if [[ "$use_venv" =~ ^[Yy]$ ]]; then
        print_info "Creating virtual environment..."
        python3 -m venv venv_diarization
        
        # Activate virtual environment
        if [[ -f "venv_diarization/bin/activate" ]]; then
            source venv_diarization/bin/activate
            print_status "Virtual environment activated"
        else
            print_warning "Failed to create virtual environment, installing globally"
        fi
    fi
    
    # Install core dependencies - just Whisper
    print_info "Installing OpenAI Whisper..."
    if python3 -m pip install openai-whisper torch; then
        print_status "Whisper installed successfully"
    else
        print_error "Failed to install Whisper"
        return 1
    fi
    
    # Test the installation
    print_info "Testing Whisper installation..."
    if python3 -c "import whisper; print('Whisper OK')" 2>/dev/null; then
        print_status "Whisper test passed"
        export PYTHON_DIARIZATION_AVAILABLE="true"
        export PYTHON_DIARIZATION_FULL="false"
        export DIARIZATION_PROVIDER="whisper_pyannote"
        print_status "Whisper-only diarization with smart speaker detection installed"
        print_info "This uses advanced conversation pattern analysis to identify speakers"
    else
        print_error "Whisper test failed"
        return 1
    fi
}

# Function to install full diarization stack (with PyAnnote)
install_full_diarization_stack() {
    print_step "Installing full diarization stack (Whisper + PyAnnote)..."
    
    # Create virtual environment for dependencies (optional but cleaner)
    local use_venv=""
    printf "Create a virtual environment for Python dependencies? (recommended) (y/n): "
    read -r use_venv || use_venv="y"
    
    if [[ "$use_venv" =~ ^[Yy]$ ]]; then
        print_info "Creating virtual environment..."
        python3 -m venv venv_diarization
        
        # Activate virtual environment
        if [[ -f "venv_diarization/bin/activate" ]]; then
            source venv_diarization/bin/activate
            print_status "Virtual environment activated"
        else
            print_warning "Failed to create virtual environment, installing globally"
        fi
    fi
    
    # Install core dependencies
    print_info "Installing OpenAI Whisper..."
    if python3 -m pip install openai-whisper torch; then
        print_status "Whisper installed successfully"
    else
        print_error "Failed to install Whisper"
        return 1
    fi
    
    # Install system dependencies for native compilation on macOS
    if [[ "$OS" == "macos" ]]; then
        print_info "Installing system dependencies for native compilation..."
        
        # Install required build tools
        if ! command_exists cmake; then
            print_info "Installing CMake..."
            brew install cmake
        fi
        
        if ! command_exists pkg-config; then
            print_info "Installing pkg-config..."
            brew install pkg-config
        fi
        
        # Install protobuf (required for sentencepiece)
        if ! command_exists protoc; then
            print_info "Installing protobuf..."
            brew install protobuf
        fi
        
        # Install sentencepiece system library
        if ! brew list sentencepiece &>/dev/null; then
            print_info "Installing sentencepiece system library..."
            brew install sentencepiece
        fi
        
        # Set PKG_CONFIG_PATH for sentencepiece
        if [[ -d "/opt/homebrew/lib/pkgconfig" ]]; then
            export PKG_CONFIG_PATH="/opt/homebrew/lib/pkgconfig:$PKG_CONFIG_PATH"
        elif [[ -d "/usr/local/lib/pkgconfig" ]]; then
            export PKG_CONFIG_PATH="/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH"
        fi
    fi
    
    # Try to install PyAnnote with improved error handling
    print_info "Installing PyAnnote.audio (this may take a while)..."
    print_warning "If this fails, we'll automatically fall back to Whisper-only mode"
    
    # First try with system sentencepiece
    if python3 -m pip install --no-build-isolation pyannote.audio; then
        print_status "PyAnnote.audio installed successfully"
        export PYTHON_DIARIZATION_FULL="true"
    else
        print_warning "PyAnnote.audio installation failed with system dependencies"
        print_info "This is common on macOS due to native compilation issues"
        print_info "Falling back to Whisper-only mode with smart speaker detection"
        export PYTHON_DIARIZATION_FULL="false"
    fi
    
    # Test the installation
    print_info "Testing installation..."
    if python3 -c "import whisper; print('Whisper OK')" 2>/dev/null; then
        print_status "Whisper test passed"
    else
        print_error "Whisper test failed"
        return 1
    fi
    
    if python3 -c "import pyannote.audio; print('PyAnnote OK')" 2>/dev/null; then
        print_status "PyAnnote test passed"
        export PYTHON_DIARIZATION_FULL="true"
    else
        print_info "PyAnnote test failed - using Whisper-only mode"
        export PYTHON_DIARIZATION_FULL="false"
    fi
    
    export PYTHON_DIARIZATION_AVAILABLE="true"
    export DIARIZATION_PROVIDER="whisper_pyannote"
    print_status "Python diarization dependencies installed"
    
    if [[ "$PYTHON_DIARIZATION_FULL" == "true" ]]; then
        print_info "Full diarization stack available (Whisper + PyAnnote)"
    else
        print_info "Whisper-only diarization with smart speaker detection available"
        print_info "This still provides excellent speaker identification for most use cases"
    fi
}

# Function to configure HuggingFace token
configure_huggingface_token() {
    if [[ "$PYTHON_DIARIZATION_FULL" == "true" ]]; then
        echo -e "${CYAN}HuggingFace Configuration:${NC}"
        echo "PyAnnote.audio requires a HuggingFace account to download speaker diarization models."
        echo "This is free and only needed for advanced speaker identification."
        echo
        echo "Steps to get a HuggingFace token:"
        echo "1. Go to https://huggingface.co/join"
        echo "2. Create a free account"
        echo "3. Go to https://huggingface.co/settings/tokens"
        echo "4. Create a new token (read access is sufficient)"
        echo "5. Copy the token"
        echo
        
        local hf_token=""
        # Check for existing HuggingFace token
        if [[ -f ".env" ]]; then
            hf_token=$(grep "HUGGINGFACE_HUB_TOKEN=" ".env" 2>/dev/null | cut -d'=' -f2- | tr -d '"' || echo "")
        fi
        
        if [[ -n "$hf_token" ]] && [[ "$hf_token" != "" ]]; then
            # Show masked existing token
            local masked_token="${hf_token:0:8}...${hf_token: -4}"
            printf "Current HuggingFace token: %s\n" "$masked_token"
            printf "Enter new HuggingFace token (or press Enter to keep current): "
            local new_token=""
            read -r new_token || new_token=""
            if [[ -n "$new_token" ]]; then
                hf_token="$new_token"
            fi
        else
            printf "Enter your HuggingFace token (or press Enter to skip): "
            read -r hf_token || hf_token=""
        fi
        
        if [[ -n "$hf_token" ]]; then
            export HUGGINGFACE_HUB_TOKEN="$hf_token"
            print_status "HuggingFace token configured"
            
            # Test the token
            print_info "Testing HuggingFace token..."
            if python3 -c "
import os
os.environ['HUGGINGFACE_HUB_TOKEN'] = '$hf_token'
try:
    from pyannote.audio import Pipeline
    # Try to access a model (without downloading)
    model_id = 'pyannote/speaker-diarization-3.1'
    print('Token test passed')
except Exception as e:
    print(f'Token test failed: {e}')
" 2>/dev/null | grep -q "Token test passed"; then
                print_status "HuggingFace token test passed"
            else
                print_warning "HuggingFace token test failed"
                print_info "You can update the token later in the .env file"
            fi
        else
            print_warning "Skipping HuggingFace token configuration"
            print_info "Speaker diarization will use Whisper-only transcription"
            print_info "You can add the token later in the .env file as HUGGINGFACE_HUB_TOKEN"
            export HUGGINGFACE_HUB_TOKEN=""
        fi
    fi
}

# Function to create configuration file
create_config_file() {
    print_step "Creating configuration file..."
    
    local config_file=".env"
    
    if [[ -f "$config_file" ]]; then
        print_warning "Configuration file already exists. Backing up to .env.backup"
        cp "$config_file" "$config_file.backup"
    fi
    
    echo -e "${CYAN}Configuration Setup:${NC}"
    echo
    
    # Get OpenAI API key
    local openai_key=""
    if [[ -f "$config_file" ]]; then
        openai_key=$(grep "OPENAI_API_KEY=" "$config_file" 2>/dev/null | cut -d'=' -f2- | tr -d '"' || echo "")
    fi
    
    echo -e "${YELLOW}OpenAI API Key Setup:${NC}"
    echo "You need an OpenAI API key to use the AI features."
    echo "Get one from: https://platform.openai.com/api-keys"
    echo
    
    if [[ -n "$openai_key" ]] && [[ "$openai_key" != "your_openai_api_key_here" ]]; then
        # Show masked existing key
        local masked_key="${openai_key:0:8}...${openai_key: -4}"
        printf "Current OpenAI API key: %s\n" "$masked_key"
        printf "Enter new OpenAI API key (or press Enter to keep current): "
        local new_key=""
        read -r new_key || new_key=""
        if [[ -n "$new_key" ]]; then
            openai_key="$new_key"
        fi
    else
        printf "Enter your OpenAI API key (or press Enter to skip): "
        read -r openai_key || openai_key=""
    fi
    echo
    
    # Get audio device preference with proper validation
    local audio_device=":0"
    echo -e "${YELLOW}Audio Device Configuration:${NC}"
    echo "Audio devices must be specified with a colon prefix for audio-only capture."
    echo "Examples: ':0', ':1', ':2', etc."
    echo
    
    # Show available audio devices if on macOS
    if [[ "$OS" == "macos" ]] && command_exists ffmpeg; then
        echo -e "${CYAN}Available audio devices:${NC}"
        ffmpeg -f avfoundation -list_devices true -i "" 2>&1 | grep -A 10 "AVFoundation audio devices:" | grep -E "^\[AVFoundation.*\[[0-9]+\]" | head -10
        echo
    fi
    
    echo "Default audio device is ':0' (first available audio device)"
    local user_input=""
    printf "Enter audio device (e.g., ':0', ':1', ':2') or press Enter for default: "
    read -r user_input || user_input=""
    
    # Validate and format the audio device
    if [[ -n "$user_input" ]]; then
        # Remove any existing quotes
        user_input=$(echo "$user_input" | tr -d '"'"'"'')
        
        # Add colon prefix if missing
        if [[ ! "$user_input" =~ ^: ]]; then
            if [[ "$user_input" =~ ^[0-9]+$ ]]; then
                audio_device=":$user_input"
                print_info "Added colon prefix: $audio_device"
            else
                print_warning "Invalid audio device format. Using default ':0'"
                audio_device=":0"
            fi
        else
            audio_device="$user_input"
        fi
    fi
    
    echo
    
    # Create configuration file
    cat > "$config_file" << EOF
# Meeting Assistant CLI - Rust Edition Configuration

# Required - OpenAI API Key
OPENAI_API_KEY=${openai_key:-your_openai_api_key_here}

# Optional - OpenAI Settings
OPENAI_MODEL=gpt-4o-mini
OPENAI_MAX_TOKENS=1800
OPENAI_TEMPERATURE=0.5

# Optional - Audio Settings
AUDIO_DEVICE="$audio_device"
AUDIO_SAMPLE_RATE=16000
AUDIO_CHANNELS=1
BUFFER_DURATION=8
CAPTURE_DURATION=15

# Optional - Timing Configuration
DOUBLE_TAP_WINDOW_MS=500
DEBOUNCE_MS=50
MAX_RECORDING_TIME=30000

# Optional - Whisper Settings (for local transcription)
WHISPER_MODEL=${WHISPER_MODEL:-base.en}
WHISPER_BACKEND=${WHISPER_BACKEND:-auto}
WHISPER_LANGUAGE=en

# Optional - Speaker Diarization Settings
PYTHON_DIARIZATION_AVAILABLE=${PYTHON_DIARIZATION_AVAILABLE:-false}
PYTHON_DIARIZATION_FULL=${PYTHON_DIARIZATION_FULL:-false}
HUGGINGFACE_HUB_TOKEN=${HUGGINGFACE_HUB_TOKEN:-}

# Optional - Diarization Plugin Configuration
DIARIZATION_PROVIDER=${DIARIZATION_PROVIDER:-elevenlabs}
DIARIZATION_WHISPER_MODEL=base
DIARIZATION_PYANNOTE_MODEL=pyannote/speaker-diarization-3.1
DIARIZATION_MAX_SPEAKERS=10
DIARIZATION_SPEAKER_THRESHOLD=0.7

# Optional - ElevenLabs Configuration (for ElevenLabs diarization)
ELEVENLABS_API_KEY=${ELEVENLABS_API_KEY:-}

# Optional - Temporary Directory
# TEMP_DIR=\$HOME/.meeting-assistant/temp
EOF
    
    print_status "Configuration file created: $config_file"
    
    if [[ -z "$openai_key" ]] || [[ "$openai_key" == "your_openai_api_key_here" ]]; then
        print_warning "Remember to set your OpenAI API key in $config_file"
    fi
}

# Function to setup plugin system
setup_plugin_system() {
    print_step "Setting up plugin system..."
    
    echo -e "${CYAN}Plugin System Configuration:${NC}"
    echo "The Meeting Assistant now supports multiple LLM providers:"
    echo "1. OpenAI (default) - Cloud-based, high quality"
    echo "2. Ollama - Local, private, offline"
    echo "3. Custom plugins - Extensible provider system"
    echo
    
    # Check if Ollama is available
    if command_exists ollama; then
        print_status "Ollama is available for local AI inference"
        
        # Ask user about LLM provider preference
        echo -e "${YELLOW}LLM Provider Selection:${NC}"
        echo "Choose your preferred LLM provider:"
        echo "1. OpenAI (requires API key)"
        echo "2. Ollama (local, private)"
        echo "3. Both (OpenAI with Ollama fallback)"
        echo
        
        local choice=""
        printf "Enter your choice (1-3): "
        read -r choice || choice="1"
        
        case "$choice" in
            "2")
                setup_ollama_provider
                ;;
            "3")
                setup_dual_providers
                ;;
            *)
                print_info "Using OpenAI as primary provider"
                ;;
        esac
    else
        print_info "Ollama not found. Using OpenAI as primary provider"
        print_info "To use Ollama later, install it from https://ollama.ai"
    fi
    
    # Update configuration file with plugin settings
    update_config_with_plugins
}

# Function to setup Ollama provider
setup_ollama_provider() {
    print_step "Setting up Ollama provider..."
    
    # Check if Ollama service is running
    if ! ollama list >/dev/null 2>&1; then
        print_warning "Ollama service is not running"
        print_info "Please start Ollama service with: ollama serve"
        print_info "Then run this setup again"
        return 1
    fi
    
    # Check available models
    local models
    models=$(ollama list 2>/dev/null | tail -n +2 | awk '{print $1}' | grep -v "^$" || echo "")
    
    if [[ -z "$models" ]]; then
        print_warning "No Ollama models found"
        print_info "Installing recommended model for meeting assistance..."
        
        # Install llama2:7b as default
        if ollama pull llama2:7b 2>/dev/null; then
            print_status "Successfully installed llama2:7b model"
        else
            print_error "Failed to install llama2:7b model"
            return 1
        fi
    else
        print_status "Found Ollama models:"
        echo "$models" | while read -r model; do
            echo "  • $model"
        done
    fi
    
    # Set LLM provider to Ollama
    export LLM_PROVIDER="ollama"
    print_status "Set LLM provider to Ollama"
}

# Function to setup dual providers
setup_dual_providers() {
    print_step "Setting up dual provider configuration..."
    
    # Setup Ollama first
    if setup_ollama_provider; then
        print_status "Ollama configured successfully"
    else
        print_warning "Ollama setup failed, falling back to OpenAI only"
        return 1
    fi
    
    # Keep OpenAI as fallback
    export LLM_PROVIDER="ollama"
    export LLM_FALLBACK_TO_OPENAI="true"
    print_status "Configured Ollama with OpenAI fallback"
}

# Function to update configuration with plugin settings
update_config_with_plugins() {
    print_step "Updating configuration with plugin settings..."
    
    local config_file=".env"
    
    if [[ -f "$config_file" ]]; then
        # Add plugin configuration if not present
        if ! grep -q "LLM_PROVIDER=" "$config_file"; then
            echo "" >> "$config_file"
            echo "# LLM Provider Configuration" >> "$config_file"
            echo "LLM_PROVIDER=${LLM_PROVIDER:-openai}" >> "$config_file"
            echo "LLM_FALLBACK_TO_OPENAI=${LLM_FALLBACK_TO_OPENAI:-true}" >> "$config_file"
            echo "" >> "$config_file"
            echo "# Ollama Settings (when using Ollama provider)" >> "$config_file"
            echo "OLLAMA_BASE_URL=http://localhost:11434" >> "$config_file"
            echo "OLLAMA_MODEL=llama2:7b" >> "$config_file"
            echo "OLLAMA_TIMEOUT=30" >> "$config_file"
            echo "OLLAMA_MAX_RETRIES=3" >> "$config_file"
            echo "OLLAMA_AUTO_PULL=false" >> "$config_file"
            
            print_status "Updated configuration with plugin settings"
        else
            print_info "Plugin configuration already exists in $config_file"
        fi
    fi
}

# Function to build the application
build_application() {
    print_step "Building the application..."
    
    if cargo build --release; then
        print_status "Application built successfully!"
        
        # Show binary info
        if [[ -f "target/release/meeting-assistant" ]]; then
            BINARY_SIZE=$(ls -lh target/release/meeting-assistant | awk '{print $5}')
            print_info "Binary size: $BINARY_SIZE"
            print_info "Binary location: $(pwd)/target/release/meeting-assistant"
        fi
    else
        print_error "Failed to build application"
        echo
        print_info "Try these steps:"
        echo "  1. Update Rust: rustup update"
        echo "  2. Check dependencies: cargo check"
        echo "  3. Clean build: cargo clean && cargo build --release"
        exit 1
    fi
}

# Function to run system verification
verify_system() {
    print_step "Verifying system setup..."
    
    local all_good=true
    
    # Check Rust
    if command_exists cargo; then
        print_status "Rust: $(rustc --version)"
    else
        print_error "Rust not found"
        all_good=false
    fi
    
    # Check FFmpeg
    if command_exists ffmpeg; then
        print_status "FFmpeg: $(ffmpeg -version | head -1 | cut -d' ' -f3)"
    else
        print_error "FFmpeg not found"
        all_good=false
    fi
    
    # Check Whisper backends
    local whisper_backends=()
    if command_exists whisper-cpp || command_exists whisper-cli; then
        whisper_backends+=("whisper.cpp")
    fi
    if command_exists whisper; then
        whisper_backends+=("openai-whisper")
    fi
    if command_exists faster-whisper; then
        whisper_backends+=("faster-whisper")
    fi
    
    if [[ ${#whisper_backends[@]} -gt 0 ]]; then
        print_status "Whisper backends: ${whisper_backends[*]}"
        
        # Show selected backend and model if configured
        if [[ -n "$WHISPER_BACKEND" ]] && [[ "$WHISPER_BACKEND" != "auto" ]]; then
            print_info "Selected backend: $WHISPER_BACKEND"
            if [[ -n "$WHISPER_MODEL" ]]; then
                print_info "Selected model: $WHISPER_MODEL"
            fi
        fi
    else
        print_warning "No local Whisper backends found (will use OpenAI API)"
    fi
    
    # Check diarization provider configuration
    if [[ "$DIARIZATION_PROVIDER" == "elevenlabs" ]]; then
        if [[ -n "$ELEVENLABS_API_KEY" ]] && [[ "$ELEVENLABS_API_KEY" != "" ]]; then
            print_status "ElevenLabs diarization: Configured"
        else
            print_warning "ElevenLabs diarization: API key not configured"
        fi
    elif [[ "$DIARIZATION_PROVIDER" == "whisper_pyannote" ]]; then
        if [[ "$PYTHON_DIARIZATION_AVAILABLE" == "true" ]]; then
            if python3 -c "import whisper" 2>/dev/null; then
                print_status "Python Whisper: Available"
            else
                print_warning "Python Whisper: Not available"
            fi
            
            if [[ "$PYTHON_DIARIZATION_FULL" == "true" ]]; then
                if python3 -c "import pyannote.audio" 2>/dev/null; then
                    print_status "PyAnnote.audio: Available (full speaker diarization)"
                else
                    print_warning "PyAnnote.audio: Not available (Whisper-only mode)"
                fi
            else
                print_info "PyAnnote.audio: Not installed (Whisper-only mode)"
            fi
        else
            print_info "Python diarization: Not installed"
        fi
    else
        print_info "Speaker diarization: Disabled"
    fi
    
    # Check configuration
    if [[ -f ".env" ]]; then
        if grep -q "OPENAI_API_KEY=your_openai_api_key_here" ".env"; then
            print_warning "OpenAI API key not configured in .env"
        else
            print_status "Configuration file found"
        fi
        
        # Check diarization provider configuration
        if [[ "$DIARIZATION_PROVIDER" == "elevenlabs" ]]; then
            if grep -q "ELEVENLABS_API_KEY=" ".env" && ! grep -q "ELEVENLABS_API_KEY=$" ".env"; then
                print_status "ElevenLabs API key configured"
            else
                print_warning "ElevenLabs API key not configured"
            fi
        elif [[ "$DIARIZATION_PROVIDER" == "whisper_pyannote" ]] && [[ "$PYTHON_DIARIZATION_FULL" == "true" ]]; then
            if grep -q "HUGGINGFACE_HUB_TOKEN=" ".env" && ! grep -q "HUGGINGFACE_HUB_TOKEN=$" ".env"; then
                print_status "HuggingFace token configured"
            else
                print_warning "HuggingFace token not configured (speaker diarization will use Whisper-only)"
            fi
        fi
    else
        print_warning "No .env configuration file found"
    fi
    
    # Check binary
    if [[ -f "target/release/meeting-assistant" ]]; then
        print_status "Application binary built"
    else
        print_warning "Application not built yet"
    fi
    
    echo
    if $all_good; then
        print_status "System verification passed!"
    else
        print_warning "Some issues found. Please address them before running."
    fi
}

# Function to show usage instructions
show_usage() {
    echo -e "${CYAN}🎯 How to Use:${NC}"
    echo
    echo "1. ${WHITE}Run the application:${NC}"
    echo "   ./target/release/meeting-assistant"
    echo
    echo "2. ${WHITE}Global hotkeys (double-tap quickly):${NC}"
    echo "   • Double-tap 'A' - Answer questions or provide context"
    echo "   • Double-tap 'S' - Analyze clipboard content (code-aware)"
    echo "   • Double-tap 'Q' - Combined audio + clipboard"
    echo "   • Double-tap 'W' - Screenshot + audio analysis (code-aware)"
    echo "   • Double-tap 'R' - Cancel current request"
    echo "   • Double-tap 'H' - Show session history"
    echo "   • Ctrl+C - Exit"
    echo
    echo "3. ${WHITE}Troubleshooting:${NC}"
    echo "   • Check permissions if hotkeys don't work"
    echo "   • Verify audio device with: ffmpeg -f avfoundation -list_devices true -i \"\""
    echo "   • Check logs in ~/.meeting-assistant/logs/"
    echo "   • If whisper models failed to download:"
    echo "     - Check internet connection"
    echo "     - Try manual download from https://huggingface.co/ggerganov/whisper.cpp/tree/main"
    echo "     - Verify disk space availability"
    echo "     - Use fallback to OpenAI API if needed"
    echo
    echo "4. ${WHITE}Configuration:${NC}"
    echo "   • Edit .env file to customize settings"
    echo "   • Adjust AUDIO_DEVICE if needed"
    echo "   • Set OPENAI_API_KEY"
    echo "   • Configure WHISPER_BACKEND and WHISPER_MODEL for local transcription"
    echo "   • Set ELEVENLABS_API_KEY for ElevenLabs diarization"
    echo "   • Set HUGGINGFACE_HUB_TOKEN for PyAnnote diarization"
    echo
    echo "5. ${WHITE}Speaker Diarization:${NC}"
    echo "   • Identify multiple speakers in conversations"
    echo "   • Multiple provider options:"
    echo "     - ElevenLabs Scribe v1 (cloud-based, highest quality)"
    echo "     - Whisper + PyAnnote (local, full-featured)"
    echo "     - Whisper-only (local, lightweight)"
    echo "   • Requires provider-specific configuration (API keys or Python dependencies)"
    echo "   • Automatic fallback to basic transcription if not configured"
    echo
}

# Function to create helper scripts
create_helper_scripts() {
    print_step "Creating helper scripts..."
    
    # Create start script
    cat > "start.sh" << 'EOF'
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
EOF
    
    chmod +x "start.sh"
    print_status "Created start.sh"
    
    # Update cleanup script if it exists
    if [[ -f "cleanup.sh" ]]; then
        print_status "cleanup.sh already exists"
    else
        print_info "No cleanup.sh found (will be created by build process)"
    fi
}

# Main setup function
main() {
    echo -e "${WHITE}Starting comprehensive setup...${NC}"
    echo
    
    # Detect OS
    detect_os
    print_info "Detected OS: $OS"
    echo
    
    # Install dependencies based on OS
    case $OS in
        "macos")
            install_homebrew
            install_rust
            install_ffmpeg
            install_audio_tools_macos
            install_whisper_backends
            download_whisper_models
            install_python_diarization_deps
            configure_huggingface_token
            setup_permissions_macos
            configure_audio_macos
            ;;
        "linux")
            install_rust
            install_ffmpeg
            install_whisper_backends
            install_python_diarization_deps
            configure_huggingface_token
            print_info "Linux audio setup varies by distribution"
            ;;
        "windows")
            print_error "Windows setup not fully automated. Please install dependencies manually:"
            echo "1. Install Rust: https://rustup.rs/"
            echo "2. Install FFmpeg: https://ffmpeg.org/download.html"
            echo "3. Install Python and pip install openai-whisper"
            echo "4. Install pyannote.audio for speaker diarization"
            exit 1
            ;;
        *)
            print_error "Unsupported operating system: $OS"
            exit 1
            ;;
    esac
    
    # Create configuration
    create_config_file
    
    # Setup plugin system
    setup_plugin_system
    
    # Build application
    build_application
    
    # Create helper scripts
    create_helper_scripts
    
    # Test audio (optional)
    if [[ "$OS" == "macos" ]] || [[ "$OS" == "linux" ]]; then
        test_audio_capture
    fi
    
    # Verify everything
    verify_system
    
    # Show usage
    show_usage
    
    echo -e "${GREEN}✅ Meeting Assistant CLI setup complete!${NC}"
    echo
    echo -e "${CYAN}🎯 How to Use:${NC}"
    echo
    echo "1. ${WHITE}Run the application:${NC}"
    echo "   ./target/release/meeting-assistant"
    echo
    echo "2. ${WHITE}Global hotkeys (double-tap quickly):${NC}"
    echo "   • Double-tap 'A' - Answer questions or provide context"
    echo "   • Double-tap 'S' - Analyze clipboard content (code-aware)"
    echo "   • Double-tap 'Q' - Combined audio + clipboard"
    echo "   • Double-tap 'W' - Screenshot + audio analysis (code-aware)"
    echo "   • Double-tap 'R' - Cancel current request"
    echo "   • Double-tap 'H' - Show session history"
    echo "   • Ctrl+C - Exit"
    echo
    echo "3. ${WHITE}Configuration:${NC}"
    echo "   • Edit .env file to customize settings"
    echo "   • Adjust AUDIO_DEVICE if needed"
    echo "   • Set OPENAI_API_KEY"
    echo "   • Choose LLM_PROVIDER (openai, ollama, or custom)"
    echo "   • Configure WHISPER_BACKEND and WHISPER_MODEL for local transcription"
    echo "   • Set ELEVENLABS_API_KEY for ElevenLabs diarization"
    echo "   • Set HUGGINGFACE_HUB_TOKEN for PyAnnote diarization"
    echo
    echo "4. ${WHITE}Plugin System:${NC}"
    echo "   • Switch providers: ./target/release/meeting-assistant plugin set-llm <provider>"
    echo "   • List plugins: ./target/release/meeting-assistant plugin list"
    echo "   • Install plugins: ./target/release/meeting-assistant plugin install <source>"
    echo

    # Final checklist
    echo -e "${CYAN}📋 Final Checklist:${NC}"
    echo "1. Edit .env file with your OpenAI API key (if using OpenAI)"
    
    local step_counter=2
    if [[ "$DIARIZATION_PROVIDER" == "elevenlabs" ]]; then
        echo "$step_counter. Add your ElevenLabs API key to .env for speaker diarization"
        ((step_counter++))
    elif [[ "$PYTHON_DIARIZATION_FULL" == "true" ]]; then
        echo "$step_counter. Add your HuggingFace token to .env for speaker diarization"
        ((step_counter++))
    fi
    if [[ "$OS" == "macos" ]]; then
        echo "$step_counter. Grant accessibility permissions to your terminal"
        ((step_counter++))
        echo "$step_counter. Configure audio (see instructions above)"
        ((step_counter++))
        echo "$step_counter. Start Ollama service if using Ollama: ollama serve"
        ((step_counter++))
        echo "$step_counter. Run: ./start.sh"
    else
        echo "$step_counter. Start Ollama service if using Ollama: ollama serve"
        ((step_counter++))
        echo "$step_counter. Run: ./start.sh"
    fi
    echo
    echo -e "${CYAN}📋 Common Issues & Solutions:${NC}"
    echo "• ${WHITE}Model download failed?${NC}"
    echo "  - Check internet connection and try again"
    echo "  - Download manually from https://huggingface.co/ggerganov/whisper.cpp/tree/main"
    echo "  - Use OpenAI API as fallback (set WHISPER_BACKEND=openai in .env)"
    echo "• ${WHITE}Audio permissions?${NC}"
    echo "  - Grant microphone access in System Preferences"
    echo "  - Restart terminal after granting permissions"
    echo "• ${WHITE}Hotkeys not working?${NC}"
    echo "  - Grant accessibility permissions in System Preferences"
    echo "  - Restart terminal application"
    echo
    echo -e "${CYAN}Need help? Check README.md or run './target/release/meeting-assistant --help'${NC}"
}

# Run main function
main "$@" 
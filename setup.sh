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
    
    # Reset terminal and display prompt
    reset_terminal
    printf "%s" "$prompt"
    
    # Use a clean read with timeout
    if read -r -t 300 input; then
        if [[ -n "$input" ]]; then
            eval "$var_name=\"$input\""
        elif [[ -n "$default_value" ]]; then
            eval "$var_name=\"$default_value\""
        fi
    else
        # Timeout or error, use default
        if [[ -n "$default_value" ]]; then
            eval "$var_name=\"$default_value\""
        fi
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
    
    # 2. Try Homebrew whisper (if available)
    if [[ $OS == "macos" ]] && ! command_exists whisper; then
        print_step "Installing Homebrew whisper..."
        brew install whisper
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
    else
        print_warning "No Whisper backend installed. Will use OpenAI API (slower)"
    fi
}

# Function to download whisper.cpp models
download_whisper_models() {
    if command_exists whisper-cpp || command_exists whisper-cli; then
        print_step "Downloading whisper.cpp models..."
        
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
        
        # Download base.en model (good balance of speed and accuracy)
        local model_file="$model_dir/ggml-base.en.bin"
        if [[ ! -f "$model_file" ]]; then
            print_step "Downloading base.en model..."
            curl -L -o "$model_file" "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
            print_status "Model downloaded to $model_file"
        else
            print_status "whisper.cpp model already exists"
        fi
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
    
    # Reset terminal state before prompts
    reset_terminal
    
    # Get OpenAI API key
    local openai_key=""
    if [[ -f "$config_file" ]]; then
        openai_key=$(grep "OPENAI_API_KEY=" "$config_file" 2>/dev/null | cut -d'=' -f2- | tr -d '"' || echo "")
    fi
    
    if [[ -z "$openai_key" ]]; then
        echo -e "${YELLOW}OpenAI API Key Setup:${NC}"
        echo "You need an OpenAI API key to use the AI features."
        echo "Get one from: https://platform.openai.com/api-keys"
        echo
        safe_read "Enter your OpenAI API key (or press Enter to skip): " openai_key ""
        echo
    fi
    
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
    safe_read "Enter audio device (e.g., ':0', ':1', ':2') or press Enter for default: " user_input ""
    
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
        safe_read "Enter your choice (1-3): " choice "1"
        
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
        whisper_backends+=("whisper")
    fi
    if command_exists faster-whisper; then
        whisper_backends+=("faster-whisper")
    fi
    
    if [[ ${#whisper_backends[@]} -gt 0 ]]; then
        print_status "Whisper backends: ${whisper_backends[*]}"
    else
        print_warning "No local Whisper backends found (will use OpenAI API)"
    fi
    
    # Check configuration
    if [[ -f ".env" ]]; then
        if grep -q "OPENAI_API_KEY=your_openai_api_key_here" ".env"; then
            print_warning "OpenAI API key not configured in .env"
        else
            print_status "Configuration file found"
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
    echo
    echo "4. ${WHITE}Configuration:${NC}"
    echo "   • Edit .env file to customize settings"
    echo "   • Adjust AUDIO_DEVICE if needed"
    echo "   • Set OPENAI_API_KEY"
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
            setup_permissions_macos
            configure_audio_macos
            ;;
        "linux")
            install_rust
            install_ffmpeg
            install_whisper_backends
            print_info "Linux audio setup varies by distribution"
            ;;
        "windows")
            print_error "Windows setup not fully automated. Please install dependencies manually:"
            echo "1. Install Rust: https://rustup.rs/"
            echo "2. Install FFmpeg: https://ffmpeg.org/download.html"
            echo "3. Install Python and pip install openai-whisper"
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
    echo
    echo "4. ${WHITE}Plugin System:${NC}"
    echo "   • Switch providers: ./target/release/meeting-assistant plugin set-llm <provider>"
    echo "   • List plugins: ./target/release/meeting-assistant plugin list"
    echo "   • Install plugins: ./target/release/meeting-assistant plugin install <source>"
    echo

    # Final checklist
    echo -e "${CYAN}📋 Final Checklist:${NC}"
    echo "1. Edit .env file with your OpenAI API key (if using OpenAI)"
    if [[ "$OS" == "macos" ]]; then
        echo "2. Grant accessibility permissions to your terminal"
        echo "3. Configure audio (see instructions above)"
        echo "4. Start Ollama service if using Ollama: ollama serve"
        echo "5. Run: ./start.sh"
    else
        echo "2. Start Ollama service if using Ollama: ollama serve"
        echo "3. Run: ./start.sh"
    fi
    echo
    echo -e "${CYAN}Need help? Check README.md or run './target/release/meeting-assistant --help'${NC}"
}

# Run main function
main "$@" 
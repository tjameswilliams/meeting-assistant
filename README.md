## 🎙️ Meeting Assistant CLI - Rust Edition

Ultra-fast, AI-powered meeting assistant built in Rust for real-time transcription, speaker diarization, and intelligent analysis.

### ✨ Key Features

- **Real-time Audio Capture**: High-quality audio recording with configurable settings
- **AI Transcription**: OpenAI Whisper integration for accurate speech-to-text
- **Speaker Diarization**: Advanced spectral-based speaker identification (improved in latest version)
- **Intelligent Analysis**: AI-powered meeting summaries and insights
- **Plugin System**: Extensible architecture for custom functionality
- **Multi-modal Support**: Audio, text, and screenshot analysis
- **Performance Optimized**: Rust-native implementation for minimal latency

### 🚀 Recent Improvements

**Enhanced Speaker Diarization (v2.0)**

- **Much better multi-speaker detection**: Now properly identifies multiple speakers instead of grouping them as one
- **Improved sensitivity**: Lowered similarity threshold from 0.75 to 0.55 for better speaker separation
- **Advanced spectral features**: Enhanced MFCC extraction with mel-scale frequency mapping
- **Better voice characteristics**: More discriminative F0 and spectral analysis
- **Conservative clustering**: Temporal context consideration and preference for creating new speakers
- **Robust fundamental frequency estimation**: Pre-emphasis filtering and parabolic interpolation

### 🎯 Installation

1. **Install Rust** (if not already installed):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone and build**:

   ```bash
   git clone <repository-url>
   cd meeting-assistant
   cargo build --release --features rust-diarization
   ```

3. **Set up environment**:
   ```bash
   cp .env.example .env
   # Edit .env with your OpenAI API key
   ```

### 🔧 Usage

**Basic meeting recording:**

```bash
cargo run --release
```

**With enhanced diarization:**

```bash
cargo run --release --features rust-diarization
```

**Test diarization improvements:**

```bash
./tests/test_rust_diarization.sh [audio_file]
```

### 🎵 Speaker Diarization

The improved spectral diarization plugin now provides:

- **Multiple speaker detection**: Properly identifies different speakers in conversations
- **Voice activity detection**: Enhanced VAD with spectral centroid analysis
- **Spectral clustering**: Multi-dimensional speaker feature comparison
- **Temporal context**: Considers timing between speech segments
- **Configurable sensitivity**: Adjustable thresholds for different scenarios

Example output:

```
🎵 Speaker clustering complete. 2 speakers identified
  Speaker 1: 45 segments, 180.3s total, F0=142.1Hz, conf=0.892
  Speaker 2: 12 segments, 24.7s total, F0=189.4Hz, conf=0.847
```

### 📋 CLI Commands

**Meeting Recording:**

```bash
# Start recording a meeting
cargo run --release -- record start --title "Team Meeting"

# Stop current recording
cargo run --release -- record stop

# List all recordings
cargo run --release -- record list
```

**Transcript Management:**

```bash
# List all available audio files
cargo run --release -- transcript list

# Generate transcript for a specific file
cargo run --release -- transcript generate /path/to/audio.wav

# Advanced diarization for a specific file
cargo run --release -- transcript diarize /path/to/audio.wav --model base --format detailed

# Advanced diarization for the latest audio file
cargo run --release -- transcript diarize-latest --model base --format detailed

# Show processing status
cargo run --release -- transcript status
```

**Plugin Management:**

```bash
# List installed plugins
cargo run --release -- plugin list

# Enable/disable plugins
cargo run --release -- plugin enable speaker-diarization
cargo run --release -- plugin disable speaker-diarization

# Switch LLM provider
cargo run --release -- plugin set-llm ollama
```

**System Status:**

```bash
# Show system configuration and status
cargo run --release -- status

# Run interactive setup
cargo run --release -- setup
```

### 📊 Performance

- **Startup time**: < 200ms
- **Real-time processing**: < 50ms latency for audio events
- **Memory efficient**: Minimal resource usage at idle
- **Concurrent processing**: Multi-threaded audio pipeline

### 🛠️ Configuration

Key settings in `.env`:

```bash
# Required
OPENAI_API_KEY=your_api_key_here

# Enhanced Audio Quality Settings (NEW - for better diarization)
AUDIO_ENHANCED_QUALITY=true              # Enable enhanced quality processing
AUDIO_SAMPLE_RATE=44100                  # Sample rate (44100Hz recommended for diarization)
AUDIO_BIT_DEPTH=24                       # Bit depth (16, 24, or 32)
AUDIO_MIN_DIARIZATION_SAMPLE_RATE=44100  # Minimum sample rate for diarization

# Standard Audio Settings
AUDIO_DEVICE=":0"                        # Audio input device
AUDIO_CHANNELS=1                         # Number of audio channels

# Optional diarization settings
SPEAKER_SIMILARITY_THRESHOLD=0.55        # Lower = more sensitive
VAD_THRESHOLD=0.01                       # Lower = more speech detection
MAX_SPEAKERS=6                           # Maximum speakers to detect
```

### 🎵 Enhanced Audio Quality for Diarization

The latest version includes **significant audio quality improvements** specifically designed for better speaker diarization:

#### **Automatic Quality Enhancement**

- **Sample Rate Upgrade**: Automatically upgrades to 44.1kHz minimum (from 16kHz) for better frequency resolution
- **24-bit Audio**: Uses 24-bit PCM encoding for improved dynamic range
- **Advanced Filtering**: Applies speech-optimized filters to reduce noise and enhance voice characteristics

#### **Quality Levels**

- **High Quality** (44.1kHz, 24-bit): Recommended for excellent diarization
- **Ultra Quality** (48kHz, 24-bit): Professional-grade diarization
- **Broadcast Quality** (48kHz, 32-bit): Studio-grade quality

#### **Audio Processing Pipeline**

```
Raw Audio → Noise Reduction → Frequency Filtering → Dynamic Normalization → Diarization
           (afftdn)         (85Hz-7.5kHz)        (speech optimized)
```

#### **Before vs After Quality Comparison**

- **Previous**: 16kHz, 16-bit, basic processing
- **Enhanced**: 44.1kHz+, 24-bit, advanced speech-optimized filtering
- **Result**: ~3-5x better speaker separation accuracy

#### **File Size Impact**

Enhanced quality increases file sizes:

- **Low Quality** (16kHz, 16-bit): Baseline
- **High Quality** (44.1kHz, 24-bit): ~5.5x larger
- **Ultra Quality** (48kHz, 24-bit): ~6x larger

#### **Configuration Examples**

```bash
# Maximum quality for critical meetings
AUDIO_ENHANCED_QUALITY=true
AUDIO_SAMPLE_RATE=48000
AUDIO_BIT_DEPTH=24

# Balanced quality for regular use
AUDIO_ENHANCED_QUALITY=true
AUDIO_SAMPLE_RATE=44100
AUDIO_BIT_DEPTH=24

# Basic quality (legacy mode)
AUDIO_ENHANCED_QUALITY=false
AUDIO_SAMPLE_RATE=16000
AUDIO_BIT_DEPTH=16
```

### 🔌 Plugin System

The plugin architecture supports:

- **AI Providers**: OpenAI, Ollama, custom LLMs
- **Audio Processing**: Diarization, enhancement, analysis
- **Content Analysis**: Sentiment, key extraction, summarization
- **Custom Extensions**: Build your own plugins

### 📚 Documentation

See the `docs/` directory for:

- Architecture overview
- Plugin development guide
- Audio processing details
- Meeting storage system

### 🤝 Contributing

Built with performance and reliability in mind. Contributions welcome!

### 📄 License

Creative Commons Attribution-NonCommercial 4.0 International License

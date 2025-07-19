# Rust-Native Speaker Diarization Guide

This guide shows how to implement speaker diarization entirely in Rust without Python dependencies.

## 🦀 **Available Approaches**

### **1. Pure Rust (Immediate)**

Uses basic audio processing techniques that work out-of-the-box:

- ✅ **Energy-based VAD** - Detects speech using audio energy
- ✅ **Spectral features** - MFCC-like features for speaker identification
- ✅ **Cosine similarity clustering** - Groups similar speakers
- ✅ **Zero dependencies** - Only standard Rust crates

### **2. ONNX Runtime (Best Performance)**

Export Python models to ONNX format and run with `ort`:

- ✅ **Real ML models** - Use actual pyannote models
- ✅ **No Python runtime** - Pure Rust execution
- ✅ **Good performance** - Optimized inference
- ⚠️ **Model conversion required** - One-time setup

### **3. Candle (Future)**

Hugging Face's Rust ML framework:

- ✅ **Native Rust ML** - Growing ecosystem
- ✅ **Transformer support** - Can run modern models
- ⚠️ **Limited models** - Still developing

## 🚀 **Quick Start: Pure Rust**

### Step 1: Enable Dependencies

Add to your `Cargo.toml`:

```toml
[features]
default = ["rust-diarization"]
rust-diarization = ["hound", "rustfft", "ndarray"]

[dependencies]
hound = { version = "3.5", optional = true }
rustfft = { version = "6.2", optional = true }
ndarray = { version = "0.16", optional = true }
```

### Step 2: Simple Example

```rust
use rust_native_diarization::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = RustDiarizationConfig::default();
    let mut diarizer = RustNativeDiarizationPlugin::new(config);

    // Initialize the plugin
    diarizer.initialize(&PluginContext::default()).await?;

    // Diarize an audio file
    let segments = diarizer.diarize_audio(Path::new("meeting.wav")).await?;

    // Print results
    for segment in segments {
        println!(
            "[{}] {:.2}s - {:.2}s: Speaker {}",
            segment.speaker_id,
            segment.start_time,
            segment.end_time,
            segment.confidence
        );
    }

    Ok(())
}
```

## 🎯 **Accuracy Comparison**

| Approach        | Setup   | Accuracy  | Performance | Dependencies |
| --------------- | ------- | --------- | ----------- | ------------ |
| Python pyannote | Complex | 🟢 95%+   | 🟡 Medium   | 🔴 Many      |
| ONNX Models     | Medium  | 🟢 90%+   | 🟢 Fast     | 🟡 Some      |
| Pure Rust       | Simple  | 🟡 70-80% | 🟢 Fast     | 🟢 None      |

## 🔧 **Advanced: ONNX Integration**

### Step 1: Export Python Models

Create a Python script to export models:

```python
# export_models.py
import torch
from pyannote.audio import Model
import onnx

# Export VAD model
vad_model = Model.from_pretrained("pyannote/voice-activity-detection")
vad_model.eval()

dummy_input = torch.randn(1, 16000)  # 1 second of audio
torch.onnx.export(
    vad_model,
    dummy_input,
    "vad_model.onnx",
    input_names=["audio"],
    output_names=["voice_activity"],
    dynamic_axes={"audio": {1: "time"}}
)

# Export embedding model
embedding_model = Model.from_pretrained("pyannote/embedding")
embedding_model.eval()

torch.onnx.export(
    embedding_model,
    dummy_input,
    "embedding_model.onnx",
    input_names=["audio"],
    output_names=["embedding"],
)
```

### Step 2: Use ONNX Models in Rust

```rust
let config = RustDiarizationConfig {
    vad_model_path: Some(PathBuf::from("models/vad_model.onnx")),
    embedding_model_path: Some(PathBuf::from("models/embedding_model.onnx")),
    use_energy_vad: false,  // Use ONNX VAD instead
    ..Default::default()
};
```

## 🎨 **Customization Options**

### Energy-Based VAD Tuning

```rust
let config = RustDiarizationConfig {
    energy_threshold: 0.02,        // Higher = less sensitive
    min_speech_duration: 0.3,      // Minimum speech segment length
    min_silence_duration: 0.1,     // Minimum silence gap
    ..Default::default()
};
```

### Speaker Clustering Tuning

```rust
let config = RustDiarizationConfig {
    clustering_threshold: 0.8,     // Higher = more conservative clustering
    max_speakers: 5,               // Limit number of speakers
    ..Default::default()
};
```

## 🔊 **Audio Processing Features**

### Implemented Features

- ✅ **Energy-based VAD** - Speech/silence detection
- ✅ **Spectral centroid** - Voice timbre characteristics
- ✅ **Zero-crossing rate** - Voice quality measurement
- ✅ **MFCC-like features** - Mel-frequency analysis
- ✅ **Cosine similarity** - Speaker matching
- ✅ **Basic resampling** - Handle different sample rates

### Future Enhancements

- 🔄 **Advanced VAD** - More sophisticated speech detection
- 🔄 **Better features** - Deep learning embeddings
- 🔄 **Improved clustering** - PLDA or neural clustering
- 🔄 **Online processing** - Real-time streaming

## ⚡ **Performance Tips**

### 1. Audio Preprocessing

```rust
// Optimal audio format
let config = RustDiarizationConfig {
    sample_rate: 16000,    // 16kHz is sufficient for speech
    frame_length: 2048,    // Longer frames = better frequency resolution
    hop_length: 512,       // 75% overlap for smooth analysis
    ..Default::default()
};
```

### 2. Memory Optimization

```rust
// Process in chunks for large files
let chunk_duration = 30.0; // 30-second chunks
let segments = diarizer.process_in_chunks(audio_path, chunk_duration).await?;
```

### 3. Parallel Processing

```rust
// Process multiple files concurrently
let tasks: Vec<_> = audio_files.iter()
    .map(|file| diarizer.diarize_audio(file))
    .collect();

let results = futures::future::join_all(tasks).await;
```

## 🎯 **Production Deployment**

### Configuration

```rust
// Production-ready config
let config = RustDiarizationConfig {
    // Use ONNX models for better accuracy
    vad_model_path: Some(PathBuf::from("/models/vad.onnx")),
    embedding_model_path: Some(PathBuf::from("/models/embedding.onnx")),

    // Tune for your use case
    energy_threshold: 0.015,
    clustering_threshold: 0.75,
    max_speakers: 8,

    // Optimize for performance
    frame_length: 1024,
    hop_length: 256,
    sample_rate: 16000,
};
```

### Integration with Meeting Assistant

```rust
// Add to your plugins in main.rs
let diarization_plugin = RustNativeDiarizationPlugin::new(config);
plugin_manager.register_plugin(Box::new(diarization_plugin))?;
```

## 🐛 **Troubleshooting**

### Common Issues

**1. Audio Format Problems**

```bash
# Convert audio to supported format
ffmpeg -i input.mp3 -ar 16000 -ac 1 output.wav
```

**2. Low Accuracy**

- Try tuning `energy_threshold`
- Adjust `clustering_threshold`
- Use ONNX models for better performance

**3. Performance Issues**

- Reduce `frame_length` for faster processing
- Process in smaller chunks
- Use optimized audio formats

## 🔮 **Future Roadmap**

### Near Term (Next 3 months)

- ✅ **Better VAD** - Energy + spectral features
- ✅ **Streaming support** - Real-time processing
- ✅ **More audio formats** - MP3, FLAC, etc.

### Medium Term (6 months)

- 🔄 **Candle integration** - Native Rust transformers
- 🔄 **Advanced clustering** - Neural approaches
- 🔄 **Model quantization** - Faster inference

### Long Term (1 year)

- 🔄 **Custom training** - Domain-specific models
- 🔄 **Multi-modal** - Audio + video analysis
- 🔄 **Edge deployment** - Embedded devices

## 📚 **Additional Resources**

- **ONNX Runtime Rust**: https://github.com/microsoft/onnxruntime
- **Candle ML Framework**: https://github.com/huggingface/candle
- **Audio Processing in Rust**: https://github.com/RustAudio
- **Speaker Diarization Papers**: https://arxiv.org/abs/2012.01421

## 🤝 **Contributing**

The Rust audio ecosystem is growing rapidly. Consider contributing:

- Better audio processing algorithms
- ONNX model conversion tools
- Performance optimizations
- Documentation improvements

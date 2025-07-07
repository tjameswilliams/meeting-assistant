/*
 * Meeting Assistant CLI - Rust Edition
 * Copyright (c) 2024 Meeting Assistant Contributors
 * 
 * This work is licensed under the Creative Commons Attribution-NonCommercial 4.0 International License.
 * To view a copy of this license, visit http://creativecommons.org/licenses/by-nc/4.0/
 * 
 * You are free to share and adapt this work for non-commercial purposes with attribution.
 * Commercial use is prohibited without explicit written permission.
 * 
 * For commercial licensing inquiries, please contact the project maintainers.
 */

use anyhow::{Result, Context};
use std::path::PathBuf;
use std::fs;
use std::io::{self, Write};
use std::env;
use colored::Colorize;
use tokio::time::{sleep, Duration};
use tokio::process::Command;
use crate::plugin_system::PluginSource;

pub struct SetupManager {
    os: String,
    temp_dir: PathBuf,
}

impl SetupManager {
    pub fn new() -> Self {
        let os = if cfg!(target_os = "macos") {
            "macos".to_string()
        } else if cfg!(target_os = "linux") {
            "linux".to_string()
        } else if cfg!(target_os = "windows") {
            "windows".to_string()
        } else {
            "unknown".to_string()
        };
        
        let temp_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".interview-assistant")
            .join("setup");
        
        Self { os, temp_dir }
    }
    
    pub async fn run_interactive_setup(&self) -> Result<()> {
        println!("{}", "🎤 Interview Assistant CLI - Interactive Setup".cyan().bold());
        println!("{}", "==============================================".cyan());
        println!();
        
        // Create temp directory
        fs::create_dir_all(&self.temp_dir)
            .context("Failed to create setup directory")?;
        
        // Welcome message
        self.print_welcome().await?;
        
        // System detection
        self.detect_system().await?;
        
        // Check existing setup
        let setup_status = self.check_existing_setup().await?;
        
        // Dependency installation
        if self.should_install_dependencies(&setup_status).await? {
            self.install_dependencies().await?;
        }
        
        // Audio configuration
        self.configure_audio_setup().await?;
        
        // Whisper setup
        self.setup_whisper_backends().await?;
        
        // Plugin system setup
        self.setup_plugin_system().await?;
        
        // Configuration file
        self.create_configuration().await?;
        
        // Permissions (macOS)
        if self.os == "macos" {
            self.setup_macos_permissions().await?;
        }
        
        // Build and test
        self.build_and_test().await?;
        
        // Final instructions
        self.show_final_instructions().await?;
        
        Ok(())
    }
    
    async fn print_welcome(&self) -> Result<()> {
        println!("{}", "Welcome to the Interview Assistant setup!".green().bold());
        println!();
        println!("This interactive setup will guide you through:");
        println!("• Installing required dependencies");
        println!("• Configuring audio capture");
        println!("• Setting up Whisper for transcription");
        println!("• Creating configuration files");
        println!("• Testing your setup");
        println!();
        
        self.wait_for_enter("Press Enter to continue...").await?;
        Ok(())
    }
    
    async fn detect_system(&self) -> Result<()> {
        println!("{}", "🔍 Detecting System Configuration".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        println!("Operating System: {}", self.os.green());
        
        // Check architecture
        let arch = std::env::consts::ARCH;
        println!("Architecture: {}", arch.green());
        
        // Check shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());
        println!("Shell: {}", shell.green());
        
        // Check terminal
        let term = std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string());
        println!("Terminal: {}", term.green());
        
        println!();
        Ok(())
    }
    
    async fn check_existing_setup(&self) -> Result<SetupStatus> {
        println!("{}", "🔍 Checking Existing Setup".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        let mut status = SetupStatus::default();
        
        // Check Rust
        status.rust_installed = self.check_command("cargo").await;
        self.print_check_result("Rust/Cargo", status.rust_installed);
        
        // Check FFmpeg
        status.ffmpeg_installed = self.check_command("ffmpeg").await;
        self.print_check_result("FFmpeg", status.ffmpeg_installed);
        
        // Check Whisper backends
        status.whisper_cpp = self.check_command("whisper-cpp").await || self.check_command("whisper-cli").await;
        status.whisper_brew = self.check_command("whisper").await;
        status.whisper_python = self.check_command("faster-whisper").await;
        
        let whisper_count = [status.whisper_cpp, status.whisper_brew, status.whisper_python]
            .iter()
            .filter(|&x| *x)
            .count();
        
        println!("Whisper backends: {}", if whisper_count > 0 {
            format!("{} available", whisper_count).green()
        } else {
            "None found".yellow()
        });
        
        // Check configuration
        status.config_exists = PathBuf::from(".env").exists();
        self.print_check_result("Configuration (.env)", status.config_exists);
        
        // Check build
        status.app_built = PathBuf::from("target/release/interview-assistant").exists();
        self.print_check_result("Application built", status.app_built);
        
        // Check plugin system
        status.plugin_system_configured = self.check_plugin_system().await;
        self.print_check_result("Plugin system configured", status.plugin_system_configured);
        
        // Check Ollama
        status.ollama_installed = self.check_command("ollama").await;
        if status.ollama_installed {
            status.ollama_running = self.check_ollama_running().await;
            status.ollama_models = self.get_ollama_models().await;
        }
        self.print_check_result("Ollama installed", status.ollama_installed);
        if status.ollama_installed {
            self.print_check_result("Ollama running", status.ollama_running);
            if !status.ollama_models.is_empty() {
                println!("Ollama models: {}", status.ollama_models.join(", ").green());
            }
        }
        
        // Check current LLM provider
        status.current_llm_provider = self.get_current_llm_provider().await;
        if let Some(provider) = &status.current_llm_provider {
            println!("Current LLM provider: {}", provider.green());
        }
        
        println!();
        Ok(status)
    }
    
    async fn should_install_dependencies(&self, status: &SetupStatus) -> Result<bool> {
        if status.rust_installed && status.ffmpeg_installed && 
           (status.whisper_cpp || status.whisper_brew || status.whisper_python) {
            println!("{}", "✅ All dependencies appear to be installed".green());
            return Ok(self.ask_yes_no("Would you like to reinstall/update dependencies?").await?);
        }
        
        let missing: Vec<&str> = vec![
            if !status.rust_installed { Some("Rust") } else { None },
            if !status.ffmpeg_installed { Some("FFmpeg") } else { None },
            if !status.whisper_cpp && !status.whisper_brew && !status.whisper_python { 
                Some("Whisper") 
            } else { 
                None 
            },
        ].into_iter().flatten().collect();
        
        if !missing.is_empty() {
            println!("{}", format!("Missing dependencies: {}", missing.join(", ")).yellow());
            return Ok(self.ask_yes_no("Would you like to install missing dependencies?").await?);
        }
        
        Ok(false)
    }
    
    async fn install_dependencies(&self) -> Result<()> {
        println!("{}", "📦 Installing Dependencies".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        match self.os.as_str() {
            "macos" => self.install_macos_dependencies().await?,
            "linux" => self.install_linux_dependencies().await?,
            "windows" => self.install_windows_dependencies().await?,
            _ => {
                println!("{}", "❌ Unsupported OS for automatic installation".red());
                return Err(anyhow::anyhow!("Unsupported OS"));
            }
        }
        
        Ok(())
    }
    
    async fn install_macos_dependencies(&self) -> Result<()> {
        // Install Homebrew if needed
        if !self.check_command("brew").await {
            println!("{}", "Installing Homebrew...".yellow());
            self.run_command_interactive(&[
                "/bin/bash",
                "-c",
                "\"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
            ]).await?;
        }
        
        // Install Rust if needed
        if !self.check_command("cargo").await {
            println!("{}", "Installing Rust...".yellow());
            self.run_command_interactive(&[
                "curl",
                "--proto", "=https",
                "--tlsv1.2",
                "-sSf",
                "https://sh.rustup.rs",
                "|", "sh", "-s", "--", "-y"
            ]).await?;
        }
        
        // Install FFmpeg
        if !self.check_command("ffmpeg").await {
            println!("{}", "Installing FFmpeg...".yellow());
            self.run_command_interactive(&["brew", "install", "ffmpeg"]).await?;
        }
        
        // Install audio tools
        println!("{}", "Installing audio tools...".yellow());
        self.run_command_interactive(&["brew", "install", "blackhole-2ch"]).await?;
        self.run_command_interactive(&["brew", "install", "sox"]).await?;
        
        // Install Whisper backends
        println!("{}", "Installing Whisper backends...".yellow());
        self.run_command_interactive(&["brew", "install", "whisper-cpp"]).await?;
        
        Ok(())
    }
    
    async fn install_linux_dependencies(&self) -> Result<()> {
        // Install Rust if needed
        if !self.check_command("cargo").await {
            println!("{}", "Installing Rust...".yellow());
            self.run_command_interactive(&[
                "curl",
                "--proto", "=https",
                "--tlsv1.2",
                "-sSf",
                "https://sh.rustup.rs",
                "|", "sh", "-s", "--", "-y"
            ]).await?;
        }
        
        // Try to detect package manager and install FFmpeg
        if self.check_command("apt-get").await {
            println!("{}", "Installing FFmpeg (apt)...".yellow());
            self.run_command_interactive(&["sudo", "apt-get", "update"]).await?;
            self.run_command_interactive(&["sudo", "apt-get", "install", "-y", "ffmpeg"]).await?;
        } else if self.check_command("yum").await {
            println!("{}", "Installing FFmpeg (yum)...".yellow());
            self.run_command_interactive(&["sudo", "yum", "install", "-y", "ffmpeg"]).await?;
        } else if self.check_command("pacman").await {
            println!("{}", "Installing FFmpeg (pacman)...".yellow());
            self.run_command_interactive(&["sudo", "pacman", "-S", "ffmpeg"]).await?;
        }
        
        // Install Python-based Whisper
        if self.check_command("pip").await {
            println!("{}", "Installing faster-whisper...".yellow());
            self.run_command_interactive(&["pip", "install", "faster-whisper"]).await?;
        }
        
        Ok(())
    }
    
    async fn install_windows_dependencies(&self) -> Result<()> {
        println!("{}", "❌ Windows automatic installation not yet supported".red());
        println!("Please install manually:");
        println!("1. Rust: https://rustup.rs/");
        println!("2. FFmpeg: https://ffmpeg.org/download.html");
        println!("3. Python Whisper: pip install openai-whisper");
        
        self.wait_for_enter("Press Enter when dependencies are installed...").await?;
        Ok(())
    }
    
    async fn configure_audio_setup(&self) -> Result<()> {
        println!("{}", "🎤 Audio Configuration".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        match self.os.as_str() {
            "macos" => self.configure_macos_audio().await?,
            "linux" => self.configure_linux_audio().await?,
            "windows" => self.configure_windows_audio().await?,
            _ => println!("{}", "❌ Unsupported OS for audio configuration".red()),
        }
        
        Ok(())
    }
    
    async fn configure_macos_audio(&self) -> Result<()> {
        println!("{}", "Detecting macOS audio devices...".yellow());
        
        // List audio devices
        let output = Command::new("ffmpeg")
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .await
            .context("Failed to list audio devices")?;
        
        let device_list = String::from_utf8_lossy(&output.stderr);
        println!("{}", "Available audio devices:".green());
        println!("{}", device_list);
        
        println!();
        println!("{}", "For system-wide audio capture, you'll need:".cyan());
        println!("1. BlackHole 2ch (virtual audio driver)");
        println!("2. An aggregate device combining your mic + BlackHole");
        println!("3. System audio routed through BlackHole");
        println!();
        
        if self.ask_yes_no("Would you like instructions for setting up system audio capture?").await? {
            self.show_macos_audio_instructions().await?;
        }
        
        Ok(())
    }
    
    async fn show_macos_audio_instructions(&self) -> Result<()> {
        println!("{}", "🔧 macOS Audio Setup Instructions".cyan().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        println!("{}", "Step 1: Install BlackHole".yellow().bold());
        println!("• BlackHole should already be installed via Homebrew");
        println!("• If not, run: brew install blackhole-2ch");
        println!();
        
        println!("{}", "Step 2: Create Aggregate Device".yellow().bold());
        println!("• Open 'Audio MIDI Setup' (Applications > Utilities)");
        println!("• Click '+' button → 'Create Aggregate Device'");
        println!("• Name it 'Interview Assistant Input'");
        println!("• Check boxes for:");
        println!("  - Your built-in microphone");
        println!("  - BlackHole 2ch");
        println!("• Set BlackHole as the master device");
        println!();
        
        println!("{}", "Step 3: Configure System Audio".yellow().bold());
        println!("• System Preferences → Sound → Output");
        println!("• Select 'BlackHole 2ch' as output device");
        println!("• System Preferences → Sound → Input");
        println!("• Select 'Interview Assistant Input' as input device");
        println!();
        
        println!("{}", "Step 4: Test Setup".yellow().bold());
        println!("• Play some audio/video");
        println!("• Speak into your microphone");
        println!("• Both should be captured by the aggregate device");
        println!();
        
        self.wait_for_enter("Press Enter when audio setup is complete...").await?;
        Ok(())
    }
    
    async fn configure_linux_audio(&self) -> Result<()> {
        println!("{}", "Linux audio configuration varies by distribution".yellow());
        println!("Common approaches:");
        println!("• PulseAudio: Use monitor devices for system audio");
        println!("• ALSA: Configure .asoundrc for capture");
        println!("• JACK: Set up routing for professional audio");
        println!();
        
        // List ALSA devices if available
        if self.check_command("arecord").await {
            println!("{}", "Available ALSA devices:".green());
            let _ = Command::new("arecord")
                .args(["-l"])
                .status()
                .await
                .context("Failed to list ALSA devices");
        }
        
        Ok(())
    }
    
    async fn configure_windows_audio(&self) -> Result<()> {
        println!("{}", "Windows audio configuration:".yellow());
        println!("• Use Windows Sound settings");
        println!("• Enable 'Stereo Mix' if available");
        println!("• Consider third-party tools like VB-Cable");
        Ok(())
    }
    
    async fn setup_whisper_backends(&self) -> Result<()> {
        println!("{}", "🗣️ Whisper Backend Setup".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Check which backends are available
        let backends = self.detect_whisper_backends().await;
        
        if backends.is_empty() {
            println!("{}", "❌ No Whisper backends found".red());
            println!("The application will use OpenAI API for transcription (slower)");
        } else {
            println!("{}", format!("✅ Found {} Whisper backend(s)", backends.len()).green());
            for backend in &backends {
                println!("  • {}", backend.green());
            }
        }
        
        // Download models for whisper.cpp if available
        if backends.iter().any(|b| b.contains("whisper.cpp")) {
            if self.ask_yes_no("Download whisper.cpp models for faster transcription?").await? {
                self.download_whisper_models().await?;
            }
        }
        
        Ok(())
    }
    
    async fn detect_whisper_backends(&self) -> Vec<String> {
        let mut backends = Vec::new();
        
        if self.check_command("whisper-cpp").await || self.check_command("whisper-cli").await {
            backends.push("whisper.cpp (ultra-fast)".to_string());
        }
        
        if self.check_command("whisper").await {
            backends.push("whisper (fast)".to_string());
        }
        
        if self.check_command("faster-whisper").await {
            backends.push("faster-whisper (good)".to_string());
        }
        
        backends
    }
    
    async fn download_whisper_models(&self) -> Result<()> {
        println!("{}", "Downloading whisper.cpp models...".yellow());
        
        let model_dir = if self.os == "macos" {
            PathBuf::from("/opt/homebrew/share/whisper.cpp/models")
        } else {
            dirs::home_dir().unwrap().join(".whisper.cpp/models")
        };
        
        fs::create_dir_all(&model_dir)
            .context("Failed to create model directory")?;
        
        let model_file = model_dir.join("ggml-base.en.bin");
        
        if !model_file.exists() {
            println!("{}", "Downloading base.en model...".yellow());
            
            let output = Command::new("curl")
                .args([
                    "-L",
                    "-o",
                    &model_file.to_string_lossy(),
                    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
                ])
                .output()
                .await
                .context("Failed to download model")?;
            
            if output.status.success() {
                println!("{}", "✅ Model downloaded successfully".green());
            } else {
                println!("{}", "❌ Failed to download model".red());
            }
        } else {
            println!("{}", "✅ Model already exists".green());
        }
        
        Ok(())
    }
    
    async fn create_configuration(&self) -> Result<()> {
        println!("{}", "⚙️ Configuration Setup".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        let config_file = PathBuf::from(".env");
        
        if config_file.exists() {
            if self.ask_yes_no("Configuration file already exists. Overwrite?").await? {
                fs::copy(&config_file, ".env.backup")
                    .context("Failed to backup existing config")?;
                println!("{}", "✅ Backed up existing config to .env.backup".green());
            } else {
                println!("{}", "Keeping existing configuration".yellow());
                return Ok(());
            }
        }
        
        // Get OpenAI API key
        let api_key = self.get_openai_api_key().await?;
        
        // Get audio device
        let audio_device = self.get_audio_device().await?;
        
        // Get LLM provider configuration
        let llm_provider = self.get_current_llm_provider().await
            .unwrap_or_else(|| "openai".to_string());
        
        // Create config file
        let config_content = format!(
            r#"# Meeting Assistant CLI - Rust Edition Configuration

# Required - OpenAI API Key (if using OpenAI provider)
OPENAI_API_KEY={}

# LLM Provider Configuration
LLM_PROVIDER={}
LLM_FALLBACK_TO_OPENAI=true

# Optional - OpenAI Settings (when using OpenAI provider)
OPENAI_MODEL=gpt-4o-mini
OPENAI_MAX_TOKENS=1800
OPENAI_TEMPERATURE=0.5

# Optional - Ollama Settings (when using Ollama provider)
OLLAMA_BASE_URL=http://localhost:11434
OLLAMA_MODEL=llama2:7b
OLLAMA_TIMEOUT=30
OLLAMA_MAX_RETRIES=3
OLLAMA_AUTO_PULL=false

# Optional - Audio Settings
AUDIO_DEVICE="{}"
AUDIO_SAMPLE_RATE=16000
AUDIO_CHANNELS=1
BUFFER_DURATION=8
CAPTURE_DURATION=15

# Optional - Timing Configuration
DOUBLE_TAP_WINDOW_MS=500
DEBOUNCE_MS=50
MAX_RECORDING_TIME=30000

# Optional - Temporary Directory
# TEMP_DIR=$HOME/.meeting-assistant/temp
"#,
            api_key, llm_provider, audio_device
        );
        
        fs::write(&config_file, config_content)
            .context("Failed to write configuration file")?;
        
        println!("{}", "✅ Configuration file created successfully".green());
        Ok(())
    }
    
    async fn get_openai_api_key(&self) -> Result<String> {
        println!("{}", "OpenAI API Key Setup".cyan().bold());
        println!("You need an OpenAI API key to use the AI features.");
        println!("Get one from: https://platform.openai.com/api-keys");
        println!();
        
        loop {
            print!("Enter your OpenAI API key (or 'skip' to configure later): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() {
                println!("{}", "Please enter an API key or 'skip'".yellow());
                continue;
            }
            
            if input.to_lowercase() == "skip" {
                return Ok("your_openai_api_key_here".to_string());
            }
            
            if input.starts_with("sk-") {
                return Ok(input.to_string());
            }
            
            println!("{}", "Invalid API key format. Should start with 'sk-'".red());
        }
    }
    
    async fn get_audio_device(&self) -> Result<String> {
        println!("{}", "Audio Device Configuration".cyan().bold());
        println!("Default audio device is ':0' (usually built-in microphone)");
        println!();
        
        print!("Enter audio device index (or press Enter for default ':0'): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            Ok(":0".to_string())
        } else {
            Ok(input.to_string())
        }
    }
    
    async fn setup_macos_permissions(&self) -> Result<()> {
        println!("{}", "🔐 macOS Permissions Setup".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        println!("{}", "The application requires several permissions to function:".cyan());
        println!();
        
        println!("{}", "1. Accessibility Access (for global hotkeys)".yellow().bold());
        println!("   • System Preferences → Security & Privacy → Privacy → Accessibility");
        println!("   • Add your terminal app (Terminal, iTerm2, etc.)");
        println!();
        
        println!("{}", "2. Microphone Access (for audio capture)".yellow().bold());
        println!("   • System Preferences → Security & Privacy → Privacy → Microphone");
        println!("   • Add your terminal app");
        println!();
        
        println!("{}", "3. Screen Recording (for screenshot feature)".yellow().bold());
        println!("   • System Preferences → Security & Privacy → Privacy → Screen Recording");
        println!("   • Add your terminal app");
        println!();
        
        println!("{}", "⚠️ Important: Restart your terminal app after granting permissions!".red().bold());
        println!();
        
        self.wait_for_enter("Press Enter when permissions are configured...").await?;
        Ok(())
    }
    
    async fn build_and_test(&self) -> Result<()> {
        println!("{}", "🔨 Building and Testing".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Build the application
        println!("{}", "Building application...".yellow());
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .output()
            .await
            .context("Failed to run cargo build")?;
        
        if output.status.success() {
            println!("{}", "✅ Build successful!".green());
            
            // Show binary info
            let binary_path = PathBuf::from("target/release/interview-assistant");
            if binary_path.exists() {
                let metadata = fs::metadata(&binary_path)?;
                let size = metadata.len();
                println!("{}", format!("Binary size: {:.1} MB", size as f64 / 1024.0 / 1024.0).green());
            }
        } else {
            println!("{}", "❌ Build failed!".red());
            let error = String::from_utf8_lossy(&output.stderr);
            println!("{}", error);
            return Err(anyhow::anyhow!("Build failed"));
        }
        
        // Test audio capture if possible
        if self.os == "macos" {
            if self.ask_yes_no("Test audio capture?").await? {
                self.test_audio_capture().await?;
            }
        }
        
        Ok(())
    }
    
    async fn test_audio_capture(&self) -> Result<()> {
        println!("{}", "Testing audio capture...".yellow());
        
        let test_file = self.temp_dir.join("test_audio.wav");
        
        // Use audio-only format for testing (none:0 for audio device 0)
        let output = Command::new("ffmpeg")
            .args([
                "-f", "avfoundation",
                "-i", "none:0",  // Audio-only from device 0
                "-t", "3",
                "-y",
                &test_file.to_string_lossy()
            ])
            .output()
            .await
            .context("Failed to test audio capture")?;
        
        if output.status.success() && test_file.exists() {
            println!("{}", "✅ Audio capture test successful!".green());
            let _ = fs::remove_file(&test_file);
        } else {
            println!("{}", "❌ Audio capture test failed".red());
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("Input/output error") {
                println!("This may be due to missing microphone permissions.");
                println!("Grant microphone access to your terminal in System Preferences.");
            } else if !error_msg.trim().is_empty() {
                println!("Error: {}", error_msg);
            }
            println!("This may be due to missing permissions or incorrect device configuration");
        }
        
        Ok(())
    }
    
    async fn show_final_instructions(&self) -> Result<()> {
        println!("{}", "🎉 Setup Complete!".green().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        
        println!("{}", "🚀 How to run:".cyan().bold());
        println!("   ./target/release/interview-assistant");
        println!("   or use: ./start.sh");
        println!();
        
        println!("{}", "🎮 Global hotkeys (double-tap quickly):".cyan().bold());
        println!("   • Double-tap 'A' - Capture recent audio");
        println!("   • Double-tap 'S' - Analyze clipboard code");
        println!("   • Double-tap 'Q' - Combined audio + clipboard");
        println!("   • Double-tap 'W' - Screenshot + audio analysis");
        println!("   • Double-tap 'R' - Cancel current request");
        println!("   • Double-tap 'H' - Show session history");
        println!("   • Ctrl+C - Exit");
        println!();
        
        println!("{}", "🔧 Configuration:".cyan().bold());
        println!("   • Edit .env file to customize settings");
        println!("   • Check README.md for detailed documentation");
        println!();
        
        println!("{}", "🔌 Plugin System:".cyan().bold());
        println!("   • Use plugin commands: ./target/release/meeting-assistant plugin <command>");
        println!("   • Switch LLM providers: ./target/release/meeting-assistant plugin set-llm <provider>");
        println!("   • List available plugins: ./target/release/meeting-assistant plugin list");
        println!("   • Install external plugins: ./target/release/meeting-assistant plugin install <source>");
        println!();
        
        println!("{}", "🦙 Ollama Usage:".cyan().bold());
        if self.check_command("ollama").await {
            println!("   • Ollama is installed and ready to use");
            println!("   • Start Ollama service: ollama serve");
            println!("   • Pull models: ollama pull <model-name>");
            println!("   • Set as default: LLM_PROVIDER=ollama in .env");
        } else {
            println!("   • Ollama not installed - install with: ./target/release/meeting-assistant setup");
        }
        println!();
        
        println!("{}", "🆘 Troubleshooting:".cyan().bold());
        println!("   • If hotkeys don't work: Check accessibility permissions");
        println!("   • If audio fails: Verify device with 'ffmpeg -f avfoundation -list_devices true -i \"\"'");
        println!("   • If Ollama fails: Check service with 'ollama list'");
        println!("   • Check logs in ~/.meeting-assistant/logs/");
        println!();
        
        println!("{}", "Ready to start your meeting assistance! 🎤".green().bold());
        Ok(())
    }
    
    // Helper methods
    async fn check_command(&self, command: &str) -> bool {
        Command::new("which")
            .arg(command)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    fn print_check_result(&self, name: &str, success: bool) {
        if success {
            println!("{}: {}", name, "✅".green());
        } else {
            println!("{}: {}", name, "❌".red());
        }
    }
    
    async fn ask_yes_no(&self, question: &str) -> Result<bool> {
        loop {
            print!("{} (y/n): ", question);
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            match input.trim().to_lowercase().as_str() {
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => println!("{}", "Please enter 'y' or 'n'".yellow()),
            }
        }
    }
    
    async fn wait_for_enter(&self, message: &str) -> Result<()> {
        print!("{}", message);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(())
    }
    
    async fn run_command_interactive(&self, args: &[&str]) -> Result<()> {
        let mut command = Command::new(args[0]);
        if args.len() > 1 {
            command.args(&args[1..]);
        }
        
        let status = command.status().await.context("Failed to execute command")?;
        
        if !status.success() {
            return Err(anyhow::anyhow!("Command failed: {}", args.join(" ")));
        }
        
        Ok(())
    }
    
    // Plugin system setup methods
    async fn setup_plugin_system(&self) -> Result<()> {
        println!("{}", "🔌 Plugin System Setup".blue().bold());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Check if plugin system is already configured
        if self.check_plugin_system().await {
            println!("{}", "✅ Plugin system already configured".green());
            if self.ask_yes_no("Would you like to reconfigure the plugin system?").await? {
                self.configure_plugin_system().await?;
            }
        } else {
            self.configure_plugin_system().await?;
        }
        
        Ok(())
    }
    
    async fn configure_plugin_system(&self) -> Result<()> {
        println!("{}", "Configuring plugin system...".yellow());
        
        // LLM Provider Selection
        let llm_provider = self.select_llm_provider().await?;
        
        // Set up selected provider
        match llm_provider.as_str() {
            "ollama" => {
                self.setup_ollama_provider().await?;
            }
            "openai" => {
                self.setup_openai_provider().await?;
            }
            _ => {
                println!("{}", "Custom provider selected. You'll need to configure it manually.".yellow());
            }
        }
        
        // Configure bundled plugins
        self.configure_bundled_plugins().await?;
        
        // Offer external plugin installation
        if self.ask_yes_no("Would you like to install external plugins?").await? {
            self.install_external_plugins().await?;
        }
        
        println!("{}", "✅ Plugin system configured successfully".green());
        Ok(())
    }
    
    async fn select_llm_provider(&self) -> Result<String> {
        println!("{}", "🤖 LLM Provider Selection".cyan().bold());
        println!("Choose your preferred LLM provider:");
        println!("1. OpenAI (GPT-4, GPT-3.5) - Cloud-based, high quality");
        println!("2. Ollama - Local, private, offline");
        println!("3. Custom - External plugin provider");
        println!();
        
        let current_provider = self.get_current_llm_provider().await;
        if let Some(provider) = &current_provider {
            println!("Current provider: {}", provider.green());
        }
        
        loop {
            print!("Enter your choice (1-3): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            match input.trim() {
                "1" => return Ok("openai".to_string()),
                "2" => return Ok("ollama".to_string()),
                "3" => {
                    print!("Enter custom provider name: ");
                    io::stdout().flush()?;
                    let mut provider_name = String::new();
                    io::stdin().read_line(&mut provider_name)?;
                    return Ok(provider_name.trim().to_string());
                }
                _ => println!("{}", "Please enter 1, 2, or 3".yellow()),
            }
        }
    }
    
    async fn setup_ollama_provider(&self) -> Result<()> {
        println!("{}", "🦙 Setting up Ollama".cyan().bold());
        
        // Install Ollama if not present
        if !self.check_command("ollama").await {
            if self.ask_yes_no("Ollama is not installed. Install it now?").await? {
                self.install_ollama().await?;
            } else {
                println!("{}", "Ollama installation skipped. You'll need to install it manually.".yellow());
                return Ok(());
            }
        }
        
        // Start Ollama service
        if !self.check_ollama_running().await {
            println!("{}", "Starting Ollama service...".yellow());
            self.start_ollama().await?;
        }
        
        // Install recommended models
        let models = self.get_ollama_models().await;
        if models.is_empty() {
            println!("{}", "No Ollama models found. Installing recommended models...".yellow());
            self.install_ollama_models().await?;
        } else {
            println!("{}", format!("Found {} Ollama models", models.len()).green());
            for model in &models {
                println!("  • {}", model.green());
            }
            
            if self.ask_yes_no("Would you like to install additional models?").await? {
                self.install_ollama_models().await?;
            }
        }
        
        // Test Ollama connection
        if self.test_ollama_connection().await? {
            println!("{}", "✅ Ollama setup completed successfully".green());
        } else {
            println!("{}", "❌ Ollama setup completed but connection test failed".yellow());
        }
        
        Ok(())
    }
    
    async fn setup_openai_provider(&self) -> Result<()> {
        println!("{}", "🤖 Setting up OpenAI".cyan().bold());
        
        // Check if API key is already configured
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            if !api_key.is_empty() && api_key != "your_openai_api_key_here" {
                println!("{}", "✅ OpenAI API key already configured".green());
                return Ok(());
            }
        }
        
        // Get API key
        let api_key = self.get_openai_api_key().await?;
        
        // Test API key
        if self.test_openai_connection(&api_key).await? {
            println!("{}", "✅ OpenAI setup completed successfully".green());
        } else {
            println!("{}", "❌ OpenAI API key test failed".yellow());
        }
        
        Ok(())
    }
    
    async fn configure_bundled_plugins(&self) -> Result<()> {
        println!("{}", "🔌 Bundled Plugins Configuration".cyan().bold());
        println!("The following plugins are available by default:");
        println!("1. Ollama LLM Provider - Local AI inference");
        println!("2. Sentiment Analyzer - Emotional analysis of conversations");
        println!();
        
        if self.ask_yes_no("Enable all bundled plugins?").await? {
            println!("{}", "✅ All bundled plugins will be enabled".green());
        } else {
            // Individual plugin selection
            if self.ask_yes_no("Enable Ollama LLM Provider plugin?").await? {
                println!("{}", "✅ Ollama LLM Provider plugin enabled".green());
            }
            
            if self.ask_yes_no("Enable Sentiment Analyzer plugin?").await? {
                println!("{}", "✅ Sentiment Analyzer plugin enabled".green());
            }
        }
        
        Ok(())
    }
    
    async fn install_external_plugins(&self) -> Result<()> {
        println!("{}", "📦 External Plugin Installation".cyan().bold());
        println!("You can install plugins from:");
        println!("1. GitHub repositories (username/repo)");
        println!("2. Local directories");
        println!("3. HTTP URLs");
        println!("4. Git repositories");
        println!();
        
        loop {
            print!("Enter plugin source (or 'done' to finish): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() || input == "done" {
                break;
            }
            
            match self.install_plugin_from_source(input).await {
                Ok(_) => {
                    println!("{}", format!("✅ Plugin installed from {}", input).green());
                }
                Err(e) => {
                    println!("{}", format!("❌ Failed to install plugin from {}: {}", input, e).red());
                }
            }
        }
        
        Ok(())
    }
    
    async fn install_plugin_from_source(&self, source: &str) -> Result<()> {
        let plugin_source = self.parse_plugin_source(source)?;
        
        // This would use the plugin manager to install the plugin
        // For now, we'll just simulate the installation
        println!("{}", format!("Installing plugin from {}...", source).yellow());
        
        // In a real implementation, this would:
        // 1. Download/clone the plugin
        // 2. Validate the plugin
        // 3. Install dependencies
        // 4. Register the plugin
        
        match plugin_source {
            PluginSource::GitHub { owner, repo, branch } => {
                println!("{}", format!("  → GitHub: {}/{} ({})", owner, repo, branch.unwrap_or("main".to_string())).green());
            }
            PluginSource::Local { path } => {
                println!("{}", format!("  → Local: {}", path.display()).green());
            }
            PluginSource::Http { url } => {
                println!("{}", format!("  → HTTP: {}", url).green());
            }
            PluginSource::Git { url, branch } => {
                println!("{}", format!("  → Git: {} ({})", url, branch.unwrap_or("main".to_string())).green());
            }
        }
        
        Ok(())
    }
    
    fn parse_plugin_source(&self, source: &str) -> Result<PluginSource> {
        // Parse different source formats
        if source.starts_with("https://github.com/") || source.starts_with("github.com/") {
            // GitHub URL format
            let source = source.strip_prefix("https://").unwrap_or(source);
            let source = source.strip_prefix("github.com/").unwrap_or(source);
            
            let parts: Vec<&str> = source.split('/').collect();
            if parts.len() >= 2 {
                let owner = parts[0].to_string();
                let repo = parts[1].to_string();
                let branch = if parts.len() > 2 { Some(parts[2].to_string()) } else { None };
                
                return Ok(PluginSource::GitHub { owner, repo, branch });
            }
        } else if source.contains('/') && !source.contains("://") {
            // GitHub owner/repo format
            let parts: Vec<&str> = source.split('/').collect();
            if parts.len() >= 2 {
                let owner = parts[0].to_string();
                let repo = parts[1].to_string();
                let branch = if parts.len() > 2 { Some(parts[2].to_string()) } else { None };
                
                return Ok(PluginSource::GitHub { owner, repo, branch });
            }
        } else if source.starts_with("http://") || source.starts_with("https://") {
            if source.ends_with(".git") {
                // Git repository
                return Ok(PluginSource::Git { url: source.to_string(), branch: None });
            } else {
                // HTTP URL
                return Ok(PluginSource::Http { url: source.to_string() });
            }
        } else if PathBuf::from(source).exists() {
            // Local path
            return Ok(PluginSource::Local { path: PathBuf::from(source) });
        }
        
        Err(anyhow::anyhow!("Invalid plugin source format: {}", source))
    }
    
    // Helper methods for plugin system
    async fn check_plugin_system(&self) -> bool {
        // Check if plugin system is configured
        PathBuf::from("plugins").exists() || 
        env::var("LLM_PROVIDER").is_ok()
    }
    
    async fn check_ollama_running(&self) -> bool {
        if !self.check_command("ollama").await {
            return false;
        }
        
        Command::new("ollama")
            .args(["list"])
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    
    async fn get_ollama_models(&self) -> Vec<String> {
        if !self.check_ollama_running().await {
            return Vec::new();
        }
        
        let output = Command::new("ollama")
            .args(["list"])
            .output()
            .await;
        
        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .lines()
                .skip(1) // Skip header
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if !parts.is_empty() {
                        Some(parts[0].to_string())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }
    
    async fn get_current_llm_provider(&self) -> Option<String> {
        env::var("LLM_PROVIDER").ok().or_else(|| {
            // Check .env file
            if let Ok(contents) = fs::read_to_string(".env") {
                for line in contents.lines() {
                    if line.starts_with("LLM_PROVIDER=") {
                        return Some(line.split('=').nth(1)?.to_string());
                    }
                }
            }
            None
        })
    }
    
    async fn install_ollama(&self) -> Result<()> {
        println!("{}", "Installing Ollama...".yellow());
        
        match self.os.as_str() {
            "macos" => {
                // Try Homebrew first
                if self.check_command("brew").await {
                    self.run_command_interactive(&["brew", "install", "ollama"]).await?;
                } else {
                    // Use curl installer
                    self.run_command_interactive(&[
                        "curl", "-fsSL", "https://ollama.ai/install.sh", "|", "sh"
                    ]).await?;
                }
            }
            "linux" => {
                self.run_command_interactive(&[
                    "curl", "-fsSL", "https://ollama.ai/install.sh", "|", "sh"
                ]).await?;
            }
            _ => {
                println!("{}", "Please install Ollama manually from https://ollama.ai".yellow());
                return Ok(());
            }
        }
        
        println!("{}", "✅ Ollama installed successfully".green());
        Ok(())
    }
    
    async fn start_ollama(&self) -> Result<()> {
        println!("{}", "Starting Ollama service...".yellow());
        
        // Start Ollama in background
        let _child = Command::new("ollama")
            .args(["serve"])
            .spawn()
            .context("Failed to start Ollama service")?;
        
        // Wait a bit for service to start
        sleep(Duration::from_secs(3)).await;
        
        if self.check_ollama_running().await {
            println!("{}", "✅ Ollama service started successfully".green());
        } else {
            println!("{}", "❌ Failed to start Ollama service".red());
        }
        
        Ok(())
    }
    
    async fn install_ollama_models(&self) -> Result<()> {
        println!("{}", "📦 Installing Ollama Models".cyan().bold());
        
        let recommended_models = vec![
            ("llama2:7b", "General purpose, good balance of speed and quality"),
            ("codellama:7b", "Optimized for code analysis and generation"),
            ("mistral:7b", "Fast and efficient for conversation"),
            ("neural-chat:7b", "Optimized for chat and conversation"),
        ];
        
        println!("Recommended models for meeting assistance:");
        for (i, (model, description)) in recommended_models.iter().enumerate() {
            println!("{}. {} - {}", i + 1, model.green(), description);
        }
        println!();
        
        if self.ask_yes_no("Install all recommended models?").await? {
            for (model, _) in &recommended_models {
                self.install_ollama_model(model).await?;
            }
        } else {
            for (model, description) in &recommended_models {
                if self.ask_yes_no(&format!("Install {} ({})?", model, description)).await? {
                    self.install_ollama_model(model).await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn install_ollama_model(&self, model: &str) -> Result<()> {
        println!("{}", format!("Installing model {}...", model).yellow());
        
        let output = Command::new("ollama")
            .args(["pull", model])
            .output()
            .await
            .context("Failed to pull Ollama model")?;
        
        if output.status.success() {
            println!("{}", format!("✅ Model {} installed successfully", model).green());
        } else {
            println!("{}", format!("❌ Failed to install model {}", model).red());
        }
        
        Ok(())
    }
    
    async fn test_ollama_connection(&self) -> Result<bool> {
        println!("{}", "Testing Ollama connection...".yellow());
        
        let output = Command::new("ollama")
            .args(["run", "llama2:7b", "Hello, can you hear me?"])
            .output()
            .await
            .context("Failed to test Ollama connection")?;
        
        Ok(output.status.success())
    }
    
    async fn test_openai_connection(&self, api_key: &str) -> Result<bool> {
        println!("{}", "Testing OpenAI connection...".yellow());
        
        // Simple test using curl
        let output = Command::new("curl")
            .args([
                "-s",
                "-H", &format!("Authorization: Bearer {}", api_key),
                "-H", "Content-Type: application/json",
                "-d", r#"{"model":"gpt-3.5-turbo","messages":[{"role":"user","content":"Hello"}],"max_tokens":5}"#,
                "https://api.openai.com/v1/chat/completions"
            ])
            .output()
            .await
            .context("Failed to test OpenAI connection")?;
        
        let response = String::from_utf8_lossy(&output.stdout);
        Ok(response.contains("choices") && !response.contains("error"))
    }
}

#[derive(Default)]
struct SetupStatus {
    rust_installed: bool,
    ffmpeg_installed: bool,
    whisper_cpp: bool,
    whisper_brew: bool,
    whisper_python: bool,
    config_exists: bool,
    app_built: bool,
    ollama_installed: bool,
    ollama_running: bool,
    ollama_models: Vec<String>,
    plugin_system_configured: bool,
    current_llm_provider: Option<String>,
}

// CLI interface for setup
pub async fn run_setup() -> Result<()> {
    let setup = SetupManager::new();
    setup.run_interactive_setup().await
} 
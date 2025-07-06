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
use colored::Colorize;
use tokio::time::{sleep, Duration};
use tokio::process::Command;

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
        self.run_command_interactive(&["brew", "install", "whisper"]).await?;
        
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
        
        // Create config file
        let config_content = format!(
            r#"# Interview Assistant CLI - Rust Edition Configuration

# Required - OpenAI API Key
OPENAI_API_KEY={}

# Optional - OpenAI Settings
OPENAI_MODEL=gpt-4o-mini
OPENAI_MAX_TOKENS=1800
OPENAI_TEMPERATURE=0.5

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
# TEMP_DIR=$HOME/.interview-assistant/temp
"#,
            api_key, audio_device
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
        
        println!("{}", "🆘 Troubleshooting:".cyan().bold());
        println!("   • If hotkeys don't work: Check accessibility permissions");
        println!("   • If audio fails: Verify device with 'ffmpeg -f avfoundation -list_devices true -i \"\"'");
        println!("   • Check logs in ~/.interview-assistant/logs/");
        println!();
        
        println!("{}", "Ready to start your interview assistance! 🎤".green().bold());
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
}

// CLI interface for setup
pub async fn run_setup() -> Result<()> {
    let setup = SetupManager::new();
    setup.run_interactive_setup().await
} 
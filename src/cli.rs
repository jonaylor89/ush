use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "ush",
    about = "Ultrasonic Shell - communicate between devices using ultrasonic sound waves",
    long_about = "ush (ultrasonic shell) enables communication between devices using ultrasonic sound frequencies. Send text messages, files, or engage in real-time chat sessions using audio transmitted through speakers and microphones."
)]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[arg(long, global = true, help = "Custom sample rate (default: 44100)")]
    pub sample_rate: Option<u32>,

    #[arg(
        long,
        global = true,
        help = "Frequency for bit '0' in Hz (default: 18000)"
    )]
    pub freq_0: Option<f32>,

    #[arg(
        long,
        global = true,
        help = "Frequency for bit '1' in Hz (default: 20000)"
    )]
    pub freq_1: Option<f32>,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Send a text message via ultrasonic audio")]
    Send {
        #[arg(help = "The message to send")]
        message: String,

        #[arg(short, long, help = "Repeat the message N times")]
        repeat: Option<u32>,

        #[arg(long, help = "Save the encoded audio to a WAV file")]
        save_wav: Option<PathBuf>,

        #[arg(
            long,
            help = "Play encoded audio from a WAV file instead of generating"
        )]
        from_wav: Option<PathBuf>,
    },

    #[command(about = "Listen for incoming ultrasonic messages")]
    Listen {
        #[arg(short, long, help = "Maximum time to listen in seconds")]
        timeout: Option<u32>,

        #[arg(long, help = "Save received audio to a WAV file")]
        save_wav: Option<PathBuf>,

        #[arg(long, help = "Process audio from a WAV file instead of microphone")]
        from_wav: Option<PathBuf>,

        #[arg(long, help = "Apply noise filtering")]
        filter: bool,

        #[arg(long, help = "Signal detection threshold (0.0-1.0, default: 0.1)")]
        threshold: Option<f32>,
    },

    #[command(about = "Start interactive chat mode")]
    Chat {
        #[arg(short, long, help = "Your username for the chat")]
        username: Option<String>,

        #[arg(long, help = "Enable automatic acknowledgments")]
        ack: bool,

        #[arg(long, help = "Chat session timeout in minutes")]
        timeout: Option<u32>,
    },

    #[command(about = "Send a file via ultrasonic audio")]
    SendFile {
        #[arg(help = "Path to the file to send")]
        file: PathBuf,

        #[arg(short, long, help = "Chunk size in bytes (default: 64)")]
        chunk_size: Option<usize>,

        #[arg(short, long, help = "Delay between chunks in milliseconds")]
        delay: Option<u64>,
    },

    #[command(about = "Receive a file via ultrasonic audio")]
    ReceiveFile {
        #[arg(help = "Output path for the received file")]
        output: PathBuf,

        #[arg(short, long, help = "Maximum time to wait for file transfer")]
        timeout: Option<u32>,
    },

    #[command(about = "Test audio devices and signal quality")]
    Test {
        #[command(subcommand)]
        test_type: TestCommands,
    },

    #[command(about = "Debug mode - show live audio analysis")]
    Debug {
        #[arg(long, help = "Show frequency spectrum")]
        spectrum: bool,

        #[arg(long, help = "Show waveform")]
        waveform: bool,

        #[arg(long, help = "Update rate in Hz (default: 10)")]
        rate: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum TestCommands {
    #[command(about = "List available audio devices")]
    Devices,

    #[command(about = "Test round-trip communication")]
    Loopback {
        #[arg(help = "Test message")]
        message: Option<String>,
    },

    #[command(about = "Test signal generation")]
    Generate {
        #[arg(help = "Frequency to generate")]
        frequency: f32,

        #[arg(short, long, help = "Duration in seconds")]
        duration: Option<f32>,
    },

    #[command(about = "Measure background noise level")]
    Noise {
        #[arg(short, long, help = "Measurement duration in seconds")]
        duration: Option<f32>,
    },
}

#[derive(Debug, Clone)]
pub struct AudioSettings {
    pub sample_rate: u32,
    pub freq_0: f32,
    pub freq_1: f32,
    pub verbose: bool,
    pub quiet: bool,
}

impl AudioSettings {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            sample_rate: cli.sample_rate.unwrap_or(44100),
            freq_0: cli.freq_0.unwrap_or(18000.0),
            freq_1: cli.freq_1.unwrap_or(20000.0),
            verbose: cli.verbose,
            quiet: cli.quiet,
        }
    }
}

impl Cli {
    pub fn get_audio_settings(&self) -> AudioSettings {
        AudioSettings::from_cli(self)
    }
}

pub fn validate_frequency(freq: f32) -> Result<f32, String> {
    if !(100.0..=24000.0).contains(&freq) {
        Err(format!(
            "Frequency {} Hz is outside valid range (100-24000 Hz)",
            freq
        ))
    } else {
        Ok(freq)
    }
}

pub fn validate_sample_rate(rate: u32) -> Result<u32, String> {
    if !(8000..=192000).contains(&rate) {
        Err(format!(
            "Sample rate {} Hz is outside valid range (8000-192000 Hz)",
            rate
        ))
    } else {
        Ok(rate)
    }
}

pub fn validate_threshold(threshold: f32) -> Result<f32, String> {
    if !(0.0..=1.0).contains(&threshold) {
        Err(format!(
            "Threshold {} is outside valid range (0.0-1.0)",
            threshold
        ))
    } else {
        Ok(threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_validation() {
        assert!(validate_frequency(18000.0).is_ok());
        assert!(validate_frequency(50.0).is_err());
        assert!(validate_frequency(30000.0).is_err());
    }

    #[test]
    fn test_sample_rate_validation() {
        assert!(validate_sample_rate(44100).is_ok());
        assert!(validate_sample_rate(1000).is_err());
        assert!(validate_sample_rate(300000).is_err());
    }

    #[test]
    fn test_threshold_validation() {
        assert!(validate_threshold(0.1).is_ok());
        assert!(validate_threshold(1.0).is_ok());
        assert!(validate_threshold(0.0).is_ok());
        assert!(validate_threshold(-0.1).is_err());
        assert!(validate_threshold(1.1).is_err());
    }
}

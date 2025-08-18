use thiserror::Error;

#[derive(Error, Debug)]
pub enum UshError {
    #[error("Audio error: {0}")]
    Audio(#[from] cpal::StreamError),

    #[error("Audio device error: {0}")]
    AudioDevice(#[from] cpal::DevicesError),

    #[error("Audio format error: {0}")]
    AudioFormat(#[from] cpal::SupportedStreamConfigsError),

    #[error("Audio build error: {0}")]
    AudioBuild(#[from] cpal::BuildStreamError),

    #[error("Audio play error: {0}")]
    AudioPlay(#[from] cpal::PlayStreamError),

    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),

    #[error("Protocol error: {message}")]
    Protocol { message: String },

    #[error("Decoding error: {message}")]
    Decoding { message: String },

    #[error("Encoding error: {message}")]
    Encoding { message: String },

    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch { expected: u32, actual: u32 },

    #[error("Timeout waiting for signal")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid configuration: {message}")]
    Config { message: String },
}

pub type UshResult<T> = Result<T, UshError>;

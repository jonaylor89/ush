use clap::Parser;
use log::info;

use ush::cli::{Cli, Commands, validate_frequency, validate_sample_rate, validate_threshold};
use ush::{UshError, UshResult};

mod app;
use app::*;

#[tokio::main]
async fn main() -> UshResult<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.quiet {
        "error"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    // Validate CLI parameters
    let settings = cli.get_audio_settings();
    validate_frequency(settings.freq_0).map_err(|e| UshError::Config { message: e })?;
    validate_frequency(settings.freq_1).map_err(|e| UshError::Config { message: e })?;
    validate_sample_rate(settings.sample_rate).map_err(|e| UshError::Config { message: e })?;

    if settings.freq_0 >= settings.freq_1 {
        return Err(UshError::Config {
            message: format!(
                "freq_0 ({}) must be less than freq_1 ({})",
                settings.freq_0, settings.freq_1
            ),
        });
    }

    info!("Starting ush v0.1.0");
    info!(
        "Audio settings: {}Hz sample rate, freq_0={}Hz, freq_1={}Hz",
        settings.sample_rate, settings.freq_0, settings.freq_1
    );

    match &cli.command {
        Commands::Send {
            message,
            repeat,
            save_wav,
            from_wav,
        } => {
            let app = UshApp::new(settings)?;
            app.send_message(message, *repeat, save_wav.as_deref(), from_wav.as_deref())
                .await
        }
        Commands::Listen {
            timeout,
            save_wav,
            from_wav,
            filter,
            threshold,
        } => {
            let app = UshApp::new(settings)?;
            let threshold = threshold
                .map(validate_threshold)
                .transpose()
                .map_err(|e| UshError::Config { message: e })?
                .unwrap_or(0.1);
            app.listen_for_messages(
                *timeout,
                save_wav.as_deref(),
                from_wav.as_deref(),
                *filter,
                threshold,
            )
            .await
        }
        Commands::Chat {
            username,
            ack,
            timeout,
        } => {
            let app = UshApp::new(settings)?;
            app.start_chat_mode(username.as_deref(), *ack, *timeout)
                .await
        }
        Commands::SendFile {
            file,
            chunk_size,
            delay,
        } => {
            let app = UshApp::new(settings)?;
            app.send_file(file, *chunk_size, *delay).await
        }
        Commands::ReceiveFile { output, timeout } => {
            let app = UshApp::new(settings)?;
            app.receive_file(output, *timeout).await
        }
        Commands::Test { test_type } => {
            let app = UshApp::new(settings)?;
            app.run_test(test_type).await
        }
        Commands::Debug {
            spectrum,
            waveform,
            rate,
        } => {
            let app = UshApp::new(settings)?;
            app.debug_mode(*spectrum, *waveform, *rate).await
        }
    }
}

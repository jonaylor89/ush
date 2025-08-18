use cpal::traits::{DeviceTrait, HostTrait};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use log::{error, info, warn};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use cpal::traits::StreamTrait;
use ush::audio::{AudioConfig, AudioManager};
use ush::cli::{AudioSettings, TestCommands};
use ush::modulation::{
    FskDemodulator, FskModulator, ModulationConfig, apply_bandpass_filter, detect_signal_start,
};
use ush::protocol::{Message, MessageType, ProtocolDecoder, ProtocolEncoder};
use ush::{UshError, UshResult};

pub struct UshApp {
    audio_manager: AudioManager,
    modulator: FskModulator,
    demodulator: FskDemodulator,
    _encoder: ProtocolEncoder,
    _decoder: ProtocolDecoder,
    settings: AudioSettings,
}

impl UshApp {
    pub fn new(settings: AudioSettings) -> UshResult<Self> {
        let audio_config = AudioConfig {
            sample_rate: settings.sample_rate,
            channels: 1,
            buffer_size: 4096,
        };

        let modulation_config = ModulationConfig {
            sample_rate: settings.sample_rate,
            freq_0: settings.freq_0,
            freq_1: settings.freq_1,
            symbol_duration: 0.01,
            ramp_duration: 0.002,
        };

        let audio_manager = AudioManager::with_config(audio_config)?;
        let modulator = FskModulator::new(modulation_config.clone());
        let demodulator = FskDemodulator::new(modulation_config);
        let encoder = ProtocolEncoder::new();
        let decoder = ProtocolDecoder::new();

        Ok(Self {
            audio_manager,
            modulator,
            demodulator,
            _encoder: encoder,
            _decoder: decoder,
            settings,
        })
    }

    pub async fn send_message(
        &self,
        message: &str,
        repeat: Option<u32>,
        save_wav: Option<&Path>,
        from_wav: Option<&Path>,
    ) -> UshResult<()> {
        info!("Sending message: \"{}\"", message);

        let samples = if let Some(wav_path) = from_wav {
            info!("Loading audio from WAV file: {:?}", wav_path);
            self.load_wav_file(wav_path)?
        } else {
            let mut encoder = ProtocolEncoder::new();
            let frame_data = encoder.encode_text(message)?;
            let samples = self.modulator.encode_bytes(&frame_data);

            if let Some(wav_path) = save_wav {
                self.save_wav_file(&samples, wav_path)?;
                info!("Saved encoded audio to: {:?}", wav_path);
            }

            samples
        };

        let repeat_count = repeat.unwrap_or(1);
        let mut full_samples = Vec::new();

        for i in 0..repeat_count {
            if i > 0 {
                // Add silence between repeats
                let silence_duration = 0.5; // 500ms
                let silence_samples =
                    (self.settings.sample_rate as f32 * silence_duration) as usize;
                full_samples.extend(vec![0.0; silence_samples]);
            }
            full_samples.extend(&samples);
        }

        info!(
            "Playing {} samples ({:.2}s) {} time(s)",
            samples.len(),
            samples.len() as f32 / self.settings.sample_rate as f32,
            repeat_count
        );

        self.play_samples(&full_samples).await
    }

    pub async fn listen_for_messages(
        &self,
        timeout_secs: Option<u32>,
        save_wav: Option<&Path>,
        from_wav: Option<&Path>,
        filter: bool,
        threshold: f32,
    ) -> UshResult<()> {
        if let Some(wav_path) = from_wav {
            info!("Processing audio from WAV file: {:?}", wav_path);
            let samples = self.load_wav_file(wav_path)?;
            return self
                .process_received_samples(&samples, filter, threshold)
                .await;
        }

        info!("Listening for messages (threshold: {:.2})...", threshold);
        if let Some(timeout) = timeout_secs {
            info!("Timeout set to {} seconds", timeout);
        }

        let recorded_samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let samples_clone = recorded_samples.clone();

        let (_tx, _rx) = mpsc::unbounded_channel::<()>();

        let input_stream = self.audio_manager.create_input_stream(move |data| {
            let mut samples = samples_clone.lock().unwrap();
            samples.extend_from_slice(data);

            // Process in chunks to avoid memory issues
            if samples.len() > 44100 * 10 {
                // 10 seconds of audio
                samples.drain(..44100 * 5); // Keep last 5 seconds
            }
        })?;

        input_stream.play()?;

        let start_time = Instant::now();
        let _decoder = ProtocolDecoder::new();
        let mut last_process_time = Instant::now();

        loop {
            // Check timeout
            if let Some(timeout) = timeout_secs {
                if start_time.elapsed().as_secs() > timeout as u64 {
                    info!("Listen timeout reached");
                    break;
                }
            }

            // Process audio data periodically
            if last_process_time.elapsed() > Duration::from_millis(100) {
                let samples = {
                    let guard = recorded_samples.lock().unwrap();
                    guard.clone()
                };

                if !samples.is_empty() {
                    if let Some(start_idx) = detect_signal_start(&samples, threshold) {
                        info!("Signal detected at sample {}", start_idx);

                        let signal_samples = &samples[start_idx..];
                        if let Err(e) = self
                            .process_received_samples(signal_samples, filter, threshold)
                            .await
                        {
                            warn!("Failed to process signal: {}", e);
                        }

                        // Clear processed samples
                        recorded_samples.lock().unwrap().clear();
                    }
                }

                last_process_time = Instant::now();
            }

            // Check for Ctrl+C
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key_event)) = event::read() {
                    if key_event.kind == KeyEventKind::Press
                        && key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        info!("Interrupted by user");
                        break;
                    }
                }
            }

            sleep(Duration::from_millis(10)).await;
        }

        drop(input_stream);

        if let Some(wav_path) = save_wav {
            let samples = recorded_samples.lock().unwrap();
            if !samples.is_empty() {
                self.save_wav_file(&samples, wav_path)?;
                info!("Saved recorded audio to: {:?}", wav_path);
            }
        }

        Ok(())
    }

    async fn process_received_samples(
        &self,
        samples: &[f32],
        filter: bool,
        _threshold: f32,
    ) -> UshResult<()> {
        let processed_samples = if filter {
            info!("Applying bandpass filter");
            apply_bandpass_filter(
                samples,
                self.settings.freq_0 - 1000.0,
                self.settings.freq_1 + 1000.0,
                self.settings.sample_rate,
            )
        } else {
            samples.to_vec()
        };

        match self.demodulator.decode_bytes(&processed_samples) {
            Ok(frame_data) => {
                let mut decoder = ProtocolDecoder::new();
                let messages = decoder.feed_data(&frame_data);
                for message in messages {
                    self.handle_received_message(&message).await?;
                }
            }
            Err(e) => {
                warn!("Failed to decode audio: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_received_message(&self, message: &Message) -> UshResult<()> {
        match &message.header.message_type {
            MessageType::Text => {
                let text = message.get_text()?;
                println!("Received: {}", text);
            }
            MessageType::Ping => {
                println!(
                    "Received ping from sequence {}",
                    message.header.sequence_number
                );
            }
            MessageType::Ack => {
                println!(
                    "Received ACK for sequence {}",
                    message.header.sequence_number
                );
            }
            MessageType::File => {
                println!(
                    "Received file chunk (sequence {})",
                    message.header.sequence_number
                );
            }
        }

        Ok(())
    }

    pub async fn start_chat_mode(
        &self,
        username: Option<&str>,
        enable_ack: bool,
        timeout_mins: Option<u32>,
    ) -> UshResult<()> {
        let username = username.unwrap_or("user");
        info!("Starting chat mode as '{}' (ACK: {})", username, enable_ack);

        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;

        let result = self.run_chat_loop(username, enable_ack, timeout_mins).await;

        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        result
    }

    async fn run_chat_loop(
        &self,
        username: &str,
        _enable_ack: bool,
        timeout_mins: Option<u32>,
    ) -> UshResult<()> {
        println!("Chat Mode - Press Ctrl+C to exit");
        println!("Type your message and press Enter to send\n");

        let mut input_buffer = String::new();
        let mut message_history = VecDeque::new();
        let start_time = Instant::now();

        loop {
            // Check timeout
            if let Some(timeout) = timeout_mins {
                if start_time.elapsed().as_secs() > (timeout as u64 * 60) {
                    println!("\nChat timeout reached");
                    break;
                }
            }

            // Handle keyboard input
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        match key_event.code {
                            KeyCode::Char('c')
                                if key_event.modifiers.contains(event::KeyModifiers::CONTROL) =>
                            {
                                println!("\nExiting chat mode...");
                                break;
                            }
                            KeyCode::Enter => {
                                if !input_buffer.trim().is_empty() {
                                    let message = format!("{}: {}", username, input_buffer.trim());
                                    println!("Sending: {}", message);

                                    if let Err(e) =
                                        self.send_message(&message, None, None, None).await
                                    {
                                        println!("Failed to send message: {}", e);
                                    } else {
                                        message_history.push_back(message);
                                        if message_history.len() > 50 {
                                            message_history.pop_front();
                                        }
                                    }

                                    input_buffer.clear();
                                }
                            }
                            KeyCode::Backspace => {
                                input_buffer.pop();
                            }
                            KeyCode::Char(c) => {
                                input_buffer.push(c);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    pub async fn send_file(
        &self,
        file_path: &Path,
        chunk_size: Option<usize>,
        delay: Option<u64>,
    ) -> UshResult<()> {
        let chunk_size = chunk_size.unwrap_or(64);
        let delay_ms = delay.unwrap_or(500);

        info!(
            "Sending file: {:?} (chunk size: {} bytes, delay: {}ms)",
            file_path, chunk_size, delay_ms
        );

        let file = File::open(file_path)?;
        let file_size = file.metadata()?.len();
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0u8; chunk_size];
        let mut bytes_sent = 0u64;
        let mut sequence = 0u32;

        println!("Sending file: {} bytes", file_size);

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk = &buffer[..bytes_read];

            // Create file message
            let message = format!(
                "FILE:{}:{}:{}",
                file_path.file_name().unwrap().to_string_lossy(),
                sequence,
                base64::encode(chunk)
            );

            if let Err(e) = self.send_message(&message, Some(1), None, None).await {
                error!("Failed to send file chunk {}: {}", sequence, e);
                return Err(e);
            }

            bytes_sent += bytes_read as u64;
            sequence += 1;

            println!(
                "Sent chunk {} ({}/{} bytes, {:.1}%)",
                sequence - 1,
                bytes_sent,
                file_size,
                (bytes_sent as f64 / file_size as f64) * 100.0
            );

            if bytes_read < chunk_size {
                break;
            }

            sleep(Duration::from_millis(delay_ms)).await;
        }

        // Send end marker
        let end_message = format!(
            "FILE:{}:END",
            file_path.file_name().unwrap().to_string_lossy()
        );
        self.send_message(&end_message, Some(1), None, None).await?;

        println!(
            "File transfer complete: {} bytes in {} chunks",
            bytes_sent, sequence
        );
        Ok(())
    }

    pub async fn receive_file(
        &self,
        output_path: &Path,
        _timeout_secs: Option<u32>,
    ) -> UshResult<()> {
        info!("Receiving file to: {:?}", output_path);

        // This would be implemented similar to listen_for_messages but with file reassembly logic
        warn!("File receiving not yet implemented in this demo");
        Ok(())
    }

    pub async fn run_test(&self, test_type: &TestCommands) -> UshResult<()> {
        match test_type {
            TestCommands::Devices => self.list_audio_devices().await,
            TestCommands::Loopback { message } => {
                let test_message = message.as_deref().unwrap_or("Hello, World!");
                self.test_loopback(test_message).await
            }
            TestCommands::Generate {
                frequency,
                duration,
            } => {
                let dur = duration.unwrap_or(1.0);
                self.generate_tone(*frequency, dur).await
            }
            TestCommands::Noise { duration } => {
                let dur = duration.unwrap_or(5.0);
                self.measure_noise_level(dur).await
            }
        }
    }

    async fn list_audio_devices(&self) -> UshResult<()> {
        let host = self.audio_manager.get_host();

        println!("Audio Host: {}", host.id().name());
        println!("{}", "=".repeat(50));

        // List input devices
        println!("\nINPUT DEVICES:");
        println!("{}", "-".repeat(30));

        match host.input_devices() {
            Ok(devices) => {
                let mut device_count = 0;
                for (i, device) in devices.enumerate() {
                    device_count += 1;
                    println!(
                        "\nDevice {}: {}",
                        i,
                        device.name().unwrap_or("Unknown".to_string())
                    );

                    // Show supported input configurations
                    match device.supported_input_configs() {
                        Ok(configs) => {
                            for (j, config) in configs.enumerate() {
                                println!(
                                    "  Config {}: {} channels, {}-{} Hz, {:?}",
                                    j,
                                    config.channels(),
                                    config.min_sample_rate().0,
                                    config.max_sample_rate().0,
                                    config.sample_format()
                                );
                            }
                        }
                        Err(e) => println!("  Error getting configs: {}", e),
                    }
                }
                if device_count == 0 {
                    println!("No input devices found");
                }
            }
            Err(e) => println!("Error listing input devices: {}", e),
        }

        // List output devices
        println!("\nOUTPUT DEVICES:");
        println!("{}", "-".repeat(30));

        match host.output_devices() {
            Ok(devices) => {
                let mut device_count = 0;
                for (i, device) in devices.enumerate() {
                    device_count += 1;
                    println!(
                        "\nDevice {}: {}",
                        i,
                        device.name().unwrap_or("Unknown".to_string())
                    );

                    // Show supported output configurations
                    match device.supported_output_configs() {
                        Ok(configs) => {
                            for (j, config) in configs.enumerate() {
                                println!(
                                    "  Config {}: {} channels, {}-{} Hz, {:?}",
                                    j,
                                    config.channels(),
                                    config.min_sample_rate().0,
                                    config.max_sample_rate().0,
                                    config.sample_format()
                                );
                            }
                        }
                        Err(e) => println!("  Error getting configs: {}", e),
                    }
                }
                if device_count == 0 {
                    println!("No output devices found");
                }
            }
            Err(e) => println!("Error listing output devices: {}", e),
        }

        // Check default devices specifically
        println!("\nDEFAULT DEVICES:");
        println!("{}", "-".repeat(30));

        if let Some(device) = host.default_input_device() {
            println!(
                "Default input: {}",
                device.name().unwrap_or("Unknown".to_string())
            );
        } else {
            println!("No default input device found!");
        }

        if let Some(device) = host.default_output_device() {
            println!(
                "Default output: {}",
                device.name().unwrap_or("Unknown".to_string())
            );
        } else {
            println!("No default output device found!");
        }

        // Show current audio configuration
        println!("\nCURRENT AUDIO CONFIG:");
        println!("{}", "-".repeat(30));
        let config = self.audio_manager.get_config();
        println!("Sample rate: {} Hz", config.sample_rate);
        println!("Channels: {}", config.channels);
        println!("Buffer size: {} samples", config.buffer_size);

        Ok(())
    }

    async fn test_loopback(&self, message: &str) -> UshResult<()> {
        println!("Testing loopback with message: \"{}\"", message);

        // Encode message
        let mut encoder = ProtocolEncoder::new();
        let frame_data = encoder.encode_text(message)?;
        let samples = self.modulator.encode_bytes(&frame_data);

        // Decode message
        let decoded_bytes = self.demodulator.decode_bytes(&samples)?;
        let mut decoder = ProtocolDecoder::new();
        let messages = decoder.feed_data(&decoded_bytes);

        if let Some(decoded_message) = messages.first() {
            let decoded_text = decoded_message.get_text()?;
            println!("Original: \"{}\"", message);
            println!("Decoded:  \"{}\"", decoded_text);

            if decoded_text == message {
                println!("✓ Loopback test PASSED");
            } else {
                println!("✗ Loopback test FAILED");
            }
        } else {
            println!("✗ Loopback test FAILED - no message decoded");
        }

        Ok(())
    }

    async fn generate_tone(&self, frequency: f32, duration: f32) -> UshResult<()> {
        println!("Generating {}Hz tone for {:.1}s", frequency, duration);

        let samples_count = (self.settings.sample_rate as f32 * duration) as usize;
        let mut samples = Vec::with_capacity(samples_count);

        for i in 0..samples_count {
            let t = i as f32 / self.settings.sample_rate as f32;
            let phase = 2.0 * std::f32::consts::PI * frequency * t;
            samples.push(phase.sin() * 0.3); // 30% volume
        }

        self.play_samples(&samples).await
    }

    async fn measure_noise_level(&self, duration: f32) -> UshResult<()> {
        println!("Measuring background noise for {:.1}s...", duration);

        let recorded_samples = Arc::new(Mutex::new(Vec::<f32>::new()));
        let samples_clone = recorded_samples.clone();

        let input_stream = self.audio_manager.create_input_stream(move |data| {
            let mut samples = samples_clone.lock().unwrap();
            samples.extend_from_slice(data);
        })?;

        input_stream.play()?;
        sleep(Duration::from_secs_f32(duration)).await;
        drop(input_stream);

        let samples = recorded_samples.lock().unwrap();
        let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);

        println!("Noise measurement results:");
        println!("  RMS level: {:.6} ({:.1} dB)", rms, 20.0 * rms.log10());
        println!("  Peak level: {:.6} ({:.1} dB)", peak, 20.0 * peak.log10());
        println!("  Samples recorded: {}", samples.len());

        Ok(())
    }

    pub async fn debug_mode(
        &self,
        spectrum: bool,
        waveform: bool,
        rate: Option<u32>,
    ) -> UshResult<()> {
        let update_rate = rate.unwrap_or(10);
        println!(
            "Debug mode - spectrum: {}, waveform: {}, rate: {}Hz",
            spectrum, waveform, update_rate
        );
        println!("This feature requires additional implementation for real-time visualization.");

        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    async fn play_samples(&self, samples: &[f32]) -> UshResult<()> {
        let samples_arc = Arc::new(Mutex::new(samples.to_vec()));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let output_stream = self.audio_manager.create_output_stream(samples_arc, tx)?;
        output_stream.play()?;

        // Wait for playback to complete
        if (rx.recv().await).is_some() {
            info!("Playback completed");
        }

        drop(output_stream);
        sleep(Duration::from_millis(100)).await; // Small delay for cleanup

        Ok(())
    }

    fn load_wav_file(&self, path: &Path) -> UshResult<Vec<f32>> {
        let mut reader = hound::WavReader::open(path)?;
        let spec = reader.spec();

        info!(
            "Loading WAV: {}Hz, {} channels, {} bits",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        );

        let samples: UshResult<Vec<f32>> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .map(|s| s.map_err(UshError::from))
                .collect(),
            hound::SampleFormat::Int => match spec.bits_per_sample {
                16 => {
                    let samples: Result<Vec<i16>, _> = reader.samples().collect();
                    Ok(samples?
                        .into_iter()
                        .map(|s| s as f32 / i16::MAX as f32)
                        .collect())
                }
                32 => {
                    let samples: Result<Vec<i32>, _> = reader.samples().collect();
                    Ok(samples?
                        .into_iter()
                        .map(|s| s as f32 / i32::MAX as f32)
                        .collect())
                }
                _ => Err(UshError::Config {
                    message: format!("Unsupported bit depth: {}", spec.bits_per_sample),
                }),
            },
        };

        samples
    }

    fn save_wav_file(&self, samples: &[f32], path: &Path) -> UshResult<()> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.settings.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(path, spec)?;

        for &sample in samples {
            writer.write_sample(sample)?;
        }

        writer.finalize()?;
        Ok(())
    }
}

// Add base64 encoding for file transfer
mod base64 {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(input: &[u8]) -> String {
        let mut result = String::new();
        let mut i = 0;

        while i < input.len() {
            let b1 = input[i];
            let b2 = if i + 1 < input.len() { input[i + 1] } else { 0 };
            let b3 = if i + 2 < input.len() { input[i + 2] } else { 0 };

            let bitmap = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

            result.push(CHARS[((bitmap >> 18) & 63) as usize] as char);
            result.push(CHARS[((bitmap >> 12) & 63) as usize] as char);
            if i + 1 < input.len() {
                result.push(CHARS[((bitmap >> 6) & 63) as usize] as char);
            } else {
                result.push('=');
            }
            if i + 2 < input.len() {
                result.push(CHARS[(bitmap & 63) as usize] as char);
            } else {
                result.push('=');
            }

            i += 3;
        }

        result
    }
}

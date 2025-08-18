# ush - Ultrasonic Shell

**ush** (ultrasonic shell) is a terminal program that enables communication between devices using ultrasonic sound waves. Send text messages, engage in real-time chat, or transfer files using audio frequencies above human hearing range (18-22 kHz).

## Features

- **Text Messaging**: Send and receive text messages via ultrasonic audio
- **Interactive Chat**: Real-time chat mode for back-and-forth conversations
- **File Transfer**: Send files by breaking them into audio packets
- **Cross-Platform**: Works on macOS, Linux, and Windows
- **Robust Protocol**: Built-in error detection with CRC checksums
- **Noise Filtering**: Signal processing to handle noisy environments
- **Audio Recording**: Save/load transmissions as WAV files for debugging

## Quick Start

### Installation

```bash
cargo instal ush
```

### Basic Usage

**Send a message** (Device A):
```bash
ush send "Hello, World!"
```

**Listen for messages** (Device B):
```bash
ush listen
```

### Requirements

- **Audio devices**: Working microphone and speakers/headphones
- **Rust**: 1.70+ (2021 edition)
- **Operating System**: macOS, Linux, or Windows

## Usage Examples

### Basic Communication

Send a simple message:
```bash
ush send "Hello from device A!"
```

Listen with a 30-second timeout:
```bash
ush listen --timeout 30
```

Send message multiple times for reliability:
```bash
ush send "Important message" --repeat 3
```

### Interactive Chat Mode

Start a chat session:
```bash
ush chat --username alice
```

Chat with automatic acknowledgments:
```bash
ush chat --username bob --ack
```

### File Transfer

Send a small file:
```bash
ush send-file document.txt
```

Send with custom chunk size and delay:
```bash
ush send-file image.jpg --chunk-size 32 --delay 1000
```

Receive a file:
```bash
ush receive-file downloaded.txt --timeout 300
```

### Audio Debugging

Save transmitted audio for analysis:
```bash
ush send "Debug test" --save-wav output.wav
```

Process audio from a file instead of live:
```bash
ush listen --from-wav recorded.wav
```

### Testing and Diagnostics

Test the complete encoding/decoding pipeline:
```bash
ush test loopback "Test message"
```

List available audio devices:
```bash
ush test devices
```

Generate a test tone:
```bash
ush test generate 19000 --duration 2.0
```

Measure background noise:
```bash
ush test noise --duration 5
```

### Debug Mode and Analysis

Audio analysis with spectrograms and FFT visualizations:

```bash
# Debug analysis of live audio capture
ush listen --timeout 30 --debug --debug-output ./debug_analysis

# Debug analysis of existing WAV file
ush listen --from-wav recording.wav --debug --debug-output ./analysis

# Quick debug loopback test
just debug-loopback "Test message"

# Comprehensive debug demonstration
just debug-demo
```

Debug mode generates:
- **Spectrograms**: Time-frequency heatmaps showing FSK signal patterns
- **FFT Analysis**: Frequency domain plots with signal quality metrics
- **Signal Metrics**: SNR estimation, dynamic range, frequency presence
- **HTML Reports**: Interactive analysis with interpretation guides
- **Raw Audio**: Complete recordings for external analysis

### Advanced Options

Use custom frequencies:
```bash
ush --freq-0 17000 --freq-1 21000 send "Custom frequencies"
```

Apply noise filtering:
```bash
ush listen --filter --threshold 0.2
```

Verbose logging:
```bash
ush --verbose send "Debug message"
```

## Configuration

### Audio Settings

- `--sample-rate`: Audio sample rate (default: 44100 Hz)
- `--freq-0`: Frequency for bit '0' (default: 18000 Hz)
- `--freq-1`: Frequency for bit '1' (default: 20000 Hz)

### Protocol Settings

The protocol uses:
- **Modulation**: FSK (Frequency Shift Keying)
- **Symbol Duration**: 10ms per bit
- **Error Detection**: CRC-32 checksums
- **Framing**: Preamble + start/end delimiters

### Performance

Typical performance characteristics:
- **Data rate**: ~50-100 characters per second
- **Frequency range**: 18-22 kHz (above human hearing)
- **Detection range**: 2-10 meters (depending on environment)
- **Latency**: ~100-500ms end-to-end

## Troubleshooting

### Common Issues

**"No audio device found"**
- Ensure microphone/speakers are connected and working
- Check system audio permissions
- Try `ush test devices` to list available devices

**"Failed to decode message"**
- Increase volume on sending device
- Reduce background noise
- Use `--filter` option for noisy environments
- Try `--repeat 3` to send message multiple times

**"Timeout waiting for signal"**
- Check that devices are within range (~2-10 meters)
- Ensure audio frequencies aren't blocked by speaker/mic limitations
- Verify both devices are using same frequency settings

### Audio Quality Tips

1. **Environment**: Use in quiet environments when possible
2. **Distance**: Keep devices 2-10 meters apart for best results
3. **Volume**: Set speaker volume to 50-80% (not maximum)
4. **Hardware**: Use external speakers/microphones for better range
5. **Interference**: Avoid other ultrasonic sources (some motion sensors, etc.)

### Debug Mode

Monitor live audio for debugging:
```bash
ush debug --spectrum --waveform --rate 20
```

### Performance Testing

Run comprehensive tests:
```bash
cargo test --release
```

Benchmark encoding/decoding speed:
```bash
cargo test --release test_performance_benchmarks -- --nocapture
```

## Architecture

### Module Structure

```
src/
├── main.rs          # CLI entry point and command routing
├── lib.rs           # Library exports and module declarations
├── app.rs           # Main application logic and coordination
├── cli.rs           # Command-line interface definitions
├── audio.rs         # Cross-platform audio I/O with cpal
├── modulation.rs    # FSK encoding/decoding with FFT
├── protocol.rs      # Message framing and error detection
└── error.rs         # Centralized error handling
```

### Key Dependencies

- **cpal**: Cross-platform audio I/O
- **rustfft**: Fast Fourier Transform for demodulation
- **hound**: WAV file reading/writing
- **clap**: Command-line argument parsing
- **tokio**: Async runtime for non-blocking I/O
- **crc**: CRC checksum calculation

### Protocol Stack

```
Application Layer    │ Text messages, files, chat
Protocol Layer       │ Framing, sequencing, CRC checksums
Modulation Layer     │ FSK (Frequency Shift Keying)
Physical Layer       │ Ultrasonic audio (18-22 kHz)
```

## Building from Source

### Prerequisites

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system dependencies (Linux only)
# Ubuntu/Debian:
sudo apt-get install libasound2-dev pkg-config

# CentOS/RHEL:
sudo yum install alsa-lib-devel pkgconfig
```

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- send "test"

# Generate documentation
cargo doc --open
```

### Cross-Compilation

Build for different platforms:
```bash
# Add targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin

# Cross-compile
cargo build --target x86_64-pc-windows-gnu --release
```

## Development

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_tests

# Specific test
cargo test test_full_pipeline

# With output
cargo test -- --nocapture
```

### Adding New Features

1. **Fork the repository**
2. **Create a feature branch**
3. **Write tests first** (TDD approach)
4. **Implement the feature**
5. **Run full test suite**
6. **Submit a pull request**

### Code Style

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Follow Rust naming conventions
- Add documentation for public APIs
- Include tests for new functionality

## FAQ

**Q: Why ultrasonic frequencies?**
A: Ultrasonic frequencies (18-22 kHz) are above human hearing range, so communication doesn't create audible noise. Most computer speakers and microphones support these frequencies.

**Q: What's the maximum range?**
A: Typically 2-10 meters depending on environment, speaker/microphone quality, and background noise. Outdoor range may be longer.

**Q: Can I use this through walls?**
A: Sound waves don't penetrate walls well. ush is designed for same-room or adjacent room communication.

**Q: Is this secure?**
A: Currently no encryption is implemented. Messages are transmitted as plaintext audio. Anyone with a microphone in range can receive messages.

**Q: Why is it slow compared to WiFi/Bluetooth?**
A: Audio-based communication is inherently slower due to symbol duration and error correction needs. It's designed for short messages, not bulk data transfer.

**Q: Does this work on mobile devices?**
A: The current implementation is desktop-focused. Mobile support would require platform-specific audio handling.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Areas where help is needed:
- Mobile platform support
- GUI interface
- Improved noise filtering
- Encryption/security features
- Performance optimizations
- Additional modulation schemes

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Credits

Built with:
- [cpal](https://github.com/RustAudio/cpal) for cross-platform audio
- [rustfft](https://github.com/ejmahler/RustFFT) for signal processing
- [clap](https://github.com/clap-rs/clap) for CLI parsing

Inspired by projects like:
- Acoustic data transmission protocols
- Ham radio digital modes
- Ultrasonic communication research

---

**Note**: This is experimental software. Use responsibly and be mindful

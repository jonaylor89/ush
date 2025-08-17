# System Architecture

## Overview

The `ush` system implements a layered architecture following the OSI model principles, adapted for acoustic data transmission. The design prioritizes modularity, testability, and cross-platform compatibility.

## Architecture Layers

```
┌─────────────────────────────────────────┐
│           Application Layer             │  CLI, Chat, File Transfer
├─────────────────────────────────────────┤
│           Protocol Layer                │  Framing, Sequencing, CRC
├─────────────────────────────────────────┤
│           Modulation Layer              │  FSK Encoding/Decoding
├─────────────────────────────────────────┤
│           Audio Abstraction             │  Cross-platform Audio I/O
├─────────────────────────────────────────┤
│           Physical Layer                │  Speakers, Microphones
└─────────────────────────────────────────┘
```

## Core Components

### 1. Application Layer (`src/main.rs`, `src/app.rs`, `src/cli.rs`)

**Purpose**: User interface and high-level application logic

**Key Components**:
- `UshApp`: Central application coordinator
- Command-line interface with `clap` parser
- Async event handling with Tokio

**Design Patterns**:
- **Command Pattern**: Each CLI command maps to a specific application method
- **Facade Pattern**: `UshApp` provides simplified interface to complex subsystems
- **Builder Pattern**: Audio and modulation configurations

```rust
pub struct UshApp {
    audio_manager: AudioManager,
    modulator: FskModulator,
    demodulator: FskDemodulator,
    encoder: ProtocolEncoder,
    decoder: ProtocolDecoder,
    settings: AudioSettings,
}
```

### 2. Protocol Layer (`src/protocol.rs`)

**Purpose**: Reliable message transmission with error detection

**Key Features**:
- **Message Framing**: Preamble + Start/End delimiters
- **Error Detection**: CRC-32 checksums using ISO HDLC polynomial
- **Sequencing**: Message ordering and acknowledgments
- **Serialization**: JSON-based message encoding

**Protocol Structure**:
```
Preamble (8 bytes) | Start (2 bytes) | Length (2 bytes) | Message | End (2 bytes)
   0xAAAAAAAA      |    0x7E7E       |   Big-endian    |  JSON   |   0x7E7E
```

**State Machine**: The decoder implements a finite state machine for robust frame detection:
- `WaitingForPreamble` → `WaitingForStart` → `ReadingLength` → `ReadingMessage` → `WaitingForEnd`

**Error Handling**: Based on telecommunications error correction principles:
- Automatic repeat request (ARQ) for corrupted frames
- Sliding window for partial message recovery
- Adaptive timeout mechanisms

### 3. Modulation Layer (`src/modulation.rs`)

**Purpose**: Convert digital data to/from acoustic signals

**Modulation Scheme**: Frequency Shift Keying (FSK)
- **Frequency 0**: 18,000 Hz (represents binary '0')
- **Frequency 1**: 20,000 Hz (represents binary '1')
- **Symbol Duration**: 10ms per bit
- **Sample Rate**: 44,100 Hz

**Encoding Process**:
1. Convert text → bytes → bits
2. Generate sinusoidal tones for each bit
3. Apply amplitude ramping to reduce spectral splatter
4. Concatenate symbols with smooth transitions

**Demodulation Process** (based on FFT analysis):
1. Segment received audio into symbol-length windows
2. Apply FFT to each window
3. Analyze frequency domain for peak detection
4. Convert dominant frequencies back to bits
5. Reconstruct original message

```rust
pub struct ModulationConfig {
    pub sample_rate: u32,    // 44,100 Hz
    pub freq_0: f32,         // 18,000 Hz
    pub freq_1: f32,         // 20,000 Hz
    pub symbol_duration: f32, // 0.01 seconds
    pub ramp_duration: f32,   // 0.002 seconds
}
```

### 4. Audio Abstraction Layer (`src/audio.rs`)

**Purpose**: Cross-platform audio I/O abstraction

**Implementation**: Built on the `cpal` library, which provides:
- **Cross-platform support**: Windows (WASAPI), macOS (CoreAudio), Linux (ALSA)
- **Low-latency streaming**: Real-time audio processing
- **Multiple sample formats**: Support for various bit depths and sample rates

**Audio Pipeline**:
```rust
fn create_output_stream(&self, samples: Arc<Mutex<Vec<f32>>>) -> UshResult<Stream> {
    // Platform-specific stream creation
    // Automatic sample format conversion
    // Real-time buffer management
}
```

**Key Features**:
- **Format Negotiation**: Automatic selection of supported audio formats
- **Buffer Management**: Circular buffering for continuous audio streaming
- **Error Recovery**: Graceful handling of audio device disconnection

### 5. Error Handling (`src/error.rs`)

**Design Philosophy**: Comprehensive error taxonomy using `thiserror`

**Error Categories**:
- **Audio Errors**: Device, format, and streaming issues
- **Protocol Errors**: Frame corruption, sequencing problems
- **Modulation Errors**: Signal processing failures
- **Configuration Errors**: Invalid parameters

```rust
#[derive(Error, Debug)]
pub enum UshError {
    #[error("Audio error: {0}")]
    Audio(#[from] cpal::StreamError),

    #[error("CRC mismatch: expected {expected}, got {actual}")]
    CrcMismatch { expected: u32, actual: u32 },
    // ... other error types
}
```

## Design Principles

### 1. Separation of Concerns
Each layer has a single, well-defined responsibility:
- Audio layer handles hardware abstraction
- Modulation layer handles signal processing
- Protocol layer handles reliable communication
- Application layer handles user interaction

### 2. Dependency Inversion
Higher-level modules don't depend on lower-level implementation details:
- Configuration objects passed down through layers
- Trait-based abstractions for testability
- Dependency injection for different backends

### 3. Fail-Fast Design
Errors are caught and reported as early as possible:
- Configuration validation at startup
- Audio device capability checking
- Protocol frame validation

### 4. Async-First Architecture
Non-blocking I/O throughout the system:
- Tokio runtime for async coordination
- Stream-based audio processing
- Concurrent encoding/decoding pipelines

## Memory Management

### Buffer Strategies
- **Circular Buffers**: For continuous audio streaming
- **Message Queues**: For protocol-level communication
- **Zero-Copy**: Minimize data copying between layers

### Resource Cleanup
- **RAII**: Automatic resource management using Rust's ownership system
- **Stream Lifecycle**: Explicit stream creation and destruction
- **Memory Bounds**: Configurable buffer sizes to prevent unbounded growth

## Concurrency Model

### Thread Safety
- **Arc<Mutex<T>>**: Shared state between audio callbacks and main thread
- **Message Passing**: Channel-based communication between components
- **Lock-Free Structures**: Where possible, using atomic operations

### Real-Time Constraints
- **Audio Callbacks**: Must complete within buffer duration (~10ms)
- **Signal Processing**: FFT operations optimized for real-time performance
- **Memory Allocation**: Minimize allocations in audio thread

## Testing Architecture

### Unit Tests
- **Module Isolation**: Each module tested independently
- **Mock Objects**: Simulated audio devices for deterministic testing
- **Property-Based Testing**: Randomized input validation

### Integration Tests
- **End-to-End**: Complete encode/decode pipeline testing
- **Error Injection**: Simulated network conditions and hardware failures
- **Performance Testing**: Latency and throughput measurements

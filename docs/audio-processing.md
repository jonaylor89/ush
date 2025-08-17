# Audio Processing and Cross-Platform I/O

## Overview

The audio processing subsystem provides cross-platform abstraction for real-time audio input/output, enabling ultrasonic communication across Windows, macOS, and Linux. Built on the `cpal` library, it handles device enumeration, format negotiation, and streaming audio data.

## Audio System Architecture

```
┌─────────────────────────────────────┐
│         Application Layer           │  UshApp
├─────────────────────────────────────┤
│        Audio Abstraction            │  AudioManager
├─────────────────────────────────────┤
│         cpal Library                │  Cross-platform Audio
├─────────────────────────────────────┤
│       Platform-Specific APIs        │
├─────────────────┬───────────────────┤
│    CoreAudio    │      WASAPI      │  │     ALSA      │
│     (macOS)     │     (Windows)    │  │    (Linux)    │
└─────────────────┴───────────────────┘  └───────────────┘
```

## Platform Support

### Audio API Backends

The system leverages different native audio APIs depending on the platform¹:

**macOS - Core Audio**:
- Low-latency audio framework
- Built into the operating system
- Hardware abstraction layer (HAL)
- Support for 16-bit, 24-bit, and 32-bit sample formats

**Windows - WASAPI (Windows Audio Session API)**:
- Modern Windows audio architecture
- Introduced in Windows Vista
- Low-latency exclusive mode available
- Shared and exclusive audio modes

**Linux - ALSA (Advanced Linux Sound Architecture)**:
- Kernel-level audio framework
- Direct hardware access
- Plugin architecture for format conversion
- PulseAudio compatibility layer

## Audio Configuration

### Sample Rate Selection

The system defaults to 44.1 kHz sampling rate, chosen for several technical reasons:

```rust
const SAMPLE_RATE: u32 = 44100;

pub struct AudioConfig {
    pub sample_rate: u32,    // 44,100 Hz
    pub channels: u16,       // Mono (1 channel)
    pub buffer_size: usize,  // 4,096 samples
}
```

**Rationale for 44.1 kHz**:
- **Nyquist Theorem Compliance²**: Supports frequencies up to 22.05 kHz
- **Ultrasonic Coverage**: Adequate for 18-22 kHz frequency range
- **Hardware Compatibility**: Universally supported sample rate
- **Audio CD Standard**: Well-established in consumer audio equipment

### Channel Configuration

**Mono Operation** (1 channel):
- Simplified processing pipeline
- Reduced computational complexity
- Consistent cross-platform behavior
- Most built-in microphones are mono

### Buffer Size Optimization

```rust
const BUFFER_SIZE: usize = 4096; // ~93ms at 44.1kHz
```

**Trade-offs**:
- **Latency**: Larger buffers increase latency
- **Stability**: Larger buffers reduce dropouts
- **Processing Time**: Must complete within buffer duration
- **Memory Usage**: Larger buffers consume more RAM

**Buffer Duration Calculation**:
```
Duration = Buffer_Size / Sample_Rate
         = 4096 / 44100
         = 0.093 seconds (93 ms)
```

## Device Management

### Device Enumeration and Selection

```rust
impl AudioManager {
    fn get_input_device(&self) -> UshResult<Device> {
        self.host
            .default_input_device()
            .ok_or_else(|| UshError::Config {
                message: "No input device available".to_string(),
            })
    }

    fn get_output_device(&self) -> UshResult<Device> {
        self.host
            .default_output_device()
            .ok_or_else(|| UshError::Config {
                message: "No output device available".to_string(),
            })
    }
}
```

### Format Negotiation

The system automatically negotiates audio formats with available hardware:

```rust
fn get_supported_config(&self, device: &Device, is_input: bool) -> UshResult<SupportedStreamConfig> {
    let sample_rate = SampleRate(self.config.sample_rate);

    if is_input {
        for config in device.supported_input_configs()? {
            if config.channels() == self.config.channels
                && config.min_sample_rate() <= sample_rate
                && sample_rate <= config.max_sample_rate()
            {
                return Ok(config.with_sample_rate(sample_rate));
            }
        }
    }
    // Similar logic for output devices...
}
```

**Capability Matching**:
1. **Channel Count**: Must support mono (1 channel)
2. **Sample Rate**: Must support 44.1 kHz
3. **Sample Format**: Automatic conversion between formats
4. **Buffer Size**: Negotiate optimal buffer size

## Sample Format Handling

### Format Conversion

The system supports multiple sample formats with automatic conversion³:

```rust
fn fill_output_buffer<T>(
    data: &mut [T],
    samples: &Arc<Mutex<Vec<f32>>>,
    // ...
) where
    T: Sample + Send + FromSample<f32>,
{
    let sample_value = samples_lock[*index_lock];
    let converted_sample = T::from_sample(sample_value);

    for sample in frame.iter_mut() {
        *sample = converted_sample;
    }
}
```

**Supported Formats**:
- **I8**: 8-bit signed integer (-128 to 127)
- **I16**: 16-bit signed integer (-32,768 to 32,767)
- **I32**: 32-bit signed integer
- **F32**: 32-bit floating point (-1.0 to 1.0)

**Conversion Process**:
```rust
// Float to Integer conversion
fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

// Integer to Float conversion
fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / i16::MAX as f32
}
```

## Streaming Audio

### Input Stream Creation

```rust
pub fn create_input_stream(
    &self,
    mut callback: impl FnMut(&[f32]) + Send + 'static,
) -> UshResult<Stream> {
    let device = self.get_input_device()?;
    let config = self.get_supported_config(&device, true)?;

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &InputCallbackInfo| {
                callback(data);
            },
            |err| warn!("Input stream error: {}", err),
            None,
        )?,
        // Handle other formats...
    };

    Ok(stream)
}
```

**Input Processing Pipeline**:
1. **Hardware → Driver**: Audio samples from microphone
2. **Driver → cpal**: Platform-specific audio buffer
3. **cpal → Callback**: Format conversion and delivery
4. **Callback → Application**: User-defined processing

### Output Stream Creation

```rust
pub fn create_output_stream(
    &self,
    samples: Arc<Mutex<Vec<f32>>>,
    finished_tx: mpsc::UnboundedSender<()>,
) -> UshResult<Stream> {
    let device = self.get_output_device()?;
    let config = self.get_supported_config(&device, false)?;

    let sample_index = Arc::new(Mutex::new(0usize));
    let channels = config.channels() as usize;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [T], _: &OutputCallbackInfo| {
            Self::fill_output_buffer(data, &samples, &sample_index, channels, &finished_tx);
        },
        |err| warn!("Output stream error: {}", err),
        None,
    )?;

    Ok(stream)
}
```

**Output Processing Pipeline**:
1. **Application**: Generate audio samples
2. **Shared Memory**: Thread-safe sample buffer
3. **Output Callback**: Fill hardware buffer
4. **Platform Audio**: Convert and send to speakers

## Real-Time Constraints

### Audio Thread Requirements

Audio callbacks operate under strict real-time constraints⁴:

**Critical Requirements**:
- **No blocking operations**: No file I/O, network, or long computations
- **Bounded execution time**: Must complete within buffer duration
- **No memory allocation**: Avoid heap allocation in callbacks
- **Lock-free when possible**: Minimize mutex usage

**Implementation Strategy**:
```rust
// Pre-allocate shared buffer
let samples = Arc::new(Mutex::new(Vec::with_capacity(MAX_SAMPLES)));

// Audio callback - minimal work
move |data: &[f32], _: &InputCallbackInfo| {
    if let Ok(mut buffer) = samples_clone.try_lock() {
        buffer.extend_from_slice(data);
    }
    // No other processing in callback
}
```

### Dropout Prevention

**Buffer Underrun/Overrun Prevention**:
- **Double Buffering**: Alternate between buffers
- **Circular Buffers**: Continuous data flow
- **Adaptive Buffer Size**: Adjust based on system performance
- **Priority Scheduling**: Real-time thread priority where available

## Threading Model

### Thread Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Main Thread   │    │  Audio Thread   │    │  Async Thread   │
│                 │    │  (OS managed)   │    │   (Tokio)       │
│ • UI/CLI        │◄──►│ • Input Callback│◄──►│ • Signal Proc.  │
│ • Control Logic │    │ • Output Callback    │ • Protocol      │
│ • Configuration │    │ • Real-time      │    │ • I/O           │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Thread Communication

**Shared State Management**:
```rust
// Thread-safe audio buffer
let audio_buffer = Arc::new(Mutex<Vec<f32>>>::new(Vec::new()));

// Message passing for control
let (control_tx, control_rx) = mpsc::unbounded_channel();

// Completion notification
let (finished_tx, finished_rx) = mpsc::unbounded_channel();
```

**Synchronization Patterns**:
- **Arc<Mutex<T>>**: Shared mutable state
- **mpsc channels**: Message passing
- **Atomic operations**: Lock-free primitives where applicable

## Error Handling and Recovery

### Common Audio Errors

**Device Errors**:
```rust
#[derive(Error, Debug)]
pub enum UshError {
    #[error("Audio device error: {0}")]
    AudioDevice(#[from] cpal::DevicesError),

    #[error("Audio format error: {0}")]
    AudioFormat(#[from] cpal::SupportedStreamConfigsError),

    #[error("Audio stream error: {0}")]
    Audio(#[from] cpal::StreamError),
}
```

**Recovery Strategies**:
1. **Device Disconnection**: Graceful degradation, user notification
2. **Format Mismatch**: Automatic fallback to supported formats
3. **Stream Failure**: Restart stream with different parameters
4. **Buffer Overflow**: Increase buffer size, reduce processing load

### Robustness Measures

**Stream Health Monitoring**:
```rust
// Error callback for stream monitoring
|err| {
    match err {
        StreamError::DeviceNotAvailable => {
            // Attempt device reconnection
            warn!("Audio device disconnected, attempting reconnection...");
        },
        StreamError::BackendSpecific { err } => {
            // Platform-specific error handling
            error!("Platform-specific audio error: {:?}", err);
        },
    }
}
```

## Performance Optimization

### CPU Usage Optimization

**Efficient Sample Processing**:
```rust
// Vectorized operations where possible
fn process_samples_simd(samples: &mut [f32]) {
    // Use SIMD instructions for bulk operations
    for chunk in samples.chunks_exact_mut(4) {
        // Process 4 samples simultaneously
    }
}
```

**Memory Layout Optimization**:
- **Cache-friendly access patterns**: Sequential memory access
- **Alignment**: Ensure proper memory alignment for SIMD
- **Buffer reuse**: Minimize allocation/deallocation

### Latency Minimization

**End-to-End Latency Components**⁵:
```
Total Latency = Input_Buffer + Processing + Output_Buffer + Hardware_Latency

Where:
- Input_Buffer: ~93ms (4096 samples at 44.1kHz)
- Processing: <10ms (signal processing time)
- Output_Buffer: ~93ms (4096 samples at 44.1kHz)
- Hardware_Latency: ~5-20ms (device dependent)

Total: ~200-220ms typical
```

**Latency Reduction Techniques**:
- **Smaller buffer sizes**: Reduce buffering delay
- **Exclusive mode**: Bypass audio mixing (Windows)
- **Real-time scheduling**: Higher thread priority
- **Hardware acceleration**: Dedicated audio processors

## Testing and Validation

### Audio System Testing

**Loopback Testing**:
```rust
#[tokio::test]
async fn test_audio_loopback() -> UshResult<()> {
    let manager = AudioManager::new()?;

    // Generate test signal
    let test_freq = 1000.0; // 1 kHz sine wave
    let test_samples = generate_sine_wave(test_freq, SAMPLE_RATE, 1.0);

    // Record output through input (requires physical loopback)
    let recorded = record_playback(&manager, &test_samples).await?;

    // Verify signal integrity
    let correlation = cross_correlate(&test_samples, &recorded);
    assert!(correlation > 0.9, "Poor audio fidelity: {}", correlation);

    Ok(())
}
```

### Cross-Platform Validation

**Platform-Specific Tests**:
- **macOS**: Core Audio compatibility testing
- **Windows**: WASAPI exclusive mode testing
- **Linux**: ALSA/PulseAudio compatibility
- **Hardware Variation**: Different audio interfaces

## Future Enhancements

### Advanced Audio Features

**Adaptive Buffer Sizing**:
```rust
fn adjust_buffer_size(current_latency: Duration, target_latency: Duration) -> usize {
    let ratio = target_latency.as_secs_f32() / current_latency.as_secs_f32();
    (BUFFER_SIZE as f32 * ratio).round() as usize
}
```

**Multi-Channel Support**:
- Stereo operation for increased data rate
- Channel-specific frequency allocation
- Spatial audio techniques

**Hardware Acceleration**:
- GPU-based FFT processing
- Dedicated DSP hardware utilization
- ASIO driver support (Windows)

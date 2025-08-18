# AGENT.md - Guidelines for AI Coding Agents

This document provides specific guidelines for AI coding agents working on the **ush** (Ultrasonic Shell) project.

## Project Overview

**ush** is a terminal application for ultrasonic communication between devices. Key characteristics:

- **Real-time audio processing** with strict timing constraints
- **Cross-platform compatibility** (macOS, Linux, Windows)
- **Signal processing** using FFT and digital filters
- **Protocol implementation** with error detection and recovery
- **CLI-focused** with potential for GUI expansion

## Development Workflow

### Primary Build System: Just

This project uses [`just`](https://github.com/casey/just) as the primary task runner. **Always prefer `just` commands over raw `cargo` commands.**

```bash
# Essential commands for agents
just                    # List all available commands
just dev                # Quick development check (check + test)
just check-all          # Full quality pipeline (fmt + lint + test)
just build-release      # Optimized build
just test-loopback      # Functional test of core features
just demo               # Comprehensive feature demonstration
```

### Code Quality Pipeline

Before any code changes, run:
```bash
just check-all
```

This executes:
1. `cargo fmt` - Code formatting
2. `cargo clippy -- -D warnings` - Linting (treats warnings as errors)
3. `cargo test` - Full test suite

### Testing Strategy

1. **Always run loopback tests** for audio changes:
   ```bash
   just test-loopback
   ```

2. **Run integration tests** for protocol changes:
   ```bash
   just test-integration
   ```

3. **Performance testing** for optimization work:
   ```bash
   just test-bench
   ```

## Code Standards for Agents

### Rust Best Practices

```rust
// ✅ Good: Proper error handling
fn process_audio_frame(samples: &[f32]) -> Result<DecodedMessage, AudioError> {
    let filtered = apply_bandpass_filter(samples)?;
    decode_message_from_samples(filtered)
}

// ❌ Bad: Using unwrap() in production code
fn process_audio_frame(samples: &[f32]) -> DecodedMessage {
    let filtered = apply_bandpass_filter(samples).unwrap();
    decode_message_from_samples(filtered).unwrap()
}

// ✅ Good: Pre-allocate when size is known
fn generate_tone(frequency: f32, duration: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration * sample_rate as f32) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    // ... fill samples
    samples
}

// ✅ Good: Comprehensive documentation
/// Decodes FSK-modulated audio samples into binary data
///
/// # Arguments
/// * `samples` - Raw audio samples (f32, normalized -1.0 to 1.0)
/// * `sample_rate` - Audio sample rate in Hz
/// * `freq_0` - Frequency representing binary '0'
/// * `freq_1` - Frequency representing binary '1'
///
/// # Returns
/// * `Ok(Vec<u8>)` - Decoded binary data
/// * `Err(DemodulationError)` - If signal could not be decoded
///
/// # Examples
/// ```
/// let samples = vec![0.1, 0.2, -0.1, 0.3];
/// let data = decode_fsk_samples(&samples, 44100, 18000.0, 20000.0)?;
/// ```
fn decode_fsk_samples(
    samples: &[f32],
    sample_rate: u32,
    freq_0: f32,
    freq_1: f32
) -> Result<Vec<u8>, DemodulationError> {
    // Implementation...
}
```

### Audio-Specific Guidelines

**Performance Critical Paths:**
- Audio callback functions must complete within 10ms
- Avoid allocations in real-time audio processing
- Use `Vec::with_capacity()` for known-size buffers
- Prefer in-place operations when possible

**Cross-Platform Audio:**
- Test changes on multiple platforms if possible
- Handle audio device errors gracefully
- Consider different audio hardware capabilities
- Use `cpal` for all audio I/O operations

**Signal Processing:**
- Validate DSP algorithms with test signals
- Use `rustfft` for frequency domain operations
- Apply appropriate windowing for FFT operations
- Consider numerical stability in floating-point operations

## Project Structure Understanding

```
src/
├── main.rs          # CLI entry point - modify for new commands
├── lib.rs           # Public API exports - update when adding modules
├── app.rs           # Application coordination - main business logic
├── cli.rs           # Command definitions - add new CLI options here
├── audio.rs         # Audio I/O abstraction - platform-specific code
├── modulation.rs    # Signal processing - DSP algorithms
├── protocol.rs      # Message framing - packet structure and CRC
└── error.rs         # Error types - add new error variants here
```

### Module Interaction Patterns

- **CLI → App → Core modules**: Commands flow through app.rs coordination
- **Audio independence**: audio.rs should be swappable for different backends
- **Protocol layering**: Keep modulation separate from framing
- **Error propagation**: Use `anyhow::Error` for application errors, specific types for library errors

## Common Tasks for Agents

### Adding New CLI Commands

1. Define command in `src/cli.rs`:
   ```rust
   #[derive(Parser)]
   pub enum Commands {
       // ... existing commands

       /// New command description
       NewCommand {
           /// Command argument
           #[arg(short, long)]
           argument: String,
       },
   }
   ```

2. Handle command in `src/main.rs` or `src/app.rs`
3. Add corresponding function to `Justfile` if needed
4. Update help text and documentation

### Modifying Audio Processing

1. **Always test with loopback first**:
   ```bash
   just test-loopback
   ```

2. **Add unit tests for new DSP functions**
3. **Consider performance impact** - profile if needed
4. **Test edge cases** (silence, noise, clipping)

### Protocol Changes

1. **Maintain backward compatibility** when possible
2. **Update protocol version** if incompatible changes
3. **Add comprehensive tests** for new message types
4. **Document protocol changes** in comments

### Adding Dependencies

1. **Justify new dependencies** in PR description
2. **Check compatibility** with existing dep versions
3. **Prefer well-maintained crates** with recent updates
4. **Update `Cargo.toml` categories/keywords** if relevant

## Testing Guidelines

### Test Hierarchy

1. **Unit tests**: Test individual functions
   ```bash
   cargo test test_function_name
   ```

2. **Integration tests**: Test module interactions
   ```bash
   just test-integration
   ```

3. **End-to-end tests**: Full pipeline validation
   ```bash
   just test-loopback
   ```

### Audio Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone_generation() {
        let samples = generate_tone(1000.0, 0.1, 44100);
        assert_eq!(samples.len(), 4410);

        // Verify frequency content
        let fft_result = analyze_frequency_content(&samples);
        assert!(fft_result.dominant_frequency_near(1000.0, 10.0));
    }

    #[test]
    fn test_encoding_decoding_roundtrip() {
        let original = b"Hello, World!";
        let audio = encode_to_audio(original, &AudioConfig::default()).unwrap();
        let decoded = decode_from_audio(&audio, &AudioConfig::default()).unwrap();
        assert_eq!(original, &decoded[..]);
    }
}
```

## Performance Considerations

### Real-time Constraints

- **Audio callbacks**: Must complete in <10ms
- **Message processing**: Target <100ms end-to-end latency
- **Memory allocation**: Minimize in hot paths

### Optimization Targets

- **Encoding speed**: >100 chars/second
- **Decoding accuracy**: >95% in quiet environments
- **Memory usage**: <50MB for typical operations
- **CPU usage**: <25% of one core during active communication

## Common Pitfalls to Avoid

1. **Don't use `.unwrap()` in production code** - always handle errors
2. **Don't ignore audio timing constraints** - profile audio callbacks
3. **Don't assume audio hardware capabilities** - handle device limitations
4. **Don't break backward compatibility** without version bump
5. **Don't skip integration tests** - unit tests aren't enough for audio
6. **Don't hardcode audio parameters** - make them configurable
7. **Don't forget cross-platform testing** - audio varies by OS

## Integration with CI/CD

The project uses GitHub Actions with these checks:
- Build verification
- Test execution
- Formatting check
- Linting validation

When making changes, ensure:
```bash
just ci  # Runs full CI pipeline locally
```

## Resources

- **Audio Programming**: Understanding DSP concepts is crucial
- **Rust Audio**: Familiarize yourself with `cpal` and `rustfft`
- **Protocol Design**: Study existing digital communication protocols
- **Cross-platform**: Test on different operating systems when possible

---

**Remember**: This is experimental software dealing with real-time audio. Always test thoroughly and consider the impact on user experience.

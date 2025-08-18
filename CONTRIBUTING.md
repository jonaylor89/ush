# Contributing to ush (Ultrasonic Shell)

Thank you for your interest in contributing to **ush**! This document provides guidelines for contributing to the project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Standards](#code-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Project Structure](#project-structure)
- [Areas for Contribution](#areas-for-contribution)
- [Community Guidelines](#community-guidelines)

## Getting Started

### Prerequisites

- **Rust**: Version 1.70+ (2021 edition)
- **Audio Hardware**: Working microphone and speakers/headphones
- **System Dependencies** (Linux only):
  - Ubuntu/Debian: `sudo apt-get install libasound2-dev pkg-config`
  - CentOS/RHEL: `sudo yum install alsa-lib-devel pkgconfig`

### Initial Setup

1. **Fork the repository** on GitHub
2. **Clone your fork**:
   ```bash
   git clone https://github.com/YOUR_USERNAME/ush.git
   cd ush
   ```
3. **Set up the development environment**:
   ```bash
   just setup
   ```
   This will install necessary Rust components and cross-compilation targets.

4. **Verify your setup**:
   ```bash
   just smoke-test
   ```

## Development Setup

### Using Just (Recommended)

This project uses [`just`](https://github.com/casey/just) as a command runner. Install it with:
```bash
cargo install just
```

Common development commands:
```bash
just                    # List all available commands
just dev                # Run development checks (check + test)
just check-all          # Run format, lint, and test
just demo               # Run comprehensive feature demo
just test-loopback      # Quick functionality test
```

### Manual Commands

If you prefer not to use `just`:
```bash
cargo build             # Build debug version
cargo test              # Run tests
cargo fmt               # Format code
cargo clippy            # Run linter
```

## Code Standards

### Rust Code Quality

- **Follow Rust conventions**: Use `snake_case` for functions/variables, `PascalCase` for types
- **Format code**: Always run `cargo fmt` before committing
- **Lint code**: Fix all `cargo clippy` warnings
- **Documentation**: Add doc comments for all public APIs using `///`
- **Error handling**: Use `Result<T, E>` and `anyhow::Error` appropriately
- **Avoid unwrap()**: Use proper error handling instead of `.unwrap()` in production code

### Code Style Guidelines

```rust
// Good: Descriptive function names with proper error handling
fn decode_audio_samples(samples: &[f32]) -> Result<Vec<u8>, AudioError> {
    // Implementation
}

// Good: Clear struct definitions with documentation
/// Represents an ultrasonic message frame with error detection
#[derive(Debug, Clone)]
pub struct MessageFrame {
    /// CRC checksum for error detection
    pub checksum: u32,
    /// Actual message payload
    pub payload: Vec<u8>,
}

// Good: Proper error handling
match audio_device.capture() {
    Ok(samples) => process_samples(samples)?,
    Err(e) => return Err(AudioError::CaptureFailure(e)),
}

// Avoid: Unwrapping without context
let samples = audio_device.capture().unwrap(); // Bad!
```

### Performance Considerations

- **Audio processing is time-critical**: Avoid allocations in hot paths
- **Use `Vec::with_capacity()`** when size is known
- **Profile audio latency**: Keep processing under 100ms when possible
- **Benchmark changes**: Use `cargo test --release test_performance_benchmarks`

### Dependencies

- **Minimize new dependencies**: Justify any additions
- **Use well-maintained crates**: Check recent updates and community support
- **Audio-specific**: Prefer `cpal` for cross-platform audio I/O
- **No std when possible**: Consider `no_std` compatibility for core algorithms

## Testing

### Test Categories

1. **Unit Tests**: Test individual functions
   ```bash
   cargo test
   ```

2. **Integration Tests**: Test complete workflows
   ```bash
   just test-integration
   ```

3. **Performance Tests**: Benchmark critical paths
   ```bash
   just test-bench
   ```

4. **Loopback Tests**: End-to-end functionality
   ```bash
   just test-loopback
   ```

### Writing Tests

- **Test all public APIs**: Every public function should have tests
- **Test error cases**: Verify proper error handling
- **Use descriptive test names**: `test_audio_encoding_with_noise_filtering`
- **Mock audio devices**: Use test fixtures instead of real hardware when possible

Example test structure:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_encoding_roundtrip() {
        let original = "Hello, World!";
        let encoded = encode_message(original).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_error_detection_with_corrupted_data() {
        let mut encoded = encode_message("test").unwrap();
        encoded[0] ^= 0xFF; // Corrupt first byte

        let result = decode_message(&encoded);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::ChecksumMismatch));
    }
}
```

## Submitting Changes

### Pull Request Process

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following the code standards above

3. **Run all checks**:
   ```bash
   just check-all
   ```

4. **Write or update tests** for your changes

5. **Update documentation** if needed (README.md, doc comments)

6. **Commit with descriptive messages**:
   ```bash
   git commit -m "feat: Add noise filtering for improved signal detection

   - Implement bandpass filter for ultrasonic range
   - Add configurable noise threshold
   - Include tests for various noise conditions"
   ```

7. **Push and create pull request**:
   ```bash
   git push origin feature/your-feature-name
   ```

### Commit Message Format

Use conventional commits:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `test:` - Test additions/changes
- `refactor:` - Code refactoring
- `perf:` - Performance improvements
- `chore:` - Build/tooling changes

### Review Process

- All PRs require review before merging
- CI must pass (build, test, lint)
- Maintain test coverage
- Address review feedback promptly

## Areas for Contribution

We welcome contributions in these areas:

### High Priority
- **Mobile platform support** (iOS/Android)
- **Improved noise filtering** algorithms
- **Performance optimizations** in signal processing

### Medium Priority
- **Encryption/security features** for private communication
- **Additional modulation schemes** (PSK, OFDM)
- **Better error recovery** protocols
- **Cross-platform audio device management**

### Low Priority
- **Plugin system** for custom protocols
- **Network bridging** (audio to TCP/UDP)
- **Advanced signal analysis** tools
- **GUI interface** for non-technical users
- **Multi-device coordination**

### Documentation
- **Video tutorials** for setup and usage
- **Technical deep-dives** on the protocol
- **API documentation** improvements
- **Translation** of documentation

## Community Guidelines

### Code of Conduct

- Be respectful and inclusive
- Welcome newcomers and help them learn
- Provide constructive feedback
- Focus on the technical merits of contributions

### Communication

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: Questions, ideas, general discussion
- **Pull Requests**: Code review and technical discussion

### Getting Help

- Check existing issues and documentation first
- Provide minimal reproducible examples
- Include system information (OS, Rust version, audio hardware)
- Be patient - maintainers are volunteers

## Audio-Specific Considerations

When working on audio features:

- **Test on multiple platforms**: Audio behavior varies significantly
- **Consider hardware limitations**: Not all speakers/mics support ultrasonic frequencies
- **Handle timing carefully**: Audio processing has real-time constraints
- **Account for environmental factors**: Noise, distance, interference
- **Validate with real hardware**: Simulators may not catch audio issues

## License

By contributing to ush, you agree that your contributions will be licensed under the License.

---

Thank you for contributing to ush! Your efforts help make ultrasonic communication more accessible and robust for everyone.

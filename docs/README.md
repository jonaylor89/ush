# Technical Documentation

This directory contains detailed technical documentation for the `ush` (ultrasonic shell) project.

## Documentation Structure

- **[architecture.md](architecture.md)** - Overall system architecture and design patterns
- **[audio-processing.md](audio-processing.md)** - Cross-platform audio I/O and signal processing
- **[modulation.md](modulation.md)** - FSK modulation theory and implementation
- **[protocol.md](protocol.md)** - Message framing, error detection, and networking protocol
- **[performance.md](performance.md)** - Performance characteristics and optimization techniques
- **[references.md](references.md)** - Academic papers, standards, and external resources

## Quick Technical Overview

`ush` implements ultrasonic data communication using:

1. **Frequency Shift Keying (FSK)** modulation at 18-22 kHz
2. **Fast Fourier Transform (FFT)** for signal demodulation
3. **CRC-32** checksums for error detection
4. **Cross-platform audio** via the `cpal` library
5. **Async I/O** with Tokio for non-blocking operations

The system achieves approximately 50-100 characters per second data transmission rate over distances of 2-10 meters, depending on environmental conditions.

## Key Technical Innovations

- **Adaptive symbol timing** with configurable duration and ramping
- **Multi-platform audio abstraction** supporting different sample formats
- **Robust protocol stack** with automatic error recovery
- **Real-time signal processing** with minimal latency
- **Comprehensive test coverage** including noise resilience testing
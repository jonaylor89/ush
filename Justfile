# Justfile for ush (ultrasonic shell)
# https://github.com/casey/just

# Default recipe - shows available commands
default:
    @just --list

# Build the project in debug mode
build:
    cargo build

# Build the project in release mode (optimized)
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run only integration tests
test-integration:
    cargo test --test integration_tests

# Run a specific test
test-single TEST:
    cargo test {{TEST}} -- --nocapture

# Run tests in release mode for performance benchmarks
test-bench:
    cargo test --release test_performance_benchmarks -- --nocapture

# Run clippy for linting
lint:
    cargo clippy -- -D warnings

# Install the binary locally
install:
    cargo install --path .

# Uninstall the binary
uninstall:
    cargo uninstall ush

# Run with debug logging
run-debug *ARGS:
    RUST_LOG=debug cargo run -- {{ARGS}}

# Run with info logging
run-info *ARGS:
    RUST_LOG=info cargo run -- {{ARGS}}

# Run basic loopback test
test-loopback:
    cargo run --release -- test loopback "Hello, Just!"

# Send a test message
send MESSAGE:
    cargo run --release -- send "{{MESSAGE}}"

# Listen for messages (30 second timeout)
listen:
    cargo run --release -- listen --timeout 30

# Start chat mode
chat:
    cargo run --release -- chat --username justuser

# Generate a test tone
tone FREQ DURATION:
    cargo run --release -- test generate {{FREQ}} --duration {{DURATION}}

# Measure background noise
noise:
    cargo run --release -- test noise --duration 5

# List audio devices
devices:
    cargo run --release -- test devices

# Run all quality checks (format, lint, test)
check-all: test
    cargo fmt
    cargo clippy -- -D warnings

# Build for all supported platforms
build-all: build-linux build-mac build-windows

# Build for Linux
build-linux:
    cargo build --release --target x86_64-unknown-linux-gnu

# Build for macOS
build-mac:
    cargo build --release --target x86_64-apple-darwin

# Build for Windows
build-windows:
    cargo build --release --target x86_64-pc-windows-gnu

# Set up cross-compilation targets
setup-targets:
    rustup target add x86_64-unknown-linux-gnu
    rustup target add x86_64-apple-darwin
    rustup target add x86_64-pc-windows-gnu

# Comprehensive demo of all features
demo:
    @echo "🔊 ush Demo - Testing all major features"
    @echo "========================================"

    @echo "\n1. Building project..."
    just build-release

    @echo "\n2. Running loopback test..."
    cargo run --release -- test loopback "Demo test message"

    @echo "\n3. Testing different message lengths..."
    cargo run --release -- test loopback "Short"
    cargo run --release -- test loopback "This is a medium length message for testing."

    @echo "\n4. Testing Unicode support..."
    cargo run --release -- test loopback "Hello 世界 🌊"

    @echo "\n5. Generating test tones..."
    @echo "   - 18kHz tone (freq_0):"
    timeout 3 cargo run --release -- test generate 18000 --duration 2 || true
    @echo "   - 20kHz tone (freq_1):"
    timeout 3 cargo run --release -- test generate 20000 --duration 2 || true

    @echo "\n6. Measuring background noise..."
    cargo run --release -- test noise --duration 3

    @echo "\n✅ Demo complete!"

# Development setup
setup:
    @echo "🔧 Setting up development environment"
    rustup component add rustfmt clippy
    just setup-targets
    @echo "✅ Development environment ready!"

# Simulate two-device communication test
test-communication:
    @echo "🔄 Testing two-device communication simulation"
    @echo "This would normally require two separate terminals/devices"
    @echo "Device 1: Sending message..."
    cargo run --release -- send "Hello from device 1" --save-wav /tmp/ush-test.wav
    @echo "Device 2: Receiving message..."
    cargo run --release -- listen --from-wav /tmp/ush-test.wav
    @rm -f /tmp/ush-test.wav

# Full CI/CD pipeline simulation
ci: test build-release
    cargo fmt
    cargo clippy -- -D warnings
    @echo "✅ All CI checks passed!"

# Development workflow helper
dev: test
    cargo check
    @echo "✅ Development checks passed!"

# Quick smoke test
smoke-test:
    cargo run --release -- test loopback "Smoke test"

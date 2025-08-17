# Communication Protocol

## Overview

The `ush` protocol implements a reliable data link layer protocol optimized for acoustic transmission. It provides message framing, error detection, sequencing, and basic flow control over the unreliable acoustic channel.

## Protocol Stack

```
┌─────────────────────────────────────┐
│        Application Messages         │  Text, Files, Commands
├─────────────────────────────────────┤
│         Message Protocol            │  JSON Serialization
├─────────────────────────────────────┤
│          Frame Protocol             │  Framing, CRC, Sequencing
├─────────────────────────────────────┤
│         FSK Modulation              │  Digital → Acoustic
├─────────────────────────────────────┤
│         Audio Channel               │  Speakers ↔ Microphones
└─────────────────────────────────────┘
```

## Frame Structure

### Frame Format

Each transmitted frame follows this structure:

```
┌─────────────┬─────────────┬─────────────┬─────────────┬─────────────┐
│   Preamble  │    Start    │   Length    │   Payload   │     End     │
│   8 bytes   │   2 bytes   │   2 bytes   │  Variable   │   2 bytes   │
└─────────────┴─────────────┴─────────────┴─────────────┴─────────────┘
```

### Field Descriptions

1. **Preamble (8 bytes)**: `0xAAAAAAAAAAAAAAAA`
   - Alternating pattern for receiver synchronization
   - Allows clock recovery and signal detection
   - Double preamble (2×4 bytes) for robust detection

2. **Start Delimiter (2 bytes)**: `0x7E7E`
   - Unique pattern indicating frame start
   - Based on HDLC flag sequence
   - Repeated for reliability

3. **Length Field (2 bytes)**: Big-endian message length
   - Maximum payload: 1024 bytes
   - Includes serialized message data
   - Used for frame boundary detection

4. **Payload (Variable)**: JSON-serialized message
   - Contains application data
   - Includes CRC checksum within message
   - Structured as `Message` object

5. **End Delimiter (2 bytes)**: `0x7E7E`
   - Frame termination marker
   - Enables frame validation
   - Same pattern as start for simplicity

### Message Structure

The payload contains a JSON-serialized `Message` object:

```rust
#[derive(Serialize, Deserialize)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
    pub checksum: u32,
}

#[derive(Serialize, Deserialize)]
pub struct MessageHeader {
    pub version: u8,           // Protocol version (currently 1)
    pub message_type: MessageType,
    pub sequence_number: u32,  // For ordering and deduplication
    pub timestamp: u64,        // Unix timestamp in seconds
    pub payload_length: u16,   // Length of payload field
}
```

### Message Types

```rust
#[derive(Serialize, Deserialize)]
pub enum MessageType {
    Text,     // Human-readable text messages
    File,     // File transfer chunks
    Ack,      // Acknowledgment messages
    Ping,     // Connectivity testing
}
```

## Error Detection

### CRC-32 Implementation

The protocol uses CRC-32 with the ISO HDLC polynomial for error detection:

**Polynomial**: `x³² + x²⁶ + x²³ + x²² + x¹⁶ + x¹² + x¹¹ + x¹⁰ + x⁸ + x⁷ + x⁵ + x⁴ + x² + x + 1`

**Implementation**:
```rust
use crc::{Crc, CRC_32_ISO_HDLC};

fn calculate_checksum(header: &MessageHeader, payload: &[u8]) -> UshResult<u32> {
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    // Serialize header to bytes for checksum calculation
    let header_bytes = serde_json::to_vec(header)?;

    let mut digest = crc.digest();
    digest.update(&header_bytes);
    digest.update(payload);

    Ok(digest.finalize())
}
```

### Error Detection Capability

CRC-32 provides strong error detection:
- **Undetected error probability**: ~2³² ≈ 1 in 4.3 billion
- **Burst error detection**: Up to 32 consecutive bit errors
- **Random error detection**: Any odd number of bit errors

### Checksum Verification Process

1. **Sender**:
   ```rust
   let checksum = calculate_checksum(&header, &payload)?;
   let message = Message { header, payload, checksum };
   ```

2. **Receiver**:
   ```rust
   fn verify_checksum(&self) -> UshResult<bool> {
       let calculated = Self::calculate_checksum(&self.header, &self.payload)?;
       Ok(calculated == self.checksum)
   }
   ```

3. **Error Handling**:
   - Invalid checksums trigger frame rejection
   - Corrupted frames are logged for debugging
   - Automatic retry logic in higher layers

## Frame Synchronization

### Decoder State Machine

The protocol decoder implements a finite state machine for robust frame processing:

```rust
#[derive(Debug, PartialEq)]
enum DecoderState {
    WaitingForPreamble,    // Scanning for preamble pattern
    WaitingForStart,       // Looking for start delimiter
    ReadingLength,         // Reading 2-byte length field
    ReadingMessage,        // Reading variable-length payload
    WaitingForEnd,         // Expecting end delimiter
}
```

### State Transitions

```
┌─────────────────────┐
│ WaitingForPreamble  │────┐
└─────────────────────┘    │ Preamble found
           ▲               ▼
           │          ┌──────────────┐
    Reset  │          │WaitingForStart│
           │          └──────────────┘
           │               │ Start delimiter found
           │               ▼
           │          ┌──────────────┐
           │          │ ReadingLength│
           │          └──────────────┘
           │               │ Length read
           │               ▼
           │          ┌──────────────┐
    Error  │          │ReadingMessage│
           │          └──────────────┘
           │               │ Message complete
           │               ▼
           │          ┌──────────────┐
           └──────────│ WaitingForEnd│
                      └──────────────┘
                           │ End delimiter found
                           ▼
                      [Frame Complete]
```

### Preamble Detection

The preamble detection algorithm uses pattern matching:

```rust
fn find_preamble(&self) -> Option<usize> {
    let double_preamble = [PREAMBLE, PREAMBLE].concat(); // 0xAAAA...

    if self.buffer.len() < double_preamble.len() {
        return None;
    }

    for i in 0..=self.buffer.len() - double_preamble.len() {
        if &self.buffer[i..i + double_preamble.len()] == double_preamble {
            return Some(i);
        }
    }

    None
}
```

### Frame Boundary Detection

The system uses multiple mechanisms for frame boundary detection:

1. **Length Field Validation**: Prevents buffer overflows
2. **Maximum Frame Size**: 1024 bytes + overhead
3. **Timeout Mechanism**: Prevents indefinite blocking
4. **Pattern Validation**: Start/end delimiters must match

## Sequencing and Flow Control

### Sequence Numbers

Messages include monotonically increasing sequence numbers:
- **32-bit counter**: Wraps at 2³² - 1
- **Per-session uniqueness**: Reset for each communication session
- **Gap detection**: Missing sequences indicate lost frames

### Acknowledgment Protocol

Basic stop-and-wait ARQ (Automatic Repeat reQuest)⁵:

```rust
pub fn new_ack(sequence_number: u32) -> UshResult<Self> {
    let header = MessageHeader {
        version: PROTOCOL_VERSION,
        message_type: MessageType::Ack,
        sequence_number,
        timestamp: current_timestamp(),
        payload_length: 0,
    };

    Ok(Self {
        header,
        payload: Vec::new(),
        checksum: Self::calculate_checksum(&header, &[])?,
    })
}
```

### Timeout and Retransmission

While not fully implemented in the current version, the protocol supports:
- **Configurable timeouts**: Based on channel characteristics
- **Exponential backoff**: Reduces network congestion
- **Maximum retry count**: Prevents infinite loops

## Serialization

### JSON Message Format

Messages are serialized using serde_json for human readability and debugging:

**Example Text Message**:
```json
{
  "header": {
    "version": 1,
    "message_type": "Text",
    "sequence_number": 42,
    "timestamp": 1703097600,
    "payload_length": 13
  },
  "payload": [72, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100, 33],
  "checksum": 3735928559
}
```

### Serialization Trade-offs

**Advantages**:
- Human-readable for debugging
- Self-describing format
- Language interoperability
- Schema evolution support

**Disadvantages**:
- Higher overhead than binary protocols
- JSON parsing complexity
- Larger frame sizes

**Alternative Considered**: MessagePack or Protocol Buffers for binary efficiency

## Protocol Extensions

### File Transfer Protocol

Large files are segmented into chunks:

```rust
let message = format!("FILE:{}:{}:{}",
                     filename,
                     sequence_number,
                     base64::encode(chunk));
```

**File Transfer State**:
- **Sender**: Tracks bytes sent, chunk sequence
- **Receiver**: Reassembles chunks, detects completion
- **Error Recovery**: Retransmit missing chunks

### Chat Protocol Enhancement

For interactive communication:
- **User identification**: Username in message headers
- **Presence indication**: Periodic ping messages
- **Message threading**: Reply-to sequence numbers

## Performance Optimization

### Buffer Management

The decoder uses bounded buffers to prevent memory exhaustion:

```rust
// Keep buffer size reasonable
if self.buffer.len() > 10000 {
    let keep_size = 5000;
    self.buffer.drain(..self.buffer.len() - keep_size);
    self.state = DecoderState::WaitingForPreamble;
}
```

### Partial Frame Processing

The decoder processes data incrementally:
- **Stream processing**: No need to buffer complete frames
- **Early validation**: Reject invalid frames quickly
- **Memory efficiency**: Bounded buffer sizes

## Security Considerations

### Current Limitations

The protocol currently lacks security features:
- **No encryption**: Messages transmitted in plaintext
- **No authentication**: No sender verification
- **No integrity beyond CRC**: CRC is error detection, not cryptographic

### Future Security Enhancements

1. **Symmetric Encryption**: AES-GCM for confidentiality
2. **Pre-shared Keys**: Simple key distribution
3. **Message Authentication**: HMAC for integrity
4. **Replay Protection**: Sequence number validation

```rust
// Potential security-enhanced message structure
pub struct SecureMessage {
    pub header: MessageHeader,
    pub encrypted_payload: Vec<u8>,  // AES-GCM encrypted
    pub auth_tag: [u8; 16],         // GCM authentication tag
    pub nonce: [u8; 12],            // GCM nonce
}
```

## Testing and Validation

### Protocol Compliance Testing

The test suite validates protocol behavior:

```rust
#[tokio::test]
async fn test_protocol_corruption_recovery() -> UshResult<()> {
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();

    let test_message = "Corruption test message";
    let mut frame_data = encoder.encode_text(test_message)?;

    // Corrupt some bytes in the middle
    let mid_idx = frame_data.len() / 2;
    frame_data[mid_idx] = 0xFF;
    frame_data[mid_idx + 1] = 0x00;

    let messages = decoder.feed_data(&frame_data);

    // Verify corrupted messages are properly rejected
    assert!(messages.is_empty() || !messages[0].verify_checksum().unwrap_or(false));
}
```

### Interoperability Testing

Future versions should include:
- **Cross-platform testing**: Different OS audio stacks
- **Hardware variation**: Different speakers/microphones
- **Environmental testing**: Various noise conditions

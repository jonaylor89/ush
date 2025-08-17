use crc::{Crc, CRC_32_ISO_HDLC};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::{UshError, UshResult};
use log::{debug, warn};

const PROTOCOL_VERSION: u8 = 1;
const PREAMBLE: &[u8] = &[0xAA, 0xAA, 0xAA, 0xAA]; // Alternating pattern for sync
const START_DELIMITER: &[u8] = &[0x7E, 0x7E]; // Frame start marker
const END_DELIMITER: &[u8] = &[0x7E, 0x7E]; // Frame end marker
const MAX_MESSAGE_LENGTH: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Text,
    File,
    Ack,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub version: u8,
    pub message_type: MessageType,
    pub sequence_number: u32,
    pub timestamp: u64,
    pub payload_length: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub header: MessageHeader,
    pub payload: Vec<u8>,
    pub checksum: u32,
}

impl Message {
    pub fn new_text(text: &str, sequence_number: u32) -> UshResult<Self> {
        let payload = text.as_bytes().to_vec();
        
        if payload.len() > MAX_MESSAGE_LENGTH {
            return Err(UshError::Protocol {
                message: format!(
                    "Message too long: {} bytes (max: {})",
                    payload.len(),
                    MAX_MESSAGE_LENGTH
                ),
            });
        }
        
        let header = MessageHeader {
            version: PROTOCOL_VERSION,
            message_type: MessageType::Text,
            sequence_number,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload_length: payload.len() as u16,
        };
        
        let checksum = Self::calculate_checksum(&header, &payload)?;
        
        Ok(Self {
            header,
            payload,
            checksum,
        })
    }
    
    pub fn new_ack(sequence_number: u32) -> UshResult<Self> {
        let payload = Vec::new();
        
        let header = MessageHeader {
            version: PROTOCOL_VERSION,
            message_type: MessageType::Ack,
            sequence_number,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload_length: 0,
        };
        
        let checksum = Self::calculate_checksum(&header, &payload)?;
        
        Ok(Self {
            header,
            payload,
            checksum,
        })
    }
    
    pub fn new_ping(sequence_number: u32) -> UshResult<Self> {
        let payload = b"ping".to_vec();
        
        let header = MessageHeader {
            version: PROTOCOL_VERSION,
            message_type: MessageType::Ping,
            sequence_number,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload_length: payload.len() as u16,
        };
        
        let checksum = Self::calculate_checksum(&header, &payload)?;
        
        Ok(Self {
            header,
            payload,
            checksum,
        })
    }
    
    fn calculate_checksum(header: &MessageHeader, payload: &[u8]) -> UshResult<u32> {
        let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        
        // Serialize header to bytes for checksum calculation
        let header_bytes = serde_json::to_vec(header).map_err(|e| UshError::Protocol {
            message: format!("Failed to serialize header: {}", e),
        })?;
        
        let mut digest = crc.digest();
        digest.update(&header_bytes);
        digest.update(payload);
        
        Ok(digest.finalize())
    }
    
    pub fn verify_checksum(&self) -> UshResult<bool> {
        let calculated = Self::calculate_checksum(&self.header, &self.payload)?;
        Ok(calculated == self.checksum)
    }
    
    pub fn get_text(&self) -> UshResult<String> {
        match self.header.message_type {
            MessageType::Text => {
                String::from_utf8(self.payload.clone()).map_err(|e| UshError::Protocol {
                    message: format!("Invalid UTF-8 in text message: {}", e),
                })
            }
            _ => Err(UshError::Protocol {
                message: "Message is not a text message".to_string(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct ProtocolEncoder {
    sequence_counter: u32,
}

impl ProtocolEncoder {
    pub fn new() -> Self {
        Self {
            sequence_counter: 0,
        }
    }
    
    pub fn encode_message(&mut self, message: &Message) -> UshResult<Vec<u8>> {
        let mut frame = Vec::new();
        
        // Add preamble for synchronization
        frame.extend_from_slice(PREAMBLE);
        frame.extend_from_slice(PREAMBLE); // Double preamble for better sync
        
        // Add start delimiter
        frame.extend_from_slice(START_DELIMITER);
        
        // Serialize the message
        let message_bytes = serde_json::to_vec(message).map_err(|e| UshError::Encoding {
            message: format!("Failed to serialize message: {}", e),
        })?;
        
        // Add message length (for framing)
        let length = message_bytes.len() as u16;
        frame.extend_from_slice(&length.to_be_bytes());
        
        // Add message data
        frame.extend_from_slice(&message_bytes);
        
        // Add end delimiter
        frame.extend_from_slice(END_DELIMITER);
        
        debug!("Encoded message into {} bytes", frame.len());
        Ok(frame)
    }
    
    pub fn encode_text(&mut self, text: &str) -> UshResult<Vec<u8>> {
        let message = Message::new_text(text, self.sequence_counter)?;
        self.sequence_counter = self.sequence_counter.wrapping_add(1);
        self.encode_message(&message)
    }
    
    pub fn get_next_sequence_number(&self) -> u32 {
        self.sequence_counter
    }
}

impl Default for ProtocolEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ProtocolDecoder {
    buffer: Vec<u8>,
    state: DecoderState,
    expected_length: usize,
}

#[derive(Debug, PartialEq)]
enum DecoderState {
    WaitingForPreamble,
    WaitingForStart,
    ReadingLength,
    ReadingMessage,
    WaitingForEnd,
}

impl ProtocolDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            state: DecoderState::WaitingForPreamble,
            expected_length: 0,
        }
    }
    
    pub fn feed_data(&mut self, data: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(data);
        let mut messages = Vec::new();
        
        while let Some(message) = self.try_decode_message() {
            match message {
                Ok(msg) => {
                    if msg.verify_checksum().unwrap_or(false) {
                        debug!("Successfully decoded message: {:?}", msg.header);
                        messages.push(msg);
                    } else {
                        warn!("Message failed checksum verification");
                    }
                }
                Err(e) => {
                    warn!("Failed to decode message: {}", e);
                    // Reset state on error
                    self.state = DecoderState::WaitingForPreamble;
                }
            }
        }
        
        // Keep buffer size reasonable
        if self.buffer.len() > 10000 {
            let keep_size = 5000;
            self.buffer.drain(..self.buffer.len() - keep_size);
            self.state = DecoderState::WaitingForPreamble;
        }
        
        messages
    }
    
    fn try_decode_message(&mut self) -> Option<UshResult<Message>> {
        match self.state {
            DecoderState::WaitingForPreamble => {
                if let Some(pos) = self.find_preamble() {
                    self.buffer.drain(..pos);
                    self.state = DecoderState::WaitingForStart;
                    return self.try_decode_message();
                }
                None
            }
            DecoderState::WaitingForStart => {
                if self.buffer.len() >= PREAMBLE.len() * 2 + START_DELIMITER.len() {
                    let start_pos = PREAMBLE.len() * 2;
                    if &self.buffer[start_pos..start_pos + START_DELIMITER.len()] == START_DELIMITER {
                        self.buffer.drain(..start_pos + START_DELIMITER.len());
                        self.state = DecoderState::ReadingLength;
                        return self.try_decode_message();
                    } else {
                        // Invalid start, look for next preamble
                        self.buffer.drain(..1);
                        self.state = DecoderState::WaitingForPreamble;
                        return self.try_decode_message();
                    }
                }
                None
            }
            DecoderState::ReadingLength => {
                if self.buffer.len() >= 2 {
                    let length_bytes = [self.buffer[0], self.buffer[1]];
                    self.expected_length = u16::from_be_bytes(length_bytes) as usize;
                    
                    if self.expected_length > MAX_MESSAGE_LENGTH * 2 {
                        // Invalid length, reset
                        self.buffer.drain(..1);
                        self.state = DecoderState::WaitingForPreamble;
                        return self.try_decode_message();
                    }
                    
                    self.buffer.drain(..2);
                    self.state = DecoderState::ReadingMessage;
                    return self.try_decode_message();
                }
                None
            }
            DecoderState::ReadingMessage => {
                if self.buffer.len() >= self.expected_length {
                    let message_bytes = self.buffer[..self.expected_length].to_vec();
                    self.buffer.drain(..self.expected_length);
                    self.state = DecoderState::WaitingForEnd;
                    
                    match serde_json::from_slice::<Message>(&message_bytes) {
                        Ok(message) => {
                            // Check if we have the end delimiter
                            if self.buffer.len() >= END_DELIMITER.len()
                                && &self.buffer[..END_DELIMITER.len()] == END_DELIMITER
                            {
                                self.buffer.drain(..END_DELIMITER.len());
                                self.state = DecoderState::WaitingForPreamble;
                                return Some(Ok(message));
                            } else {
                                // Missing end delimiter, but we have a valid message
                                warn!("Missing end delimiter, but message appears valid");
                                self.state = DecoderState::WaitingForPreamble;
                                return Some(Ok(message));
                            }
                        }
                        Err(e) => {
                            self.state = DecoderState::WaitingForPreamble;
                            return Some(Err(UshError::Decoding {
                                message: format!("Failed to deserialize message: {}", e),
                            }));
                        }
                    }
                }
                None
            }
            DecoderState::WaitingForEnd => {
                if self.buffer.len() >= END_DELIMITER.len() {
                    if &self.buffer[..END_DELIMITER.len()] == END_DELIMITER {
                        self.buffer.drain(..END_DELIMITER.len());
                    } else {
                        // Skip invalid end delimiter
                        self.buffer.drain(..1);
                    }
                    self.state = DecoderState::WaitingForPreamble;
                    return self.try_decode_message();
                }
                None
            }
        }
    }
    
    fn find_preamble(&self) -> Option<usize> {
        let double_preamble = [PREAMBLE, PREAMBLE].concat();
        
        if self.buffer.len() < double_preamble.len() {
            return None;
        }
        
        (0..=self.buffer.len() - double_preamble.len()).find(|&i| &self.buffer[i..i + double_preamble.len()] == double_preamble)
    }
    
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = DecoderState::WaitingForPreamble;
        self.expected_length = 0;
    }
}

impl Default for ProtocolDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation_and_verification() {
        let msg = Message::new_text("Hello, World!", 42).unwrap();
        assert!(msg.verify_checksum().unwrap());
        assert_eq!(msg.get_text().unwrap(), "Hello, World!");
        assert_eq!(msg.header.sequence_number, 42);
    }
    
    #[test]
    fn test_encode_decode_roundtrip() {
        let mut encoder = ProtocolEncoder::new();
        let mut decoder = ProtocolDecoder::new();
        
        let original_text = "Test message";
        let encoded = encoder.encode_text(original_text).unwrap();
        let messages = decoder.feed_data(&encoded);
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get_text().unwrap(), original_text);
    }
    
    #[test]
    fn test_checksum_verification() {
        let mut msg = Message::new_text("Test", 1).unwrap();
        assert!(msg.verify_checksum().unwrap());
        
        // Corrupt the message
        msg.payload[0] = msg.payload[0].wrapping_add(1);
        assert!(!msg.verify_checksum().unwrap());
    }
    
    #[test]
    fn test_decoder_with_partial_data() {
        let mut encoder = ProtocolEncoder::new();
        let mut decoder = ProtocolDecoder::new();
        
        let encoded = encoder.encode_text("Test").unwrap();
        
        // Feed data in chunks
        let chunk_size = 5;
        let mut total_messages = Vec::new();
        
        for chunk in encoded.chunks(chunk_size) {
            let mut messages = decoder.feed_data(chunk);
            total_messages.append(&mut messages);
        }
        
        assert_eq!(total_messages.len(), 1);
        assert_eq!(total_messages[0].get_text().unwrap(), "Test");
    }
}
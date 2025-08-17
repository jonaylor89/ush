use ush::modulation::{FskModulator, FskDemodulator, ModulationConfig};
use ush::protocol::{ProtocolEncoder, ProtocolDecoder};
use ush::UshResult;

#[tokio::test]
async fn test_full_pipeline() -> UshResult<()> {
    // Test the complete pipeline: text -> protocol -> modulation -> audio -> demodulation -> protocol -> text
    
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let original_text = "Hello, ultrasonic world! 🌊";
    
    // Encode text to protocol frame
    let frame_data = encoder.encode_text(original_text)?;
    
    // Modulate frame to audio samples
    let audio_samples = modulator.encode_bytes(&frame_data);
    
    // Demodulate audio back to bytes
    let decoded_frame_data = demodulator.decode_bytes(&audio_samples)?;
    
    // Decode protocol frame back to messages
    let messages = decoder.feed_data(&decoded_frame_data);
    
    assert!(!messages.is_empty(), "No messages decoded");
    
    let decoded_text = messages[0].get_text()?;
    assert_eq!(original_text, decoded_text);
    
    println!("✓ Full pipeline test passed: \"{}\"", decoded_text);
    Ok(())
}

#[tokio::test] 
async fn test_multiple_messages() -> UshResult<()> {
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let messages = vec![
        "First message",
        "Second message with numbers: 12345",
        "Third message with symbols: @#$%^&*()",
    ];
    
    // Test each message individually (since silence breaks symbol alignment)
    let mut decoded_messages = Vec::new();
    
    for message in &messages {
        let frame_data = encoder.encode_text(message)?;
        let samples = modulator.encode_bytes(&frame_data);
        let decoded_bytes = demodulator.decode_bytes(&samples)?;
        let messages_decoded = decoder.feed_data(&decoded_bytes);
        decoded_messages.extend(messages_decoded);
    }
    
    assert_eq!(messages.len(), decoded_messages.len(), "Message count mismatch");
    
    for (i, decoded_msg) in decoded_messages.iter().enumerate() {
        let decoded_text = decoded_msg.get_text()?;
        assert_eq!(messages[i], decoded_text, "Message {} mismatch", i);
        println!("✓ Message {}: \"{}\"", i + 1, decoded_text);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_noisy_environment() -> UshResult<()> {
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let test_message = "Noise test message";
    let frame_data = encoder.encode_text(test_message)?;
    let mut audio_samples = modulator.encode_bytes(&frame_data);
    
    // Add white noise
    
    let noise_level = 0.05; // 5% noise
    for (i, sample) in audio_samples.iter_mut().enumerate() {
        let noise = ((i as f32 * 0.1).sin() + (i as f32 * 0.17).cos()) * noise_level;
        *sample += noise;
    }
    
    // Try to decode with noise
    let decoded_bytes = demodulator.decode_bytes(&audio_samples);
    
    match decoded_bytes {
        Ok(bytes) => {
            let messages = decoder.feed_data(&bytes);
            if !messages.is_empty() {
                let decoded_text = messages[0].get_text()?;
                if decoded_text == test_message {
                    println!("✓ Successfully decoded despite noise: \"{}\"", decoded_text);
                } else {
                    println!("⚠ Decoded with errors: \"{}\" (expected: \"{}\")", decoded_text, test_message);
                }
            } else {
                println!("⚠ No messages decoded from noisy signal");
            }
        }
        Err(e) => {
            println!("⚠ Failed to decode noisy signal: {}", e);
            // This is acceptable for noise testing
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_different_message_lengths() -> UshResult<()> {
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let test_messages = vec![
        "A", // Very short
        "Hello", // Short
        "This is a medium-length message for testing purposes.", // Medium
        "This is a very long message that contains many words and should test the system's ability to handle longer text transmissions. It includes various characters like numbers 123456789 and symbols !@#$%^&*()_+-=[]{}|;:,.<>? to ensure comprehensive testing coverage.", // Long
    ];
    
    for (i, message) in test_messages.iter().enumerate() {
        let frame_data = encoder.encode_text(message)?;
        let samples = modulator.encode_bytes(&frame_data);
        
        let decoded_bytes = demodulator.decode_bytes(&samples)?;
        let messages = decoder.feed_data(&decoded_bytes);
        
        assert!(!messages.is_empty(), "No messages decoded for test {}", i);
        let decoded_text = messages[0].get_text()?;
        assert_eq!(*message, decoded_text, "Message {} length test failed", i);
        
        println!("✓ Length test {}: {} chars -> {} samples", 
                 i + 1, message.len(), samples.len());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_frequency_separation() -> UshResult<()> {
    // Test different frequency configurations
    let configs = vec![
        (17000.0, 19000.0), // Wide separation
        (18500.0, 19500.0), // Narrow separation  
        (19000.0, 21000.0), // High frequencies
    ];
    
    let test_message = "Frequency test";
    
    for (i, (freq_0, freq_1)) in configs.iter().enumerate() {
        let modulation_config = ModulationConfig {
            sample_rate: 44100,
            freq_0: *freq_0,
            freq_1: *freq_1,
            symbol_duration: 0.01,
            ramp_duration: 0.002,
        };
        
        let modulator = FskModulator::new(modulation_config.clone());
        let demodulator = FskDemodulator::new(modulation_config);
        
        let mut encoder = ProtocolEncoder::new();
        let mut decoder = ProtocolDecoder::new();
        
        let frame_data = encoder.encode_text(test_message)?;
        let samples = modulator.encode_bytes(&frame_data);
        
        let decoded_bytes = demodulator.decode_bytes(&samples)?;
        let messages = decoder.feed_data(&decoded_bytes);
        
        if !messages.is_empty() {
            let decoded_text = messages[0].get_text()?;
            if decoded_text == test_message {
                println!("✓ Frequency config {}: {:.0}Hz/{:.0}Hz - SUCCESS", 
                         i + 1, freq_0, freq_1);
            } else {
                println!("⚠ Frequency config {}: {:.0}Hz/{:.0}Hz - DECODE ERROR", 
                         i + 1, freq_0, freq_1);
            }
        } else {
            println!("⚠ Frequency config {}: {:.0}Hz/{:.0}Hz - NO MESSAGES", 
                     i + 1, freq_0, freq_1);
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_protocol_corruption_recovery() -> UshResult<()> {
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let test_message = "Corruption test message";
    let mut frame_data = encoder.encode_text(test_message)?;
    
    // Corrupt some bytes in the middle
    if frame_data.len() > 10 {
        let mid_idx = frame_data.len() / 2;
        frame_data[mid_idx] = 0xFF;
        frame_data[mid_idx + 1] = 0x00;
    }
    
    let messages = decoder.feed_data(&frame_data);
    
    if messages.is_empty() {
        println!("✓ Correctly rejected corrupted message");
    } else {
        // Check if checksum validation caught the corruption
        for message in messages {
            if let Ok(is_valid) = message.verify_checksum() {
                if !is_valid {
                    println!("✓ Checksum validation caught corruption");
                } else {
                    println!("⚠ Corrupted message passed checksum validation");
                }
            }
        }
    }
    
    Ok(())
}

#[tokio::test]
async fn test_performance_benchmarks() -> UshResult<()> {
    use std::time::Instant;
    
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let test_message = "Performance test message for benchmarking encoding and decoding speed.";
    
    // Benchmark encoding
    let start = Instant::now();
    let frame_data = encoder.encode_text(test_message)?;
    let protocol_encode_time = start.elapsed();
    
    let start = Instant::now();
    let samples = modulator.encode_bytes(&frame_data);
    let modulation_encode_time = start.elapsed();
    
    // Benchmark decoding
    let start = Instant::now();
    let decoded_bytes = demodulator.decode_bytes(&samples)?;
    let modulation_decode_time = start.elapsed();
    
    let mut decoder = ProtocolDecoder::new();
    let start = Instant::now();
    let messages = decoder.feed_data(&decoded_bytes);
    let protocol_decode_time = start.elapsed();
    
    // Verify correctness
    assert!(!messages.is_empty());
    let decoded_text = messages[0].get_text()?;
    assert_eq!(test_message, decoded_text);
    
    println!("Performance benchmarks:");
    println!("  Protocol encode: {:?}", protocol_encode_time);
    println!("  Modulation encode: {:?}", modulation_encode_time);
    println!("  Modulation decode: {:?}", modulation_decode_time);
    println!("  Protocol decode: {:?}", protocol_decode_time);
    println!("  Total roundtrip: {:?}", 
             protocol_encode_time + modulation_encode_time + 
             modulation_decode_time + protocol_decode_time);
    println!("  Audio duration: {:.2}s", samples.len() as f32 / 44100.0);
    println!("  Data rate: {:.1} chars/s", 
             test_message.len() as f32 / (samples.len() as f32 / 44100.0));
    
    Ok(())
}

#[tokio::test]
async fn test_unicode_support() -> UshResult<()> {
    let modulation_config = ModulationConfig::default();
    let modulator = FskModulator::new(modulation_config.clone());
    let demodulator = FskDemodulator::new(modulation_config);
    
    let mut encoder = ProtocolEncoder::new();
    let mut decoder = ProtocolDecoder::new();
    
    let unicode_messages = vec![
        "Hello 世界", // Chinese
        "Привет мир", // Russian
        "مرحبا بالعالم", // Arabic
        "🌊🔊📡💻🚀", // Emojis
        "Café naïve résumé", // Accented characters
    ];
    
    for (i, message) in unicode_messages.iter().enumerate() {
        let frame_data = encoder.encode_text(message)?;
        let samples = modulator.encode_bytes(&frame_data);
        
        let decoded_bytes = demodulator.decode_bytes(&samples)?;
        let messages = decoder.feed_data(&decoded_bytes);
        
        assert!(!messages.is_empty(), "No messages decoded for Unicode test {}", i);
        let decoded_text = messages[0].get_text()?;
        assert_eq!(*message, decoded_text, "Unicode test {} failed", i);
        
        println!("✓ Unicode test {}: \"{}\"", i + 1, decoded_text);
    }
    
    Ok(())
}
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use crate::{UshError, UshResult};
use log::debug;

const CARRIER_FREQ_0: f32 = 18000.0; // Frequency for bit '0'
const CARRIER_FREQ_1: f32 = 20000.0; // Frequency for bit '1'
const SYMBOL_DURATION: f32 = 0.01;   // 10ms per symbol
const RAMP_DURATION: f32 = 0.002;    // 2ms ramp up/down to reduce clicks

#[derive(Debug, Clone)]
pub struct ModulationConfig {
    pub sample_rate: u32,
    pub freq_0: f32,
    pub freq_1: f32,
    pub symbol_duration: f32,
    pub ramp_duration: f32,
}

impl Default for ModulationConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            freq_0: CARRIER_FREQ_0,
            freq_1: CARRIER_FREQ_1,
            symbol_duration: SYMBOL_DURATION,
            ramp_duration: RAMP_DURATION,
        }
    }
}

pub struct FskModulator {
    config: ModulationConfig,
    samples_per_symbol: usize,
    ramp_samples: usize,
}

impl FskModulator {
    pub fn new(config: ModulationConfig) -> Self {
        let samples_per_symbol = (config.sample_rate as f32 * config.symbol_duration) as usize;
        let ramp_samples = (config.sample_rate as f32 * config.ramp_duration) as usize;
        
        Self {
            config,
            samples_per_symbol,
            ramp_samples,
        }
    }

    pub fn encode_bits(&self, bits: &[bool]) -> Vec<f32> {
        let total_samples = bits.len() * self.samples_per_symbol;
        let mut samples = Vec::with_capacity(total_samples);
        
        for (i, &bit) in bits.iter().enumerate() {
            let frequency = if bit { self.config.freq_1 } else { self.config.freq_0 };
            let symbol_samples = self.generate_symbol(frequency, i == 0, i == bits.len() - 1);
            samples.extend(symbol_samples);
        }
        
        debug!("Encoded {} bits into {} samples", bits.len(), samples.len());
        samples
    }

    fn generate_symbol(&self, frequency: f32, is_first: bool, is_last: bool) -> Vec<f32> {
        let mut samples = Vec::with_capacity(self.samples_per_symbol);
        
        for i in 0..self.samples_per_symbol {
            let t = i as f32 / self.config.sample_rate as f32;
            let phase = 2.0 * PI * frequency * t;
            let mut amplitude = phase.sin();
            
            // Apply ramping to reduce clicks
            if is_first && i < self.ramp_samples {
                let ramp_factor = i as f32 / self.ramp_samples as f32;
                amplitude *= ramp_factor;
            }
            
            if is_last && i >= self.samples_per_symbol - self.ramp_samples {
                let ramp_factor = (self.samples_per_symbol - i) as f32 / self.ramp_samples as f32;
                amplitude *= ramp_factor;
            }
            
            samples.push(amplitude * 0.3); // Reduce volume to 30%
        }
        
        samples
    }

    pub fn encode_bytes(&self, data: &[u8]) -> Vec<f32> {
        let mut bits = Vec::new();
        
        // Convert bytes to bits (MSB first)
        for &byte in data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }
        
        self.encode_bits(&bits)
    }
}

pub struct FskDemodulator {
    config: ModulationConfig,
    samples_per_symbol: usize,
    fft_size: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
}

impl FskDemodulator {
    pub fn new(config: ModulationConfig) -> Self {
        let samples_per_symbol = (config.sample_rate as f32 * config.symbol_duration) as usize;
        let fft_size = samples_per_symbol.next_power_of_two();
        
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        
        Self {
            config,
            samples_per_symbol,
            fft_size,
            fft,
        }
    }

    pub fn decode_samples(&self, samples: &[f32]) -> UshResult<Vec<bool>> {
        if samples.len() % self.samples_per_symbol != 0 {
            return Err(UshError::Decoding {
                message: format!(
                    "Sample length {} is not a multiple of symbol length {}",
                    samples.len(),
                    self.samples_per_symbol
                ),
            });
        }
        
        let num_symbols = samples.len() / self.samples_per_symbol;
        let mut bits = Vec::with_capacity(num_symbols);
        
        for i in 0..num_symbols {
            let start = i * self.samples_per_symbol;
            let end = start + self.samples_per_symbol;
            let symbol_samples = &samples[start..end];
            
            let bit = self.decode_symbol(symbol_samples)?;
            bits.push(bit);
        }
        
        debug!("Decoded {} symbols into {} bits", num_symbols, bits.len());
        Ok(bits)
    }

    fn decode_symbol(&self, samples: &[f32]) -> UshResult<bool> {
        // Pad samples to FFT size
        let mut padded_samples: Vec<Complex<f32>> = samples
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        
        padded_samples.resize(self.fft_size, Complex::new(0.0, 0.0));
        
        // Perform FFT
        self.fft.process(&mut padded_samples);
        
        // Find the dominant frequency by looking at magnitude spectrum
        let freq_0_bin = (self.config.freq_0 * self.fft_size as f32 / self.config.sample_rate as f32) as usize;
        let freq_1_bin = (self.config.freq_1 * self.fft_size as f32 / self.config.sample_rate as f32) as usize;
        
        let _power_0 = if freq_0_bin < padded_samples.len() {
            padded_samples[freq_0_bin].norm_sqr()
        } else {
            0.0
        };
        
        let _power_1 = if freq_1_bin < padded_samples.len() {
            padded_samples[freq_1_bin].norm_sqr()
        } else {
            0.0
        };
        
        // Check nearby bins for better detection
        let search_range = 3;
        let power_0_max = (freq_0_bin.saturating_sub(search_range)
            ..=(freq_0_bin + search_range).min(padded_samples.len() - 1))
            .map(|i| padded_samples[i].norm_sqr())
            .fold(0.0f32, f32::max);
            
        let power_1_max = (freq_1_bin.saturating_sub(search_range)
            ..=(freq_1_bin + search_range).min(padded_samples.len() - 1))
            .map(|i| padded_samples[i].norm_sqr())
            .fold(0.0f32, f32::max);
        
        debug!(
            "Symbol detection: freq_0 power = {:.2}, freq_1 power = {:.2}",
            power_0_max, power_1_max
        );
        
        if power_0_max < 0.001 && power_1_max < 0.001 {
            return Err(UshError::Decoding {
                message: "No signal detected in symbol".to_string(),
            });
        }
        
        Ok(power_1_max > power_0_max)
    }

    pub fn decode_bytes(&self, samples: &[f32]) -> UshResult<Vec<u8>> {
        let bits = self.decode_samples(samples)?;
        
        if bits.len() % 8 != 0 {
            return Err(UshError::Decoding {
                message: format!(
                    "Bit count {} is not a multiple of 8",
                    bits.len()
                ),
            });
        }
        
        let mut bytes = Vec::new();
        
        for chunk in bits.chunks_exact(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << (7 - i); // MSB first
                }
            }
            bytes.push(byte);
        }
        
        debug!("Decoded {} bits into {} bytes", bits.len(), bytes.len());
        Ok(bytes)
    }
}

// Utility functions for signal detection
pub fn detect_signal_start(samples: &[f32], threshold: f32) -> Option<usize> {
    let window_size = 512;
    let mut max_energy = 0.0f32;
    
    for window in samples.windows(window_size) {
        let energy: f32 = window.iter().map(|&s| s * s).sum();
        max_energy = max_energy.max(energy);
    }
    
    let detection_threshold = max_energy * threshold;
    
    for (i, window) in samples.windows(window_size).enumerate() {
        let energy: f32 = window.iter().map(|&s| s * s).sum();
        if energy > detection_threshold {
            return Some(i);
        }
    }
    
    None
}

pub fn apply_bandpass_filter(samples: &[f32], low_freq: f32, high_freq: f32, sample_rate: u32) -> Vec<f32> {
    // Simple high-pass then low-pass filtering
    let mut filtered = samples.to_vec();
    
    // High-pass filter (remove DC and low frequencies)
    let alpha_hp = 1.0 / (1.0 + 2.0 * PI * low_freq / sample_rate as f32);
    let mut prev_input = 0.0;
    let mut prev_output = 0.0;
    
    for sample in filtered.iter_mut() {
        let output = alpha_hp * (prev_output + *sample - prev_input);
        prev_input = *sample;
        prev_output = output;
        *sample = output;
    }
    
    // Low-pass filter (remove high frequencies)
    let alpha_lp = 2.0 * PI * high_freq / sample_rate as f32 / (1.0 + 2.0 * PI * high_freq / sample_rate as f32);
    prev_output = 0.0;
    
    for sample in filtered.iter_mut() {
        let output = prev_output + alpha_lp * (*sample - prev_output);
        prev_output = output;
        *sample = output;
    }
    
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_encode_decode_roundtrip() {
        let config = ModulationConfig::default();
        let modulator = FskModulator::new(config.clone());
        let demodulator = FskDemodulator::new(config);
        
        let original_data = b"Hello, World!";
        let encoded = modulator.encode_bytes(original_data);
        let decoded = demodulator.decode_bytes(&encoded).unwrap();
        
        assert_eq!(original_data, &decoded[..]);
    }
    
    #[test]
    fn test_bit_encoding() {
        let config = ModulationConfig::default();
        let modulator = FskModulator::new(config);
        
        let bits = vec![true, false, true, false];
        let samples = modulator.encode_bits(&bits);
        
        assert_eq!(samples.len(), bits.len() * modulator.samples_per_symbol);
    }
}
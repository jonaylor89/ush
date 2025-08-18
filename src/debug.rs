//! Debug analysis module for spectrograms and FFT visualization
//!
//! This module provides comprehensive audio analysis capabilities for the ush system,
//! generating spectrograms, FFT plots, and detailed signal analysis reports.

use crate::{UshError, UshResult};
use colorgrad::viridis;
use image::{ImageBuffer, Rgb, RgbImage};
use log::{debug, info, warn};
use plotters::prelude::*;
use rustfft::{FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for debug analysis
#[derive(Debug, Clone)]
pub struct DebugConfig {
    pub sample_rate: u32,
    pub freq_0: f32, // FSK frequency for '0'
    pub freq_1: f32, // FSK frequency for '1'
    pub output_dir: PathBuf,
    pub window_size: usize,
    pub hop_size: usize,
    pub fft_size: usize,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            freq_0: 18000.0,
            freq_1: 20000.0,
            output_dir: PathBuf::from("debug_analysis"),
            window_size: 1024,
            hop_size: 256,
            fft_size: 512,
        }
    }
}

/// Spectrogram data structure
#[derive(Debug)]
pub struct SpectrogramData {
    pub magnitude_data: Vec<Vec<f32>>, // [time][frequency]
    pub time_resolution: f32,          // seconds per time bin
    pub freq_resolution: f32,          // Hz per frequency bin
    pub max_magnitude: f32,
    pub min_magnitude: f32,
}

/// FFT analysis result
#[derive(Debug, Serialize, Deserialize)]
pub struct FftAnalysis {
    pub frequencies: Vec<f32>,
    pub magnitudes: Vec<f32>,
    pub peak_frequency: f32,
    pub peak_magnitude: f32,
    pub freq_0_power: f32,
    pub freq_1_power: f32,
    pub snr_estimate: f32,
}

/// Signal quality metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct SignalMetrics {
    pub duration_seconds: f32,
    pub samples_count: usize,
    pub rms_level: f32,
    pub peak_level: f32,
    pub dynamic_range: f32,
    pub estimated_snr: f32,
    pub freq_0_presence: f32,      // 0.0 to 1.0
    pub freq_1_presence: f32,      // 0.0 to 1.0
    pub signal_activity: Vec<f32>, // Activity level over time
}

/// Complete debug analysis session
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugAnalysis {
    pub session_id: String,
    pub timestamp: u64,
    pub config: DebugConfigJson,
    pub signal_metrics: SignalMetrics,
    pub fft_analysis: FftAnalysis,
    pub files_generated: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugConfigJson {
    pub sample_rate: u32,
    pub freq_0: f32,
    pub freq_1: f32,
    pub window_size: usize,
    pub hop_size: usize,
    pub fft_size: usize,
}

/// Audio buffer for continuous recording during debug session
pub struct DebugAudioBuffer {
    buffer: Arc<Mutex<VecDeque<f32>>>,
    max_duration_seconds: f32,
    sample_rate: u32,
}

impl DebugAudioBuffer {
    pub fn new(max_duration_seconds: f32, sample_rate: u32) -> Self {
        let max_samples = (max_duration_seconds * sample_rate as f32) as usize;
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(max_samples))),
            max_duration_seconds,
            sample_rate,
        }
    }

    pub fn get_buffer_clone(&self) -> Arc<Mutex<VecDeque<f32>>> {
        self.buffer.clone()
    }

    pub fn add_samples(&self, samples: &[f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        let max_samples = (self.max_duration_seconds * self.sample_rate as f32) as usize;

        // Add new samples
        buffer.extend(samples.iter());

        // Remove old samples if we exceed max duration
        while buffer.len() > max_samples {
            buffer.pop_front();
        }
    }

    pub fn get_all_samples(&self) -> Vec<f32> {
        self.buffer.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }
}

/// Main debug analyzer
pub struct DebugAnalyzer {
    config: DebugConfig,
    session_id: String,
    fft_planner: FftPlanner<f32>,
}

impl DebugAnalyzer {
    pub fn new(config: DebugConfig) -> UshResult<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let session_id = format!("session_{}", timestamp);

        // Create output directory
        let session_dir = config.output_dir.join(&session_id);
        fs::create_dir_all(&session_dir).map_err(|e| UshError::Io(e))?;

        info!("Debug session started: {}", session_id);
        info!("Output directory: {:?}", session_dir);

        Ok(Self {
            config,
            session_id,
            fft_planner: FftPlanner::new(),
        })
    }

    /// Generate complete debug analysis from audio samples
    pub fn analyze_audio(&mut self, samples: &[f32]) -> UshResult<DebugAnalysis> {
        info!("Starting debug analysis of {} samples", samples.len());

        let session_dir = self.config.output_dir.join(&self.session_id);

        // 1. Save raw audio
        let raw_audio_path = session_dir.join("raw_audio.wav");
        self.save_wav_file(samples, &raw_audio_path)?;

        // 2. Generate signal metrics
        let signal_metrics = self.calculate_signal_metrics(samples);

        // 3. Generate spectrogram
        let spectrogram = self.generate_spectrogram(samples)?;
        let spectrogram_path = session_dir.join("spectrogram.png");
        self.save_spectrogram(&spectrogram, &spectrogram_path)?;

        // 4. Generate FFT analysis
        let fft_analysis = self.analyze_full_spectrum(samples)?;
        let fft_path = session_dir.join("full_spectrum_fft.png");
        self.save_fft_plot(&fft_analysis, &fft_path, "Full Spectrum FFT")?;

        // 5. Generate communication band analysis
        let comm_band_fft = self.analyze_communication_band(samples)?;
        let comm_fft_path = session_dir.join("communication_band_fft.png");
        self.save_fft_plot(
            &comm_band_fft,
            &comm_fft_path,
            "Communication Band (15-25 kHz)",
        )?;

        // 6. Generate time-segmented FFTs
        self.generate_time_segmented_ffts(samples, &session_dir)?;

        // 7. Generate analysis report
        let analysis = DebugAnalysis {
            session_id: self.session_id.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            config: DebugConfigJson {
                sample_rate: self.config.sample_rate,
                freq_0: self.config.freq_0,
                freq_1: self.config.freq_1,
                window_size: self.config.window_size,
                hop_size: self.config.hop_size,
                fft_size: self.config.fft_size,
            },
            signal_metrics,
            fft_analysis,
            files_generated: vec![
                "raw_audio.wav".to_string(),
                "spectrogram.png".to_string(),
                "full_spectrum_fft.png".to_string(),
                "communication_band_fft.png".to_string(),
                "time_segmented_ffts.png".to_string(),
                "analysis_report.json".to_string(),
                "debug_report.html".to_string(),
            ],
        };

        // 8. Save analysis report
        let analysis_path = session_dir.join("analysis_report.json");
        let analysis_json =
            serde_json::to_string_pretty(&analysis).map_err(|e| UshError::Config {
                message: format!("JSON serialization error: {}", e),
            })?;
        fs::write(&analysis_path, analysis_json).map_err(|e| UshError::Io(e))?;

        // 9. Generate HTML report
        self.generate_html_report(&analysis, &session_dir)?;

        info!("Debug analysis complete. Files saved to: {:?}", session_dir);
        Ok(analysis)
    }

    /// Calculate comprehensive signal metrics
    fn calculate_signal_metrics(&mut self, samples: &[f32]) -> SignalMetrics {
        let duration = samples.len() as f32 / self.config.sample_rate as f32;

        // RMS and peak levels
        let rms = (samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // Dynamic range (peak to RMS ratio in dB)
        let dynamic_range = if rms > 0.0 {
            20.0 * (peak / rms).log10()
        } else {
            0.0
        };

        // Analyze frequency content for FSK detection
        let (freq_0_presence, freq_1_presence, snr_estimate) = self.analyze_fsk_presence(samples);

        // Activity level over time (divide into 1-second segments)
        let segment_samples = self.config.sample_rate as usize;
        let mut activity = Vec::new();

        for chunk in samples.chunks(segment_samples) {
            let chunk_rms = (chunk.iter().map(|&s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
            activity.push(chunk_rms);
        }

        SignalMetrics {
            duration_seconds: duration,
            samples_count: samples.len(),
            rms_level: rms,
            peak_level: peak,
            dynamic_range,
            estimated_snr: snr_estimate,
            freq_0_presence,
            freq_1_presence,
            signal_activity: activity,
        }
    }

    /// Analyze FSK frequency presence
    fn analyze_fsk_presence(&mut self, samples: &[f32]) -> (f32, f32, f32) {
        let fft_size = 4096; // Larger FFT for better frequency resolution
        let fft = self.fft_planner.plan_fft_forward(fft_size);

        let mut fft_input: Vec<Complex<f32>> = samples[..fft_size.min(samples.len())]
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        fft_input.resize(fft_size, Complex::new(0.0, 0.0));

        fft.process(&mut fft_input);

        let freq_resolution = self.config.sample_rate as f32 / fft_size as f32;

        // Find bins for FSK frequencies
        let freq_0_bin = (self.config.freq_0 / freq_resolution) as usize;
        let freq_1_bin = (self.config.freq_1 / freq_resolution) as usize;

        // Measure power in frequency bands (±3 bins for robustness)
        let freq_0_power = self.measure_bin_power(&fft_input, freq_0_bin, 3);
        let freq_1_power = self.measure_bin_power(&fft_input, freq_1_bin, 3);

        // Estimate noise floor (average power excluding signal bands)
        let noise_power = self.estimate_noise_floor(&fft_input, &[freq_0_bin, freq_1_bin]);

        // Calculate presence indicators (0.0 to 1.0)
        let max_power = freq_0_power.max(freq_1_power);
        let freq_0_presence = if max_power > 0.0 {
            freq_0_power / max_power
        } else {
            0.0
        };
        let freq_1_presence = if max_power > 0.0 {
            freq_1_power / max_power
        } else {
            0.0
        };

        // Estimate SNR
        let signal_power = freq_0_power + freq_1_power;
        let snr_linear = if noise_power > 0.0 {
            signal_power / noise_power
        } else {
            0.0
        };
        let snr_db = if snr_linear > 0.0 {
            10.0 * snr_linear.log10()
        } else {
            -60.0
        };

        (freq_0_presence, freq_1_presence, snr_db)
    }

    /// Measure power in a frequency bin with surrounding bins
    fn measure_bin_power(
        &self,
        fft_result: &[Complex<f32>],
        center_bin: usize,
        range: usize,
    ) -> f32 {
        (center_bin.saturating_sub(range)..=(center_bin + range).min(fft_result.len() - 1))
            .map(|i| fft_result[i].norm_sqr())
            .fold(0.0f32, f32::max)
    }

    /// Estimate noise floor excluding signal bins
    fn estimate_noise_floor(&self, fft_result: &[Complex<f32>], exclude_bins: &[usize]) -> f32 {
        let mut total_power = 0.0;
        let mut count = 0;

        for (i, sample) in fft_result.iter().enumerate() {
            // Skip DC, high frequencies, and signal bins
            if i > 10 && i < fft_result.len() / 2 && !exclude_bins.contains(&i) {
                total_power += sample.norm_sqr();
                count += 1;
            }
        }

        if count > 0 {
            total_power / count as f32
        } else {
            0.0
        }
    }

    /// Generate spectrogram using STFT
    fn generate_spectrogram(&mut self, samples: &[f32]) -> UshResult<SpectrogramData> {
        info!("Generating spectrogram...");

        let window_size = self.config.window_size;
        let hop_size = self.config.hop_size;
        let fft_size = window_size; // Use same size as window

        let fft = self.fft_planner.plan_fft_forward(fft_size);
        let mut magnitude_data = Vec::new();

        let mut max_magnitude = 0.0f32;
        let mut min_magnitude = f32::INFINITY;

        // Apply Hamming window
        let window: Vec<f32> = (0..window_size)
            .map(|i| {
                0.54 - 0.46
                    * (2.0 * std::f32::consts::PI * i as f32 / (window_size - 1) as f32).cos()
            })
            .collect();

        for start in (0..samples.len()).step_by(hop_size) {
            let end = (start + window_size).min(samples.len());
            if end - start < window_size {
                break; // Skip incomplete windows
            }

            // Apply window function and prepare FFT input
            let mut fft_input: Vec<Complex<f32>> = samples[start..end]
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();

            fft.process(&mut fft_input);

            // Calculate magnitude spectrum (only positive frequencies)
            let magnitudes: Vec<f32> = fft_input[..fft_size / 2]
                .iter()
                .map(|c| c.norm().log10() * 20.0) // Convert to dB
                .collect();

            // Update min/max for normalization
            for &mag in &magnitudes {
                if mag.is_finite() {
                    max_magnitude = max_magnitude.max(mag);
                    min_magnitude = min_magnitude.min(mag);
                }
            }

            magnitude_data.push(magnitudes);
        }

        let time_resolution = hop_size as f32 / self.config.sample_rate as f32;
        let freq_resolution = self.config.sample_rate as f32 / fft_size as f32;

        info!(
            "Spectrogram generated: {} time bins, {} frequency bins",
            magnitude_data.len(),
            if magnitude_data.is_empty() {
                0
            } else {
                magnitude_data[0].len()
            }
        );

        Ok(SpectrogramData {
            magnitude_data,
            time_resolution,
            freq_resolution,
            max_magnitude,
            min_magnitude: if min_magnitude.is_finite() {
                min_magnitude
            } else {
                -60.0
            },
        })
    }

    /// Save spectrogram as PNG image
    fn save_spectrogram(&self, spectrogram: &SpectrogramData, path: &Path) -> UshResult<()> {
        info!("Saving spectrogram to: {:?}", path);

        if spectrogram.magnitude_data.is_empty() {
            warn!("No spectrogram data to save");
            return Ok(());
        }

        let width = spectrogram.magnitude_data.len();
        let height = spectrogram.magnitude_data[0].len();

        let mut img: RgbImage = ImageBuffer::new(width as u32, height as u32);
        let gradient = viridis();

        let magnitude_range = spectrogram.max_magnitude - spectrogram.min_magnitude;

        for (x, time_slice) in spectrogram.magnitude_data.iter().enumerate() {
            for (y, &magnitude) in time_slice.iter().enumerate() {
                // Normalize magnitude to 0.0-1.0 range
                let normalized = if magnitude_range > 0.0 {
                    ((magnitude - spectrogram.min_magnitude) / magnitude_range).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let color = gradient.at(normalized as f64);
                let rgb_color = color.to_rgba8();

                // Flip Y axis (frequency increases upward)
                let pixel_y = height - 1 - y;
                img.put_pixel(
                    x as u32,
                    pixel_y as u32,
                    Rgb([rgb_color[0], rgb_color[1], rgb_color[2]]),
                );
            }
        }

        img.save(path)
            .map_err(|e| UshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        info!("Spectrogram saved successfully");
        Ok(())
    }

    /// Analyze full frequency spectrum
    fn analyze_full_spectrum(&mut self, samples: &[f32]) -> UshResult<FftAnalysis> {
        let fft_size = 4096; // High resolution for full spectrum
        self.perform_fft_analysis(samples, fft_size, 0.0, self.config.sample_rate as f32 / 2.0)
    }

    /// Analyze communication frequency band
    fn analyze_communication_band(&mut self, samples: &[f32]) -> UshResult<FftAnalysis> {
        let fft_size = 2048; // Good resolution for narrow band
        self.perform_fft_analysis(samples, fft_size, 15000.0, 25000.0)
    }

    /// Perform FFT analysis on samples
    fn perform_fft_analysis(
        &mut self,
        samples: &[f32],
        fft_size: usize,
        freq_min: f32,
        freq_max: f32,
    ) -> UshResult<FftAnalysis> {
        let fft = self.fft_planner.plan_fft_forward(fft_size);

        let mut fft_input: Vec<Complex<f32>> = samples[..fft_size.min(samples.len())]
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        fft_input.resize(fft_size, Complex::new(0.0, 0.0));

        fft.process(&mut fft_input);

        let freq_resolution = self.config.sample_rate as f32 / fft_size as f32;
        let min_bin = (freq_min / freq_resolution) as usize;
        let max_bin = ((freq_max / freq_resolution) as usize).min(fft_size / 2);

        let mut frequencies = Vec::new();
        let mut magnitudes = Vec::new();
        let mut peak_magnitude = 0.0f32;
        let mut peak_frequency = 0.0f32;

        for i in min_bin..max_bin {
            let freq = i as f32 * freq_resolution;
            let magnitude = fft_input[i].norm();

            frequencies.push(freq);
            magnitudes.push(magnitude);

            if magnitude > peak_magnitude {
                peak_magnitude = magnitude;
                peak_frequency = freq;
            }
        }

        // Calculate power at FSK frequencies
        let freq_0_bin = (self.config.freq_0 / freq_resolution) as usize;
        let freq_1_bin = (self.config.freq_1 / freq_resolution) as usize;

        let freq_0_power = if freq_0_bin < fft_size / 2 {
            self.measure_bin_power(&fft_input, freq_0_bin, 2)
        } else {
            0.0
        };

        let freq_1_power = if freq_1_bin < fft_size / 2 {
            self.measure_bin_power(&fft_input, freq_1_bin, 2)
        } else {
            0.0
        };

        // Estimate SNR
        let noise_power = self.estimate_noise_floor(&fft_input, &[freq_0_bin, freq_1_bin]);
        let signal_power = freq_0_power + freq_1_power;
        let snr_estimate = if noise_power > 0.0 {
            10.0 * (signal_power / noise_power).log10()
        } else {
            -60.0
        };

        Ok(FftAnalysis {
            frequencies,
            magnitudes,
            peak_frequency,
            peak_magnitude,
            freq_0_power,
            freq_1_power,
            snr_estimate,
        })
    }

    /// Save FFT plot as PNG
    fn save_fft_plot(&self, analysis: &FftAnalysis, path: &Path, title: &str) -> UshResult<()> {
        info!("Saving FFT plot to: {:?}", path);

        let root = BitMapBackend::new(path, (1200, 800)).into_drawing_area();
        root.fill(&WHITE).map_err(|e| UshError::Config {
            message: format!("Plot error: {}", e),
        })?;

        let max_magnitude = analysis.magnitudes.iter().fold(0.0f32, |a, &b| a.max(b));
        let min_freq = analysis.frequencies.first().copied().unwrap_or(0.0);
        let max_freq = analysis.frequencies.last().copied().unwrap_or(1.0);

        let mut chart = ChartBuilder::on(&root)
            .caption(title, ("sans-serif", 40))
            .margin(20)
            .x_label_area_size(60)
            .y_label_area_size(80)
            .build_cartesian_2d(
                min_freq as f64..max_freq as f64,
                0.0f64..(max_magnitude * 1.1) as f64,
            )
            .map_err(|e| UshError::Config {
                message: format!("Chart error: {}", e),
            })?;

        chart
            .configure_mesh()
            .x_desc("Frequency (Hz)")
            .y_desc("Magnitude")
            .draw()
            .map_err(|e| UshError::Config {
                message: format!("Mesh error: {}", e),
            })?;

        // Plot FFT magnitude
        chart
            .draw_series(LineSeries::new(
                analysis
                    .frequencies
                    .iter()
                    .zip(analysis.magnitudes.iter())
                    .map(|(&f, &m)| (f as f64, m as f64)),
                &BLUE,
            ))
            .map_err(|e| UshError::Config {
                message: format!("Series error: {}", e),
            })?
            .label("FFT Magnitude")
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 10, y)], &BLUE));

        // Mark FSK frequencies
        chart
            .draw_series(std::iter::once(Circle::new(
                (self.config.freq_0 as f64, (max_magnitude * 0.9) as f64),
                5,
                RED.filled(),
            )))
            .map_err(|e| UshError::Config {
                message: format!("Freq0 marker error: {}", e),
            })?
            .label(&format!("freq_0 ({} Hz)", self.config.freq_0))
            .legend(|(x, y)| Circle::new((x + 5, y), 3, RED.filled()));

        chart
            .draw_series(std::iter::once(Circle::new(
                (self.config.freq_1 as f64, (max_magnitude * 0.8) as f64),
                5,
                GREEN.filled(),
            )))
            .map_err(|e| UshError::Config {
                message: format!("Freq1 marker error: {}", e),
            })?
            .label(&format!("freq_1 ({} Hz)", self.config.freq_1))
            .legend(|(x, y)| Circle::new((x + 5, y), 3, GREEN.filled()));

        chart
            .configure_series_labels()
            .draw()
            .map_err(|e| UshError::Config {
                message: format!("Legend error: {}", e),
            })?;

        root.present().map_err(|e| UshError::Config {
            message: format!("Present error: {}", e),
        })?;
        info!("FFT plot saved successfully");
        Ok(())
    }

    /// Generate time-segmented FFT analysis
    fn generate_time_segmented_ffts(
        &mut self,
        samples: &[f32],
        output_dir: &Path,
    ) -> UshResult<()> {
        info!("Generating time-segmented FFT analysis...");

        let segment_duration = 5.0; // 5 seconds per segment
        let segment_samples = (segment_duration * self.config.sample_rate as f32) as usize;

        let fft_output_path = output_dir.join("time_segmented_ffts.png");
        let root = BitMapBackend::new(&fft_output_path, (1600, 1200)).into_drawing_area();
        root.fill(&WHITE).map_err(|e| UshError::Config {
            message: format!("Plot error: {}", e),
        })?;

        let segments: Vec<_> = samples.chunks(segment_samples).enumerate().collect();
        let rows = ((segments.len() as f32).sqrt().ceil() as usize).max(1);
        let cols = (segments.len() + rows - 1) / rows;

        let areas = root.split_evenly((rows, cols));

        for ((segment_idx, segment), area) in segments.iter().zip(areas.iter()) {
            let start_time = *segment_idx as f32 * segment_duration;
            let analysis = self.perform_fft_analysis(segment, 1024, 15000.0, 25000.0)?;

            let title = format!("t={:.1}s", start_time);
            let max_magnitude = analysis.magnitudes.iter().fold(0.0f32, |a, &b| a.max(b));

            if max_magnitude > 0.0 {
                let mut chart = ChartBuilder::on(area)
                    .caption(&title, ("sans-serif", 20))
                    .margin(10)
                    .x_label_area_size(30)
                    .y_label_area_size(40)
                    .build_cartesian_2d(
                        15000.0f64..25000.0f64,
                        0.0f64..(max_magnitude * 1.1) as f64,
                    )
                    .map_err(|e| UshError::Config {
                        message: format!("Chart error: {}", e),
                    })?;

                chart
                    .configure_mesh()
                    .x_desc("Hz")
                    .y_desc("Mag")
                    .label_style(("sans-serif", 12))
                    .draw()
                    .map_err(|e| UshError::Config {
                        message: format!("Mesh error: {}", e),
                    })?;

                chart
                    .draw_series(LineSeries::new(
                        analysis
                            .frequencies
                            .iter()
                            .zip(analysis.magnitudes.iter())
                            .map(|(&f, &m)| (f as f64, m as f64)),
                        &BLUE,
                    ))
                    .map_err(|e| UshError::Config {
                        message: format!("Series error: {}", e),
                    })?;

                // Mark FSK frequencies
                if self.config.freq_0 >= 15000.0 && self.config.freq_0 <= 25000.0 {
                    chart
                        .draw_series(std::iter::once(Circle::new(
                            (self.config.freq_0 as f64, (max_magnitude * 0.8) as f64),
                            2,
                            RED.filled(),
                        )))
                        .map_err(|e| UshError::Config {
                            message: format!("Marker error: {}", e),
                        })?;
                }

                if self.config.freq_1 >= 15000.0 && self.config.freq_1 <= 25000.0 {
                    chart
                        .draw_series(std::iter::once(Circle::new(
                            (self.config.freq_1 as f64, (max_magnitude * 0.8) as f64),
                            2,
                            GREEN.filled(),
                        )))
                        .map_err(|e| UshError::Config {
                            message: format!("Marker error: {}", e),
                        })?;
                }
            }
        }

        root.present().map_err(|e| UshError::Config {
            message: format!("Present error: {}", e),
        })?;
        info!("Time-segmented FFT analysis saved");
        Ok(())
    }

    /// Generate comprehensive HTML report
    fn generate_html_report(&self, analysis: &DebugAnalysis, output_dir: &Path) -> UshResult<()> {
        let html_content = format!(
            r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>USH Debug Analysis Report - {}</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .header {{ background: #2c3e50; color: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; }}
        .section {{ margin: 20px 0; padding: 15px; border: 1px solid #ddd; border-radius: 5px; }}
        .metrics {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 15px; }}
        .metric {{ background: #ecf0f1; padding: 10px; border-radius: 5px; }}
        .metric-value {{ font-size: 1.2em; font-weight: bold; color: #2c3e50; }}
        .good {{ color: #27ae60; }}
        .warning {{ color: #f39c12; }}
        .bad {{ color: #e74c3c; }}
        .visualization {{ text-align: center; margin: 20px 0; }}
        .visualization img {{ max-width: 100%; height: auto; border: 1px solid #ddd; border-radius: 5px; }}
        .file-list {{ list-style: none; padding: 0; }}
        .file-list li {{ background: #ecf0f1; margin: 5px 0; padding: 8px; border-radius: 3px; }}
        table {{ width: 100%; border-collapse: collapse; margin: 10px 0; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background: #f8f9fa; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🔊 USH Debug Analysis Report</h1>
            <p>Session: {} | Generated: {}</p>
        </div>

        <div class="section">
            <h2>📊 Signal Quality Metrics</h2>
            <div class="metrics">
                <div class="metric">
                    <div>Duration</div>
                    <div class="metric-value">{:.2} seconds</div>
                </div>
                <div class="metric">
                    <div>Sample Count</div>
                    <div class="metric-value">{} samples</div>
                </div>
                <div class="metric">
                    <div>RMS Level</div>
                    <div class="metric-value">{:.4}</div>
                </div>
                <div class="metric">
                    <div>Peak Level</div>
                    <div class="metric-value">{:.4}</div>
                </div>
                <div class="metric">
                    <div>Dynamic Range</div>
                    <div class="metric-value">{:.1} dB</div>
                </div>
                <div class="metric">
                    <div>Estimated SNR</div>
                    <div class="metric-value {}">{:.1} dB</div>
                </div>
            </div>
        </div>

        <div class="section">
            <h2>🎵 FSK Signal Detection</h2>
            <div class="metrics">
                <div class="metric">
                    <div>Frequency 0 ({} Hz) Presence</div>
                    <div class="metric-value {}">{:.1}%</div>
                </div>
                <div class="metric">
                    <div>Frequency 1 ({} Hz) Presence</div>
                    <div class="metric-value {}">{:.1}%</div>
                </div>
                <div class="metric">
                    <div>Peak Frequency</div>
                    <div class="metric-value">{:.0} Hz</div>
                </div>
                <div class="metric">
                    <div>Peak Magnitude</div>
                    <div class="metric-value">{:.4}</div>
                </div>
            </div>
        </div>

        <div class="section">
            <h2>📈 Visualizations</h2>

            <div class="visualization">
                <h3>Spectrogram (Time-Frequency Analysis)</h3>
                <img src="spectrogram.png" alt="Spectrogram showing frequency content over time">
                <p>Shows frequency content over time. FSK signals should appear as horizontal lines at {} Hz and {} Hz.</p>
            </div>

            <div class="visualization">
                <h3>Full Spectrum FFT</h3>
                <img src="full_spectrum_fft.png" alt="Full spectrum frequency analysis">
                <p>Complete frequency spectrum showing all detected frequencies.</p>
            </div>

            <div class="visualization">
                <h3>Communication Band Analysis (15-25 kHz)</h3>
                <img src="communication_band_fft.png" alt="Communication band frequency analysis">
                <p>Focused analysis of the ultrasonic communication frequency range.</p>
            </div>

            <div class="visualization">
                <h3>Time-Segmented Analysis</h3>
                <img src="time_segmented_ffts.png" alt="Time-segmented FFT analysis">
                <p>Frequency analysis over time segments showing signal evolution.</p>
            </div>
        </div>

        <div class="section">
            <h2>📁 Generated Files</h2>
            <ul class="file-list">
                {}
            </ul>
        </div>

        <div class="section">
            <h2>⚙️ Configuration</h2>
            <table>
                <tr><th>Parameter</th><th>Value</th></tr>
                <tr><td>Sample Rate</td><td>{} Hz</td></tr>
                <tr><td>FSK Frequency 0</td><td>{} Hz</td></tr>
                <tr><td>FSK Frequency 1</td><td>{} Hz</td></tr>
                <tr><td>Window Size</td><td>{} samples</td></tr>
                <tr><td>Hop Size</td><td>{} samples</td></tr>
                <tr><td>FFT Size</td><td>{} samples</td></tr>
            </table>
        </div>

        <div class="section">
            <h2>💡 Interpretation Guide</h2>
            <h3>Signal Quality Indicators:</h3>
            <ul>
                <li><strong>SNR > 10 dB:</strong> <span class="good">Excellent signal quality</span></li>
                <li><strong>SNR 5-10 dB:</strong> <span class="warning">Good signal quality</span></li>
                <li><strong>SNR < 5 dB:</strong> <span class="bad">Poor signal quality, may affect decoding</span></li>
            </ul>

            <h3>FSK Detection:</h3>
            <ul>
                <li><strong>Presence > 50%:</strong> Strong signal detected at frequency</li>
                <li><strong>Presence 20-50%:</strong> Moderate signal presence</li>
                <li><strong>Presence < 20%:</strong> Weak or no signal detected</li>
            </ul>
        </div>
    </div>
</body>
</html>
"#,
            analysis.session_id,
            analysis.session_id,
            chrono::DateTime::from_timestamp(analysis.timestamp as i64, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S UTC"),
            analysis.signal_metrics.duration_seconds,
            analysis.signal_metrics.samples_count,
            analysis.signal_metrics.rms_level,
            analysis.signal_metrics.peak_level,
            analysis.signal_metrics.dynamic_range,
            if analysis.signal_metrics.estimated_snr > 10.0 {
                "good"
            } else if analysis.signal_metrics.estimated_snr > 5.0 {
                "warning"
            } else {
                "bad"
            },
            analysis.signal_metrics.estimated_snr,
            analysis.config.freq_0,
            if analysis.signal_metrics.freq_0_presence > 0.5 {
                "good"
            } else if analysis.signal_metrics.freq_0_presence > 0.2 {
                "warning"
            } else {
                "bad"
            },
            analysis.signal_metrics.freq_0_presence * 100.0,
            analysis.config.freq_1,
            if analysis.signal_metrics.freq_1_presence > 0.5 {
                "good"
            } else if analysis.signal_metrics.freq_1_presence > 0.2 {
                "warning"
            } else {
                "bad"
            },
            analysis.signal_metrics.freq_1_presence * 100.0,
            analysis.fft_analysis.peak_frequency,
            analysis.fft_analysis.peak_magnitude,
            analysis.config.freq_0,
            analysis.config.freq_1,
            analysis
                .files_generated
                .iter()
                .map(|f| format!("<li>📄 {}</li>", f))
                .collect::<Vec<_>>()
                .join("\n                "),
            analysis.config.sample_rate,
            analysis.config.freq_0,
            analysis.config.freq_1,
            analysis.config.window_size,
            analysis.config.hop_size,
            analysis.config.fft_size
        );

        let html_path = output_dir.join("debug_report.html");
        fs::write(&html_path, html_content).map_err(|e| UshError::Io(e))?;
        info!("HTML report saved to: {:?}", html_path);
        Ok(())
    }

    /// Save audio samples as WAV file
    fn save_wav_file(&self, samples: &[f32], path: &Path) -> UshResult<()> {
        use hound::{WavSpec, WavWriter};

        let spec = WavSpec {
            channels: 1,
            sample_rate: self.config.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = WavWriter::create(path, spec)
            .map_err(|e| UshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        for &sample in samples {
            writer
                .write_sample(sample)
                .map_err(|e| UshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        }

        writer
            .finalize()
            .map_err(|e| UshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        debug!("WAV file saved: {:?}", path);
        Ok(())
    }
}

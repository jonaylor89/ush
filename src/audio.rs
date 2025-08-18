use crate::{UshError, UshResult};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{
    Device, FromSample, Host, InputCallbackInfo, OutputCallbackInfo, Sample, SampleFormat,
    SampleRate, Stream, SupportedStreamConfig,
};
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

const SAMPLE_RATE: u32 = 44100;
const CHANNELS: u16 = 1;

#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
            buffer_size: 4096,
        }
    }
}

pub struct AudioManager {
    host: Host,
    config: AudioConfig,
}

impl AudioManager {
    pub fn new() -> UshResult<Self> {
        let host = cpal::default_host();
        info!("Using audio host: {}", host.id().name());

        Ok(Self {
            host,
            config: AudioConfig::default(),
        })
    }

    pub fn with_config(config: AudioConfig) -> UshResult<Self> {
        let host = cpal::default_host();
        info!("Using audio host: {}", host.id().name());

        Ok(Self { host, config })
    }

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

    fn get_supported_config(
        &self,
        device: &Device,
        is_input: bool,
    ) -> UshResult<SupportedStreamConfig> {
        let sample_rate = SampleRate(self.config.sample_rate);

        let mut available_configs = Vec::new();

        // Try to find ANY compatible configuration
        if is_input {
            for config in device.supported_input_configs()? {
                available_configs.push((
                    config.channels(),
                    config.min_sample_rate().0,
                    config.max_sample_rate().0,
                    config.sample_format(),
                ));

                // Accept any reasonable configuration we can adapt
                if (config.channels() >= 1 && config.channels() <= 2) // mono or stereo
                    && config.min_sample_rate() <= sample_rate
                    && sample_rate <= config.max_sample_rate()
                {
                    if config.channels() != self.config.channels {
                        warn!(
                            "Using {} channels instead of requested {}",
                            config.channels(),
                            self.config.channels
                        );
                    }
                    return Ok(config.with_sample_rate(sample_rate));
                }
            }

            // Try alternative sample rates
            for &alt_rate in &[48000, 22050, 96000] {
                let alt_sample_rate = SampleRate(alt_rate);
                for config in device.supported_input_configs()? {
                    if (config.channels() >= 1 && config.channels() <= 2)
                        && config.min_sample_rate() <= alt_sample_rate
                        && alt_sample_rate <= config.max_sample_rate()
                    {
                        warn!(
                            "Using {}Hz sample rate instead of requested {}Hz",
                            alt_rate, self.config.sample_rate
                        );
                        warn!(
                            "Using {} channels instead of requested {}",
                            config.channels(),
                            self.config.channels
                        );
                        return Ok(config.with_sample_rate(alt_sample_rate));
                    }
                }
            }
        } else {
            for config in device.supported_output_configs()? {
                available_configs.push((
                    config.channels(),
                    config.min_sample_rate().0,
                    config.max_sample_rate().0,
                    config.sample_format(),
                ));

                // Accept any reasonable configuration we can adapt
                if (config.channels() >= 1 && config.channels() <= 2) // mono or stereo
                    && config.min_sample_rate() <= sample_rate
                    && sample_rate <= config.max_sample_rate()
                {
                    if config.channels() != self.config.channels {
                        warn!(
                            "Using {} channels instead of requested {}",
                            config.channels(),
                            self.config.channels
                        );
                    }
                    return Ok(config.with_sample_rate(sample_rate));
                }
            }

            // Try alternative sample rates
            for &alt_rate in &[48000, 22050, 96000] {
                let alt_sample_rate = SampleRate(alt_rate);
                for config in device.supported_output_configs()? {
                    if (config.channels() >= 1 && config.channels() <= 2)
                        && config.min_sample_rate() <= alt_sample_rate
                        && alt_sample_rate <= config.max_sample_rate()
                    {
                        warn!(
                            "Using {}Hz sample rate instead of requested {}Hz",
                            alt_rate, self.config.sample_rate
                        );
                        warn!(
                            "Using {} channels instead of requested {}",
                            config.channels(),
                            self.config.channels
                        );
                        return Ok(config.with_sample_rate(alt_sample_rate));
                    }
                }
            }
        }

        // Log available configurations for debugging
        error!("Available audio configurations:");
        for (channels, min_rate, max_rate, format) in &available_configs {
            error!(
                "  {} channels, {}-{} Hz, {:?}",
                channels, min_rate, max_rate, format
            );
        }

        Err(UshError::Config {
            message: format!(
                "No suitable audio configuration found for {} channels at {} Hz. Available: {:?}",
                self.config.channels, self.config.sample_rate, available_configs
            ),
        })
    }

    pub fn create_input_stream(
        &self,
        mut callback: impl FnMut(&[f32]) + Send + 'static,
    ) -> UshResult<Stream> {
        let device = self.get_input_device()?;
        let config = self.get_supported_config(&device, true)?;

        info!("Input device: {}", device.name().unwrap_or_default());
        debug!("Input config: {:?}", config);

        let stream = match config.sample_format() {
            SampleFormat::I8 => {
                let channels = config.channels() as usize;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i8], _: &InputCallbackInfo| {
                        let samples: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i8::MAX as f32).collect();
                        if channels == 1 {
                            callback(&samples);
                        } else {
                            let mono_samples: Vec<f32> = samples
                                .chunks_exact(channels)
                                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                                .collect();
                            callback(&mono_samples);
                        }
                    },
                    |err| warn!("Input stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                let channels = config.channels() as usize;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &InputCallbackInfo| {
                        let samples: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        if channels == 1 {
                            callback(&samples);
                        } else {
                            let mono_samples: Vec<f32> = samples
                                .chunks_exact(channels)
                                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                                .collect();
                            callback(&mono_samples);
                        }
                    },
                    |err| warn!("Input stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I32 => {
                let channels = config.channels() as usize;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i32], _: &InputCallbackInfo| {
                        let samples: Vec<f32> =
                            data.iter().map(|&s| s as f32 / i32::MAX as f32).collect();
                        if channels == 1 {
                            callback(&samples);
                        } else {
                            let mono_samples: Vec<f32> = samples
                                .chunks_exact(channels)
                                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                                .collect();
                            callback(&mono_samples);
                        }
                    },
                    |err| warn!("Input stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::F32 => {
                let channels = config.channels() as usize;
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &InputCallbackInfo| {
                        if channels == 1 {
                            // Already mono, pass through directly
                            callback(data);
                        } else {
                            // Convert stereo to mono by averaging channels
                            let mono_samples: Vec<f32> = data
                                .chunks_exact(channels)
                                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                                .collect();
                            callback(&mono_samples);
                        }
                    },
                    |err| warn!("Input stream error: {}", err),
                    None,
                )?
            }
            _ => {
                return Err(UshError::Config {
                    message: format!("Unsupported sample format: {:?}", config.sample_format()),
                });
            }
        };

        Ok(stream)
    }

    pub fn create_output_stream(
        &self,
        samples: Arc<Mutex<Vec<f32>>>,
        finished_tx: mpsc::UnboundedSender<()>,
    ) -> UshResult<Stream> {
        let device = self.get_output_device()?;
        let config = self.get_supported_config(&device, false)?;

        info!("Output device: {}", device.name().unwrap_or_default());
        debug!("Output config: {:?}", config);

        let sample_index = Arc::new(Mutex::new(0usize));
        let channels = config.channels() as usize;

        let stream = match config.sample_format() {
            SampleFormat::I8 => {
                let samples = samples.clone();
                let sample_index = sample_index.clone();
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [i8], _: &OutputCallbackInfo| {
                        Self::fill_output_buffer(
                            data,
                            &samples,
                            &sample_index,
                            channels,
                            &finished_tx,
                        );
                    },
                    |err| warn!("Output stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                let samples = samples.clone();
                let sample_index = sample_index.clone();
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [i16], _: &OutputCallbackInfo| {
                        Self::fill_output_buffer(
                            data,
                            &samples,
                            &sample_index,
                            channels,
                            &finished_tx,
                        );
                    },
                    |err| warn!("Output stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I32 => {
                let samples = samples.clone();
                let sample_index = sample_index.clone();
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [i32], _: &OutputCallbackInfo| {
                        Self::fill_output_buffer(
                            data,
                            &samples,
                            &sample_index,
                            channels,
                            &finished_tx,
                        );
                    },
                    |err| warn!("Output stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::F32 => {
                let samples = samples.clone();
                let sample_index = sample_index.clone();
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &OutputCallbackInfo| {
                        Self::fill_output_buffer(
                            data,
                            &samples,
                            &sample_index,
                            channels,
                            &finished_tx,
                        );
                    },
                    |err| warn!("Output stream error: {}", err),
                    None,
                )?
            }
            _ => {
                return Err(UshError::Config {
                    message: format!("Unsupported sample format: {:?}", config.sample_format()),
                });
            }
        };

        Ok(stream)
    }

    fn fill_output_buffer<T>(
        data: &mut [T],
        samples: &Arc<Mutex<Vec<f32>>>,
        sample_index: &Arc<Mutex<usize>>,
        channels: usize,
        finished_tx: &mpsc::UnboundedSender<()>,
    ) where
        T: Sample + Send + FromSample<f32>,
    {
        let samples_lock = samples.lock().unwrap();
        let mut index_lock = sample_index.lock().unwrap();

        for frame in data.chunks_mut(channels) {
            if *index_lock >= samples_lock.len() {
                for sample in frame.iter_mut() {
                    *sample = T::EQUILIBRIUM;
                }
                let _ = finished_tx.send(());
                return;
            }

            let sample_value = samples_lock[*index_lock];
            let converted_sample = T::from_sample(sample_value);

            // Fill all channels with the same mono sample (mono -> stereo conversion)
            for sample in frame.iter_mut() {
                *sample = converted_sample;
            }

            *index_lock += 1;
        }
    }

    pub fn get_config(&self) -> &AudioConfig {
        &self.config
    }

    pub fn get_host(&self) -> &Host {
        &self.host
    }
}

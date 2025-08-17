use cpal::{
    Device, Host, InputCallbackInfo, OutputCallbackInfo, Sample, SampleFormat, SampleRate,
    Stream, SupportedStreamConfig, FromSample,
};
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use crate::{UshError, UshResult};
use log::{info, warn, debug};

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
        
        if is_input {
            for config in device.supported_input_configs()? {
                if config.channels() == self.config.channels
                    && config.min_sample_rate() <= sample_rate
                    && sample_rate <= config.max_sample_rate()
                {
                    return Ok(config.with_sample_rate(sample_rate));
                }
            }
        } else {
            for config in device.supported_output_configs()? {
                if config.channels() == self.config.channels
                    && config.min_sample_rate() <= sample_rate
                    && sample_rate <= config.max_sample_rate()
                {
                    return Ok(config.with_sample_rate(sample_rate));
                }
            }
        }


        Err(UshError::Config {
            message: format!(
                "No suitable audio configuration found for {} channels at {} Hz",
                self.config.channels, self.config.sample_rate
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
            SampleFormat::I8 => device.build_input_stream(
                &config.into(),
                move |data: &[i8], _: &InputCallbackInfo| {
                    let samples: Vec<f32> = data.iter().map(|&s| s as f32 / i8::MAX as f32).collect();
                    callback(&samples);
                },
                |err| warn!("Input stream error: {}", err),
                None,
            )?,
            SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &InputCallbackInfo| {
                    let samples: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    callback(&samples);
                },
                |err| warn!("Input stream error: {}", err),
                None,
            )?,
            SampleFormat::I32 => device.build_input_stream(
                &config.into(),
                move |data: &[i32], _: &InputCallbackInfo| {
                    let samples: Vec<f32> = data.iter().map(|&s| s as f32 / i32::MAX as f32).collect();
                    callback(&samples);
                },
                |err| warn!("Input stream error: {}", err),
                None,
            )?,
            SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &InputCallbackInfo| {
                    callback(data);
                },
                |err| warn!("Input stream error: {}", err),
                None,
            )?,
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
                        Self::fill_output_buffer(data, &samples, &sample_index, channels, &finished_tx);
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
                        Self::fill_output_buffer(data, &samples, &sample_index, channels, &finished_tx);
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
                        Self::fill_output_buffer(data, &samples, &sample_index, channels, &finished_tx);
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
                        Self::fill_output_buffer(data, &samples, &sample_index, channels, &finished_tx);
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
            
            for sample in frame.iter_mut() {
                *sample = converted_sample;
            }
            
            *index_lock += 1;
        }
    }

    pub fn get_config(&self) -> &AudioConfig {
        &self.config
    }
}
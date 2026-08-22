//! Android Low-Latency Audio Stream Pipeline (AAudio / Oboe Backend).
//!
//! Provides sub-30ms output latency, lock-free ring buffer synchronization,
//! and battery-conscious Android lifecycle integration (`pause_audio_stream` / `resume_audio_stream`).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;

use crate::audio::{AudioProducer, DEFAULT_BUFFER_CAPACITY};
use crate::platform::PlatformAudio;

/// Target audio output latency for Android AAudio stream (<= 30ms).
pub const ANDROID_TARGET_LATENCY_MS: u32 = 30;

/// Android-specific audio player wrapping `cpal` AAudio/Oboe backend.
pub struct AndroidAudioPlayer {
    stream: Option<cpal::Stream>,
    producer: AudioProducer,
    sample_rate: u32,
    channels: u16,
    device_name: String,
    stream_config: cpal::StreamConfig,
    is_paused: bool,
}

impl AndroidAudioPlayer {
    /// Initializes low-latency Android audio output stream targeting AAudio/Oboe.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default Android audio output device found")?;

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Android AAudio Device".to_string());
        info!("Android Audio Device: {}", device_name);

        let default_config = device.default_output_config()?;
        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        info!(
            "Android Audio Device Config: channels={}, sample_rate={} Hz, format={:?}",
            channels,
            sample_rate,
            default_config.sample_format()
        );

        // Calculate burst buffer frame size for low-latency target (<= 30ms)
        // 48000 Hz * 0.020s = 960 frames (~20ms burst)
        let burst_frames = ((sample_rate as f32 * 0.020).round() as u32).clamp(256, 1024);
        let mut stream_config: cpal::StreamConfig = default_config.into();
        stream_config.buffer_size = cpal::BufferSize::Fixed(burst_frames);

        let ring_buffer = HeapRb::<f32>::new(DEFAULT_BUFFER_CAPACITY);
        let (prod, cons) = ring_buffer.split();

        let producer = AudioProducer::with_rates(prod, 65536.0, sample_rate as f64);

        let stream = Self::build_stream(&device, &stream_config, cons)?;
        stream.play()?;

        info!(
            "Android AAudio low-latency stream active: {} Hz, buffer burst={} frames (<= {}ms latency)",
            sample_rate, burst_frames, ANDROID_TARGET_LATENCY_MS
        );

        Ok(Self {
            stream: Some(stream),
            producer,
            sample_rate,
            channels,
            device_name,
            stream_config,
            is_paused: false,
        })
    }

    /// Internal helper to build output stream callback.
    fn build_stream(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut cons: ringbuf::HeapCons<f32>,
    ) -> Result<cpal::Stream, Box<dyn std::error::Error>> {
        let err_fn = |err| error!("Android audio stream callback error: {:?}", err);
        let ch_count = config.channels as usize;

        let mut last_left = 0.0_f32;
        let mut last_right = 0.0_f32;

        let stream = if ch_count == 1 {
            device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for sample in data.iter_mut() {
                        if let Some(s) = cons.try_pop() {
                            last_left = s;
                            *sample = s;
                        } else {
                            // Smooth exponential decay on underrun prevents audio pop
                            last_left *= 0.92;
                            *sample = last_left;
                        }
                    }
                },
                err_fn,
                None,
            )?
        } else {
            device.build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / ch_count;
                    for frame in 0..frames {
                        let left = if let Some(s) = cons.try_pop() {
                            last_left = s;
                            s
                        } else {
                            last_left *= 0.92;
                            last_left
                        };

                        let right = if let Some(s) = cons.try_pop() {
                            last_right = s;
                            s
                        } else {
                            last_right *= 0.92;
                            last_right
                        };

                        let base = frame * ch_count;
                        if base < data.len() {
                            data[base] = left;
                        }
                        if base + 1 < data.len() {
                            data[base + 1] = right;
                        }
                        for extra_ch in 2..ch_count {
                            if base + extra_ch < data.len() {
                                data[base + extra_ch] = 0.0;
                            }
                        }
                    }
                },
                err_fn,
                None,
            )?
        };

        Ok(stream)
    }

    /// Automatically pauses/stops audio stream when `android_activity::MainEvent::Pause` occurs to prevent battery drain.
    pub fn pause_audio_stream(&mut self) {
        if self.is_paused {
            return;
        }

        if let Some(ref stream) = self.stream {
            if let Err(err) = stream.pause() {
                warn!("Failed to pause Android audio stream: {:?}", err);
            } else {
                info!("Android audio stream paused on MainEvent::Pause");
            }
        }
        self.is_paused = true;
    }

    /// Automatically reinitializes/resumes audio stream when `android_activity::MainEvent::Resume` occurs with fresh ring buffer state.
    pub fn resume_audio_stream(&mut self) {
        if !self.is_paused {
            return;
        }

        // Flush stale buffered audio samples and reset rate controller
        self.producer.clear_buffer();

        if let Some(ref stream) = self.stream {
            if let Err(err) = stream.play() {
                warn!("Failed to resume Android audio stream: {:?}", err);
            } else {
                info!("Android audio stream resumed on MainEvent::Resume with clean buffer");
            }
        }
        self.is_paused = false;
    }

    /// Returns a cloneable producer handle for pushing audio samples.
    pub fn producer(&self) -> AudioProducer {
        self.producer.clone()
    }

    /// Returns the active audio stream sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the number of output channels.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Returns the active Android audio output device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Returns the active stream configuration.
    pub fn stream_config(&self) -> &cpal::StreamConfig {
        &self.stream_config
    }
}

impl PlatformAudio for AndroidAudioPlayer {
    fn producer(&self) -> AudioProducer {
        self.producer.clone()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn pause(&mut self) {
        self.pause_audio_stream();
    }

    fn resume(&mut self) {
        self.resume_audio_stream();
    }

    fn set_muted(&self, muted: bool) {
        self.producer.set_muted(muted);
    }

    fn is_muted(&self) -> bool {
        self.producer.is_muted()
    }

    fn set_volume(&self, volume: f32) {
        self.producer.set_volume(volume);
    }

    fn volume(&self) -> f32 {
        self.producer.volume()
    }

    fn set_fast_forward(&self, enabled: bool) {
        self.producer.set_fast_forward(enabled);
    }
}

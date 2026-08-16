#![allow(dead_code)]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::{Arc, Mutex};

pub const DEFAULT_BUFFER_CAPACITY: usize = 4096 * 2; // 4096 stereo frames (8192 f32 values)

/// 2nd-order Butterworth IIR Low-Pass Biquad Filter.
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    d1: f32,
    d2: f32,
}

impl BiquadFilter {
    pub fn new_lowpass(cutoff_hz: f32, sample_rate: f32) -> Self {
        let cutoff = cutoff_hz.min(sample_rate * 0.45);
        let omega = 2.0 * std::f32::consts::PI * (cutoff / sample_rate);
        let cos_w = omega.cos();
        let sin_w = omega.sin();
        let q = 0.7071068_f32; // Butterworth Q
        let alpha = sin_w / (2.0 * q);

        let a0 = 1.0 + alpha;
        let b0 = ((1.0 - cos_w) / 2.0) / a0;
        let b1 = (1.0 - cos_w) / a0;
        let b2 = ((1.0 - cos_w) / 2.0) / a0;
        let a1 = (-2.0 * cos_w) / a0;
        let a2 = (1.0 - alpha) / a0;

        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            d1: 0.0,
            d2: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.d1;
        self.d1 = self.b1 * x - self.a1 * y + self.d2;
        self.d2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Single-pole DC blocker filter.
#[derive(Debug, Clone)]
pub struct DcBlocker {
    prev_x: f32,
    prev_y: f32,
    r: f32,
}

impl DcBlocker {
    pub fn new() -> Self {
        Self {
            prev_x: 0.0,
            prev_y: 0.0,
            r: 0.995,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.prev_x + self.r * self.prev_y;
        self.prev_x = x;
        self.prev_y = y;
        y
    }
}

/// Catmull-Rom 4-point cubic Hermite spline interpolation.
#[inline(always)]
fn cubic_hermite(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c0 = y1;
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + c0
}

/// Smooth soft-knee saturation curve to prevent hard digital clipping and harshness.
#[inline(always)]
fn soft_limit(x: f32) -> f32 {
    if x.abs() <= 0.85 {
        x
    } else {
        let sign = x.signum();
        let excess = x.abs() - 0.85;
        sign * (0.85 + 0.15 * (excess / 0.15).tanh())
    }
}

/// High-quality Catmull-Rom cubic Hermite audio resampler with 2nd-order Butterworth anti-aliasing.
#[derive(Debug, Clone)]
pub struct Resampler {
    in_rate: f64,
    out_rate: f64,
    phase: f64,

    // 4-point sample history for cubic interpolation
    hist_l: [f32; 4],
    hist_r: [f32; 4],

    // Post-resampling 2nd-order Butterworth low-pass filters (~15 kHz cutoff)
    lp_filter_l: BiquadFilter,
    lp_filter_r: BiquadFilter,

    // DC blockers
    dc_blocker_l: DcBlocker,
    dc_blocker_r: DcBlocker,

    initialized: bool,
}

impl Resampler {
    pub fn new(in_rate: f64, out_rate: f64) -> Self {
        let valid_in = if in_rate > 0.0 { in_rate } else { 65536.0 };
        let valid_out = if out_rate > 0.0 { out_rate } else { 48000.0 };

        Self {
            in_rate: valid_in,
            out_rate: valid_out,
            phase: 0.0,
            hist_l: [0.0; 4],
            hist_r: [0.0; 4],
            lp_filter_l: BiquadFilter::new_lowpass(15000.0, valid_out as f32),
            lp_filter_r: BiquadFilter::new_lowpass(15000.0, valid_out as f32),
            dc_blocker_l: DcBlocker::new(),
            dc_blocker_r: DcBlocker::new(),
            initialized: false,
        }
    }

    pub fn set_input_rate(&mut self, in_rate: f64) {
        if in_rate > 0.0 {
            info!("Resampler input sample rate set to {:.1} Hz (Target: {:.1} Hz)", in_rate, self.out_rate);
            self.in_rate = in_rate;
        }
    }

    /// Resample interleaved i16 slice `[L, R, L, R, ...]` into output buffer of f32s.
    pub fn resample_i16_slice(
        &mut self,
        input: &[i16],
        output: &mut Vec<f32>,
        dynamic_adjustment: f64,
    ) {
        if input.is_empty() {
            return;
        }

        let num_frames = input.len() / 2;
        let step = (self.in_rate / self.out_rate) * dynamic_adjustment;

        let mut curr_idx = 0;
        if !self.initialized && num_frames > 0 {
            let l = input[0] as f32 / 32768.0;
            let r = input[1] as f32 / 32768.0;
            self.hist_l = [l; 4];
            self.hist_r = [r; 4];
            self.initialized = true;
        }

        while curr_idx < num_frames {
            let curr_l = input[curr_idx * 2] as f32 / 32768.0;
            let curr_r = input[curr_idx * 2 + 1] as f32 / 32768.0;

            while self.phase < 1.0 {
                let frac = self.phase as f32;
                let interp_l = cubic_hermite(
                    self.hist_l[0],
                    self.hist_l[1],
                    self.hist_l[2],
                    curr_l,
                    frac,
                );
                let interp_r = cubic_hermite(
                    self.hist_r[0],
                    self.hist_r[1],
                    self.hist_r[2],
                    curr_r,
                    frac,
                );

                // Anti-aliasing filter & DC blocking
                let filtered_l = self.lp_filter_l.process(self.dc_blocker_l.process(interp_l));
                let filtered_r = self.lp_filter_r.process(self.dc_blocker_r.process(interp_r));

                output.push(soft_limit(filtered_l));
                output.push(soft_limit(filtered_r));

                self.phase += step;
            }

            // Shift history window for cubic Hermite interpolation
            self.hist_l[0] = self.hist_l[1];
            self.hist_l[1] = self.hist_l[2];
            self.hist_l[2] = curr_l;

            self.hist_r[0] = self.hist_r[1];
            self.hist_r[1] = self.hist_r[2];
            self.hist_r[2] = curr_r;

            self.phase -= 1.0;
            curr_idx += 1;
        }
    }

    /// Resample interleaved f32 slice `[L, R, L, R, ...]` into output buffer of f32s.
    pub fn resample_f32_slice(
        &mut self,
        input: &[f32],
        output: &mut Vec<f32>,
        dynamic_adjustment: f64,
    ) {
        if input.is_empty() {
            return;
        }

        let num_frames = input.len() / 2;
        let step = (self.in_rate / self.out_rate) * dynamic_adjustment;

        let mut curr_idx = 0;
        if !self.initialized && num_frames > 0 {
            let l = input[0];
            let r = input[1];
            self.hist_l = [l; 4];
            self.hist_r = [r; 4];
            self.initialized = true;
        }

        while curr_idx < num_frames {
            let curr_l = input[curr_idx * 2];
            let curr_r = input[curr_idx * 2 + 1];

            while self.phase < 1.0 {
                let frac = self.phase as f32;
                let interp_l = cubic_hermite(
                    self.hist_l[0],
                    self.hist_l[1],
                    self.hist_l[2],
                    curr_l,
                    frac,
                );
                let interp_r = cubic_hermite(
                    self.hist_r[0],
                    self.hist_r[1],
                    self.hist_r[2],
                    curr_r,
                    frac,
                );

                let filtered_l = self.lp_filter_l.process(self.dc_blocker_l.process(interp_l));
                let filtered_r = self.lp_filter_r.process(self.dc_blocker_r.process(interp_r));

                output.push(soft_limit(filtered_l));
                output.push(soft_limit(filtered_r));

                self.phase += step;
            }

            self.hist_l[0] = self.hist_l[1];
            self.hist_l[1] = self.hist_l[2];
            self.hist_l[2] = curr_l;

            self.hist_r[0] = self.hist_r[1];
            self.hist_r[1] = self.hist_r[2];
            self.hist_r[2] = curr_r;

            self.phase -= 1.0;
            curr_idx += 1;
        }
    }
}

struct ProducerInner {
    prod: ringbuf::HeapProd<f32>,
    resampler: Resampler,
    work_buf: Vec<f32>,
    fast_forward: bool,
    muted: bool,
    volume: f32,
}

/// Thread-safe sample producer handle shared between the emulator thread and the host audio output.
#[derive(Clone)]
pub struct AudioProducer {
    inner: Arc<Mutex<ProducerInner>>,
}

impl std::fmt::Debug for AudioProducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioProducer").finish()
    }
}

impl AudioProducer {
    /// Creates an AudioProducer wrapping a ringbuf producer with standard 48kHz target.
    pub fn from_producer(prod: ringbuf::HeapProd<f32>) -> Self {
        Self::with_rates(prod, 65536.0, 48000.0)
    }

    /// Creates an AudioProducer with explicit input and output sample rates.
    pub fn with_rates(prod: ringbuf::HeapProd<f32>, in_rate: f64, out_rate: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProducerInner {
                prod,
                resampler: Resampler::new(in_rate, out_rate),
                work_buf: Vec::with_capacity(2048),
                fast_forward: false,
                muted: false,
                volume: 1.0,
            })),
        }
    }

    /// Creates an AudioProducer and consumer pair with given capacity.
    pub fn new_pair(capacity: usize) -> (Self, ringbuf::HeapCons<f32>) {
        let ring_buffer = HeapRb::<f32>::new(capacity);
        let (prod, cons) = ring_buffer.split();
        (Self::from_producer(prod), cons)
    }

    /// Sets master volume level (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.volume = volume.clamp(0.0, 1.0);
        }
    }

    /// Returns the current master volume level (0.0 to 1.0).
    pub fn volume(&self) -> f32 {
        if let Ok(inner) = self.inner.lock() {
            inner.volume
        } else {
            1.0
        }
    }

    /// Sets whether fast-forward mode is active (drops samples during fast-forward).
    pub fn set_fast_forward(&self, enabled: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.fast_forward != enabled {
                inner.fast_forward = enabled;
                if !enabled {
                    inner.work_buf.clear();
                }
            }
        }
    }

    /// Returns whether fast-forward mode is currently active.
    pub fn is_fast_forward(&self) -> bool {
        if let Ok(inner) = self.inner.lock() {
            inner.fast_forward
        } else {
            false
        }
    }

    /// Sets mute state on audio stream.
    pub fn set_muted(&self, muted: bool) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.muted = muted;
            if !muted {
                inner.work_buf.clear();
            }
        }
    }

    /// Toggles mute state and returns the new muted status.
    pub fn toggle_mute(&self) -> bool {
        if let Ok(mut inner) = self.inner.lock() {
            inner.muted = !inner.muted;
            if !inner.muted {
                inner.work_buf.clear();
            }
            inner.muted
        } else {
            false
        }
    }

    /// Returns whether audio output is currently muted.
    pub fn is_muted(&self) -> bool {
        if let Ok(inner) = self.inner.lock() {
            inner.muted
        } else {
            false
        }
    }

    /// Set the input sample rate from the active emulation core (e.g. 65536.0 Hz for GBA).
    pub fn set_input_sample_rate(&self, in_rate: f64) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.resampler.set_input_rate(in_rate);
        }
    }

    /// Push a single stereo sample pair normalized to [-1.0, 1.0].
    pub fn push_stereo_sample(&self, left: f32, right: f32) {
        self.push_f32_slice(&[left, right]);
    }

    /// Push a 16-bit integer stereo sample pair.
    pub fn push_i16_pair(&self, left: i16, right: i16) {
        self.push_i16_slice(&[left, right]);
    }

    /// Push an interleaved slice of 16-bit integer stereo samples `[L, R, L, R, ...]`.
    pub fn push_i16_slice(&self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }

        if let Ok(mut inner) = self.inner.lock() {
            if inner.muted || inner.fast_forward {
                // Discard incoming audio frames when muted or fast-forwarding
                return;
            }

            let capacity = inner.prod.capacity().get();
            let occupied = inner.prod.occupied_len();

            // Continuous Proportional Dynamic Rate Control (targets 50% buffer fill)
            let fill_ratio = occupied as f64 / capacity as f64;
            let diff = fill_ratio - 0.50;
            let dynamic_adj = 1.0 + (diff * 0.015).clamp(-0.0075, 0.0075);

            inner.work_buf.clear();
            let ProducerInner {
                ref mut prod,
                ref mut resampler,
                ref mut work_buf,
                volume,
                ..
            } = *inner;

            resampler.resample_i16_slice(samples, work_buf, dynamic_adj);

            let vacant = prod.vacant_len();
            let count = work_buf.len().min(vacant);
            for &s in &work_buf[..count] {
                let _ = prod.try_push(s * volume);
            }
        }
    }

    /// Push an interleaved slice of normalized float stereo samples `[L, R, L, R, ...]`.
    pub fn push_f32_slice(&self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        if let Ok(mut inner) = self.inner.lock() {
            if inner.muted || inner.fast_forward {
                // Discard incoming audio frames when muted or fast-forwarding
                return;
            }

            let capacity = inner.prod.capacity().get();
            let occupied = inner.prod.occupied_len();

            let fill_ratio = occupied as f64 / capacity as f64;
            let diff = fill_ratio - 0.50;
            let dynamic_adj = 1.0 + (diff * 0.015).clamp(-0.0075, 0.0075);

            inner.work_buf.clear();
            let ProducerInner {
                ref mut prod,
                ref mut resampler,
                ref mut work_buf,
                volume,
                ..
            } = *inner;

            resampler.resample_f32_slice(samples, work_buf, dynamic_adj);

            let vacant = prod.vacant_len();
            let count = work_buf.len().min(vacant);
            for &s in &work_buf[..count] {
                let _ = prod.try_push(s * volume);
            }
        }
    }
}

/// Host audio output player managing the cpal output audio stream and lock-free ring buffer.
pub struct AudioPlayer {
    stream: Option<cpal::Stream>,
    producer: AudioProducer,
    sample_rate: u32,
    channels: u16,
}

impl AudioPlayer {
    /// Detects default host audio device, builds output stream, and starts playback.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No default audio output device available on host system")?;

        let device_name = device.name().unwrap_or_else(|_| "Default Audio Device".to_string());
        info!("Host Audio Device: {}", device_name);

        let default_config = device.default_output_config()?;
        info!(
            "Default Audio Config: Channels={}, SampleRate={}, Format={:?}",
            default_config.channels(),
            default_config.sample_rate().0,
            default_config.sample_format()
        );

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        let ring_buffer = HeapRb::<f32>::new(DEFAULT_BUFFER_CAPACITY);
        let (prod, mut cons) = ring_buffer.split();

        let producer = AudioProducer::with_rates(prod, 65536.0, sample_rate as f64);

        let err_fn = |err| error!("Audio output stream error: {:?}", err);

        let stream_config: cpal::StreamConfig = default_config.into();

        let mut last_left = 0.0_f32;
        let mut last_right = 0.0_f32;

        let stream = match stream_config.channels {
            1 => {
                // Mono output
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        for sample in data.iter_mut() {
                            if let Some(s) = cons.try_pop() {
                                last_left = s;
                                *sample = s;
                            } else {
                                // Smooth decay on underrun prevents audio pop
                                last_left *= 0.92;
                                *sample = last_left;
                            }
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            _ => {
                // Stereo / Multi-channel output
                let ch_count = stream_config.channels as usize;
                device.build_output_stream(
                    &stream_config,
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
            }
        };

        stream.play()?;
        info!("Host audio playback stream successfully started ({} Hz).", sample_rate);

        Ok(Self {
            stream: Some(stream),
            producer,
            sample_rate,
            channels,
        })
    }

    /// Returns a cloneable sample producer handle.
    pub fn producer(&self) -> AudioProducer {
        self.producer.clone()
    }

    /// Returns the active audio stream sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Pause audio playback stream.
    pub fn pause(&self) {
        if let Some(ref stream) = self.stream {
            if let Err(err) = stream.pause() {
                warn!("Failed to pause audio stream: {:?}", err);
            }
        }
    }

    /// Resume audio playback stream.
    pub fn resume(&self) {
        if let Some(ref stream) = self.stream {
            if let Err(err) = stream.play() {
                warn!("Failed to resume audio stream: {:?}", err);
            }
        }
    }

    /// Set fast-forward audio mute/throttling state.
    pub fn set_fast_forward(&self, enabled: bool) {
        self.producer.set_fast_forward(enabled);
    }

    /// Sets whether audio output is muted.
    pub fn set_muted(&self, muted: bool) {
        self.producer.set_muted(muted);
    }

    /// Toggles mute state and returns the new muted status.
    pub fn toggle_mute(&self) -> bool {
        self.producer.toggle_mute()
    }

    /// Returns whether audio output is currently muted.
    pub fn is_muted(&self) -> bool {
        self.producer.is_muted()
    }

    /// Sets master volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        self.producer.set_volume(volume);
    }

    /// Returns master volume (0.0 to 1.0).
    pub fn volume(&self) -> f32 {
        self.producer.volume()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_producer_push_stereo() {
        let (producer, mut cons) = AudioProducer::new_pair(32);
        producer.push_stereo_sample(0.5, -0.5);

        assert!(cons.occupied_len() > 0);
        let left = cons.try_pop().unwrap();
        let right = cons.try_pop().unwrap();
        assert!(left > 0.0);
        assert!(right < 0.0);
    }

    #[test]
    fn test_audio_producer_push_i16() {
        let (producer, mut cons) = AudioProducer::new_pair(32);
        producer.push_i16_pair(16384, -16384);

        assert!(cons.occupied_len() > 0);
        let left = cons.try_pop().unwrap();
        let right = cons.try_pop().unwrap();
        assert!(left > 0.0);
        assert!(right < 0.0);
    }

    #[test]
    fn test_audio_mute_toggle() {
        let (producer, cons) = AudioProducer::new_pair(64);
        assert!(!producer.is_muted());

        // Push when unmuted: enters ring buffer
        producer.push_f32_slice(&[0.5, 0.5]);
        assert!(cons.occupied_len() > 0);

        // Mute: incoming samples dropped
        let muted = producer.toggle_mute();
        assert!(muted);
        assert!(producer.is_muted());
        let before_len = cons.occupied_len();
        producer.push_f32_slice(&[0.8, 0.8]);
        assert_eq!(cons.occupied_len(), before_len, "Muted audio must not push samples");

        // Unmute: samples accepted again
        let muted_again = producer.toggle_mute();
        assert!(!muted_again);
        assert!(!producer.is_muted());
        producer.push_f32_slice(&[0.3, 0.3]);
        assert!(cons.occupied_len() > before_len);
    }

    #[test]
    fn test_fast_forward_audio_throttling() {
        let (producer, cons) = AudioProducer::new_pair(64);
        assert!(!producer.is_fast_forward());

        // Normal push: samples should enter ring buffer
        producer.push_f32_slice(&[0.5, 0.5, -0.5, -0.5]);
        assert!(cons.occupied_len() > 0);

        // Enable fast-forward: incoming samples must be discarded
        producer.set_fast_forward(true);
        assert!(producer.is_fast_forward());
        let before_len = cons.occupied_len();
        producer.push_f32_slice(&[0.8, 0.8, -0.8, -0.8]);
        assert_eq!(cons.occupied_len(), before_len, "Samples must be dropped in fast-forward");

        // Disable fast-forward: incoming samples accepted again
        producer.set_fast_forward(false);
        assert!(!producer.is_fast_forward());
        producer.push_f32_slice(&[0.2, 0.2, -0.2, -0.2]);
        assert!(cons.occupied_len() > before_len, "Samples must be accepted after fast-forward");
    }

    #[test]
    fn test_resampler_ratio_scaling() {
        let mut resampler = Resampler::new(65536.0, 48000.0);
        let input: Vec<i16> = (0..1000).map(|i| if i % 2 == 0 { 10000 } else { -10000 }).collect();
        let mut output = Vec::new();
        resampler.resample_i16_slice(&input, &mut output, 1.0);

        let expected_frames = (500.0 * 48000.0 / 65536.0) as usize;
        let actual_frames = output.len() / 2;
        assert!((actual_frames as i32 - expected_frames as i32).abs() <= 2);
    }
}

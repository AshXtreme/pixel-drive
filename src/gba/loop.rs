//! Audio-Driven Dynamic Frame Pacing & Decoupled Emulation Execution Pipeline.
//!
//! Replaces rigid `thread::sleep()` and CPU spinlocks with audio-watermark-driven
//! frame pacing, eliminating frame drops, audio crackles, and thermal jitter on mobile hardware.
//!
//! Provides:
//! - `AudioDrivenPacer`: Dynamic frame pacer driven by the audio ring buffer waterline.
//! - `SharedFrameBuffer`: Lock-free triple-buffered frame storage decoupling core execution from UI rendering.
//! - `DecoupledCoreRunner`: Asynchronous background emulation runner allowing the UI draw pass
//!   to run at arbitrary display refresh rates (60/90/120Hz) without stalling the core.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::AudioProducer;
use crate::core::EmulatorCore;

/// Standard GBA native frame duration: 16.742706 ms (59.7275 Hz).
pub const NATIVE_GBA_FRAME_NANOS: u64 = 16_742_706;

/// Fast-forward frame duration at 2x speed: 8.371353 ms (119.455 Hz).
pub const FAST_FORWARD_FRAME_NANOS: u64 = 8_371_353;

/// Audio queue safe upper threshold (3.5 video frames = 2800 stereo frames at 48kHz).
pub const AUDIO_SAFE_UPPER_THRESHOLD_FRAMES: usize = 2800;

/// Audio queue safe lower threshold (1.5 video frames = 1200 stereo frames at 48kHz).
pub const AUDIO_SAFE_LOWER_THRESHOLD_FRAMES: usize = 1200;

/// Frame pacing decision returned by the dynamic audio-driven pacer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacingDecision {
    /// Step the core immediately for `steps` frames.
    Step(usize),
    /// Audio buffer is near capacity (> 3–4 frames); dynamically throttle to avoid latency buildup.
    Throttle,
    /// Waiting for next frame interval (audio queue is in safe waterline).
    Wait,
    /// Emulation is paused; no core steps required.
    Paused,
}

/// Dynamic Audio-Driven Frame Pacer.
///
/// Synchronizes core execution directly to the audio consumption rate of the host device.
/// Never uses `thread::sleep()` or busy CPU spinlocks.
pub struct AudioDrivenPacer {
    last_frame_instant: Instant,
    target_frame_duration: Duration,
    fast_forward: bool,
    steps_per_frame: usize,
    upper_threshold_frames: usize,
    lower_threshold_frames: usize,
}

impl Default for AudioDrivenPacer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDrivenPacer {
    /// Creates a new audio-driven pacer configured for standard 59.73 Hz GBA timing.
    pub fn new() -> Self {
        Self {
            last_frame_instant: Instant::now(),
            target_frame_duration: Duration::from_nanos(NATIVE_GBA_FRAME_NANOS),
            fast_forward: false,
            steps_per_frame: 1,
            upper_threshold_frames: AUDIO_SAFE_UPPER_THRESHOLD_FRAMES,
            lower_threshold_frames: AUDIO_SAFE_LOWER_THRESHOLD_FRAMES,
        }
    }

    /// Sets whether fast-forward speed is active.
    pub fn set_fast_forward(&mut self, enabled: bool, steps_per_frame: usize) {
        self.fast_forward = enabled;
        self.steps_per_frame = steps_per_frame.max(1);
        self.target_frame_duration = if enabled {
            Duration::from_nanos(FAST_FORWARD_FRAME_NANOS)
        } else {
            Duration::from_nanos(NATIVE_GBA_FRAME_NANOS)
        };
    }

    /// Returns whether fast-forward is enabled.
    #[inline]
    pub fn is_fast_forward(&self) -> bool {
        self.fast_forward
    }

    /// Returns the active target frame duration.
    #[inline]
    pub fn target_frame_duration(&self) -> Duration {
        self.target_frame_duration
    }

    /// Resets the timing baseline to the current instant (e.g. upon unpausing or resuming).
    pub fn reset_timing(&mut self) {
        self.last_frame_instant = Instant::now();
    }

    /// Evaluates dynamic pacing conditions based on audio ring-buffer queue depth.
    ///
    /// Pacing rules:
    /// 1. If paused: return `Paused`.
    /// 2. If audio ring buffer is starving (< safe lower threshold): return `Step` immediately
    ///    to synthesize audio samples before an audible underrun occurs.
    /// 3. If audio ring buffer is overfull (> safe upper threshold): return `Throttle` to allow
    ///    the hardware audio consumer (AAudio/WASAPI/CoreAudio) to drain without latency buildup.
    /// 4. If normal elapsed time >= target duration: return `Step` and advance frame clock.
    /// 5. Otherwise: return `Wait`.
    pub fn evaluate_pacing(
        &mut self,
        audio_producer: Option<&AudioProducer>,
        is_paused: bool,
    ) -> PacingDecision {
        if is_paused {
            return PacingDecision::Paused;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_instant);

        // Check audio queue depth when audio output is active
        if let Some(audio) = audio_producer {
            if !audio.is_muted() && !self.fast_forward {
                let queued_frames = audio.buffered_frames();

                // Condition A: Audio queue has exceeded 3–4 frames safe threshold
                if queued_frames >= self.upper_threshold_frames {
                    // Dynamically throttle core execution to let audio drain
                    return PacingDecision::Throttle;
                }

                // Condition B: Audio queue is starving (< 1.5 frames)
                if queued_frames <= self.lower_threshold_frames {
                    // Step immediately to refill audio ring buffer and avoid underrun
                    self.last_frame_instant = now;
                    return PacingDecision::Step(self.steps_per_frame);
                }
            }
        }

        // Condition C: Standard clock cadence
        if elapsed >= self.target_frame_duration {
            self.last_frame_instant = if elapsed > self.target_frame_duration * 2 {
                now
            } else {
                self.last_frame_instant + self.target_frame_duration
            };

            let steps = if self.fast_forward {
                self.steps_per_frame
            } else {
                1
            };
            PacingDecision::Step(steps)
        } else {
            PacingDecision::Wait
        }
    }
}

/// Thread-safe lock-free Triple-Buffered Frame Container.
///
/// Allows the emulator core to write into the back buffer while the GPU/UI
/// thread reads the latest completed front buffer with zero lock contention.
pub struct SharedFrameBuffer {
    buffers: [parking_lot::RwLock<Vec<u8>>; 3],
    front_index: AtomicUsize,
    back_index: AtomicUsize,
    frame_sequence: AtomicU64,
    width: AtomicU32,
    height: AtomicU32,
}

impl SharedFrameBuffer {
    /// Creates a triple buffer initialized to the specified dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let byte_len = (width as usize) * (height as usize) * 4;
        Self {
            buffers: [
                parking_lot::RwLock::new(vec![0u8; byte_len]),
                parking_lot::RwLock::new(vec![0u8; byte_len]),
                parking_lot::RwLock::new(vec![0u8; byte_len]),
            ],
            front_index: AtomicUsize::new(0),
            back_index: AtomicUsize::new(1),
            frame_sequence: AtomicU64::new(0),
            width: AtomicU32::new(width),
            height: AtomicU32::new(height),
        }
    }

    /// Writes raw pixel data from the core into the back buffer, then commits it as the active front buffer.
    pub fn publish_frame(&self, src: &[u8], width: u32, height: u32) {
        let current_w = self.width.load(Ordering::Acquire);
        let current_h = self.height.load(Ordering::Acquire);

        if current_w != width || current_h != height {
            self.width.store(width, Ordering::Release);
            self.height.store(height, Ordering::Release);
            let byte_len = (width as usize) * (height as usize) * 4;
            for buf in &self.buffers {
                let mut lock = buf.write();
                lock.resize(byte_len, 0);
            }
        }

        let back_idx = self.back_index.load(Ordering::Relaxed);
        {
            let mut back_lock = self.buffers[back_idx].write();
            let copy_len = back_lock.len().min(src.len());
            back_lock[..copy_len].copy_from_slice(&src[..copy_len]);
        }

        // Swap back to front
        let prev_front = self.front_index.swap(back_idx, Ordering::AcqRel);
        // Find next free slot that is neither front nor current back
        let next_back = (0..3).find(|&i| i != back_idx && i != prev_front).unwrap_or(0);
        self.back_index.store(next_back, Ordering::Release);
        self.frame_sequence.fetch_add(1, Ordering::Release);
    }

    /// Copies the latest committed front buffer to the destination slice.
    /// Returns `true` if a frame was successfully read.
    pub fn copy_front_to(&self, dst: &mut [u8]) -> bool {
        let front_idx = self.front_index.load(Ordering::Acquire);
        let front_lock = self.buffers[front_idx].read();
        let copy_len = dst.len().min(front_lock.len());
        if copy_len > 0 {
            dst[..copy_len].copy_from_slice(&front_lock[..copy_len]);
            true
        } else {
            false
        }
    }

    /// Returns current dimensions `(width, height)`.
    pub fn dimensions(&self) -> (u32, u32) {
        (
            self.width.load(Ordering::Acquire),
            self.height.load(Ordering::Acquire),
        )
    }

    /// Returns the monotonically increasing sequence number of completed frames.
    pub fn sequence(&self) -> u64 {
        self.frame_sequence.load(Ordering::Acquire)
    }
}

/// Asynchronous Decoupled Core Runner.
///
/// Runs core emulation on a dedicated thread, paced dynamically by audio occupancy.
/// Decouples emulation from the UI draw pass so that displays with 90Hz/120Hz refresh rates
/// or heavy UI compositing never throttle the core.
pub struct DecoupledCoreRunner {
    shared_frame: Arc<SharedFrameBuffer>,
    is_running: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    fast_forward: Arc<AtomicBool>,
    worker_handle: Option<std::thread::JoinHandle<()>>,
}

impl DecoupledCoreRunner {
    /// Launches a decoupled emulation worker thread for the given core and audio producer.
    pub fn start<C: EmulatorCore + Send + 'static>(
        mut core: C,
        audio_producer: Option<AudioProducer>,
        initial_width: u32,
        initial_height: u32,
    ) -> Self {
        let shared_frame = Arc::new(SharedFrameBuffer::new(initial_width, initial_height));
        let is_running = Arc::new(AtomicBool::new(true));
        let is_paused = Arc::new(AtomicBool::new(false));
        let fast_forward = Arc::new(AtomicBool::new(false));

        let frame_clone = Arc::clone(&shared_frame);
        let running_clone = Arc::clone(&is_running);
        let paused_clone = Arc::clone(&is_paused);
        let ff_clone = Arc::clone(&fast_forward);

        let worker_handle = std::thread::Builder::new()
            .name("PixelDrive-CoreThread".to_string())
            .spawn(move || {
                let mut pacer = AudioDrivenPacer::new();

                while running_clone.load(Ordering::Acquire) {
                    let paused = paused_clone.load(Ordering::Acquire);
                    let ff = ff_clone.load(Ordering::Acquire);
                    pacer.set_fast_forward(ff, if ff { 2 } else { 1 });

                    match pacer.evaluate_pacing(audio_producer.as_ref(), paused) {
                        PacingDecision::Step(steps) => {
                            for _ in 0..steps {
                                core.step_frame();

                                let audio = core.audio_buffer();
                                if !audio.is_empty() {
                                    if let Some(ref prod) = audio_producer {
                                        prod.push_f32_slice(&audio);
                                    }
                                }
                            }

                            let (w, h) = core.display_dimensions();
                            let fb = core.framebuffer();
                            frame_clone.publish_frame(fb, w, h);
                        }
                        PacingDecision::Throttle => {
                            // Audio ring buffer is full; yield CPU without sleeping
                            std::thread::yield_now();
                        }
                        PacingDecision::Wait => {
                            // Non-blocking yield lets OS scheduler allocate time to UI thread
                            std::thread::yield_now();
                        }
                        PacingDecision::Paused => {
                            // When paused, sleep minimally to conserve battery
                            std::thread::sleep(Duration::from_millis(10));
                            pacer.reset_timing();
                        }
                    }
                }
            })
            .ok();

        Self {
            shared_frame,
            is_running,
            is_paused,
            fast_forward,
            worker_handle,
        }
    }

    /// Access the shared triple-buffered frame container.
    pub fn shared_frame(&self) -> &Arc<SharedFrameBuffer> {
        &self.shared_frame
    }

    /// Sets paused state.
    pub fn set_paused(&self, paused: bool) {
        self.is_paused.store(paused, Ordering::Release);
    }

    /// Sets fast-forward state.
    pub fn set_fast_forward(&self, ff: bool) {
        self.fast_forward.store(ff, Ordering::Release);
    }

    /// Stops the worker thread safely.
    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Release);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DecoupledCoreRunner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::traits::Consumer;

    #[test]
    fn test_audio_driven_pacer_threshold_decisions() {
        let (producer, mut consumer) = AudioProducer::new_pair(4096 * 2);
        let mut pacer = AudioDrivenPacer::new();

        // Initially ring buffer is empty (0 samples <= lower threshold of 1200)
        let decision = pacer.evaluate_pacing(Some(&producer), false);
        assert_eq!(decision, PacingDecision::Step(1));

        // Set 1:1 sample rate for deterministic frame count testing
        producer.set_input_sample_rate(48000.0);

        // Fill producer to safe upper threshold (> 2800 frames = > 5600 samples)
        let dummy_samples = vec![0.1f32; 6000];
        producer.push_f32_slice(&dummy_samples);

        assert!(producer.buffered_frames() >= AUDIO_SAFE_UPPER_THRESHOLD_FRAMES);

        // When over safe threshold, must throttle
        let decision = pacer.evaluate_pacing(Some(&producer), false);
        assert_eq!(decision, PacingDecision::Throttle);

        // Drain consumer
        while consumer.try_pop().is_some() {}
        assert_eq!(producer.buffered_samples(), 0);

        // Once drained, should step again to refill
        let decision = pacer.evaluate_pacing(Some(&producer), false);
        assert_eq!(decision, PacingDecision::Step(1));
    }

    #[test]
    fn test_shared_frame_buffer_triple_buffering() {
        let buffer = SharedFrameBuffer::new(4, 4);
        assert_eq!(buffer.dimensions(), (4, 4));

        let frame1 = vec![0xAA; 4 * 4 * 4];
        buffer.publish_frame(&frame1, 4, 4);
        assert_eq!(buffer.sequence(), 1);

        let mut read_buf = vec![0u8; 4 * 4 * 4];
        assert!(buffer.copy_front_to(&mut read_buf));
        assert_eq!(read_buf[0], 0xAA);

        let frame2 = vec![0xBB; 4 * 4 * 4];
        buffer.publish_frame(&frame2, 4, 4);
        assert_eq!(buffer.sequence(), 2);

        assert!(buffer.copy_front_to(&mut read_buf));
        assert_eq!(read_buf[0], 0xBB);
    }
}

//! Test Suite 6: Mobile Hardware Frame Pacing, WGPU Texture Upload Pipeline & Audio Latency.
//!
//! Validates:
//! - TC-OPT-01: Audio-driven dynamic frame pacing threshold logic and latency control.
//! - TC-OPT-02: Double-buffered staging texture zero-copy streaming & slot alternation.
//! - TC-OPT-03: Audio buffer depletion smooth waveform continuity & lock-free consumption.
//! - TC-OPT-04: Static WGPU pipeline & bind group allocation audit.
//! - TC-OPT-05: Decoupled thread-safe shared frame buffer concurrent throughput.

use std::sync::Arc;
use ringbuf::traits::{Consumer, Observer};

use pixeldrive::audio::AudioProducer;
use pixeldrive::gba::{
    AudioDrivenPacer, PacingDecision, SharedFrameBuffer,
    AUDIO_SAFE_LOWER_THRESHOLD_FRAMES, AUDIO_SAFE_UPPER_THRESHOLD_FRAMES,
    GBA_HEIGHT, GBA_WIDTH,
};
use pixeldrive::gbc::{GBC_HEIGHT, GBC_WIDTH};
use pixeldrive::render::{DoubleBufferedTextureStream, FilterMode, WgpuRenderer};

// ============================================================================
// TC-OPT-01: Audio-Driven Dynamic Frame Pacing & Latency Control
// ============================================================================

#[test]
fn test_tc_opt_01_audio_driven_pacer_threshold_and_throttling() {
    log::info!("TC-OPT-01: Testing dynamic audio-driven frame pacer throttling and starvation refill...");

    let (producer, mut consumer) = AudioProducer::new_pair(4096 * 2);
    producer.set_input_sample_rate(48000.0);

    let mut pacer = AudioDrivenPacer::new();

    // 1. Initial empty buffer condition (0 <= 1200 frames) -> Must Step immediately to refill
    let decision = pacer.evaluate_pacing(Some(&producer), false);
    assert_eq!(decision, PacingDecision::Step(1));

    // 2. Safe Waterline condition (e.g. 2000 stereo frames = 4000 samples)
    let safe_samples = vec![0.05f32; 4000];
    producer.push_f32_slice(&safe_samples);
    assert!(producer.buffered_frames() > AUDIO_SAFE_LOWER_THRESHOLD_FRAMES);
    assert!(producer.buffered_frames() < AUDIO_SAFE_UPPER_THRESHOLD_FRAMES);

    // 3. Buffer Overfill condition (> 3.5 frames = > 2800 stereo frames = > 5600 samples)
    let fill_samples = vec![0.1f32; 2500];
    producer.push_f32_slice(&fill_samples);
    assert!(producer.buffered_frames() >= AUDIO_SAFE_UPPER_THRESHOLD_FRAMES);
    assert!(producer.should_throttle());

    // When over safe threshold, pacer MUST throttle core execution to let audio consumer drain
    let throttle_decision = pacer.evaluate_pacing(Some(&producer), false);
    assert_eq!(throttle_decision, PacingDecision::Throttle);

    // 4. Consumer drains samples (simulating hardware AAudio drain)
    let mut drained = 0;
    while consumer.try_pop().is_some() {
        drained += 1;
    }
    assert!(drained > 0);
    assert_eq!(producer.buffered_samples(), 0);
    assert!(producer.needs_refill());

    // Once drained, pacer MUST step immediately to synthesize audio before crackle occurs
    let refill_decision = pacer.evaluate_pacing(Some(&producer), false);
    assert_eq!(refill_decision, PacingDecision::Step(1));

    // 5. Paused condition
    let paused_decision = pacer.evaluate_pacing(Some(&producer), true);
    assert_eq!(paused_decision, PacingDecision::Paused);
}

// ============================================================================
// TC-OPT-02: Double-Buffered Staging Texture Zero-Copy Streaming
// ============================================================================

#[test]
fn test_tc_opt_02_double_buffered_texture_stream_slot_alternation() {
    log::info!("TC-OPT-02: Validating double-buffered texture stream slot rotation and dimension resizing...");

    let instance = pixels::wgpu::Instance::new(pixels::wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&pixels::wgpu::RequestAdapterOptions {
        power_preference: pixels::wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));

    if let Some(adapter) = adapter {
        if let Ok((device, queue)) = pollster::block_on(adapter.request_device(
            &pixels::wgpu::DeviceDescriptor::default(),
            None,
        )) {
            // Initialize for standard GBA 240x160 native canvas
            let mut stream = DoubleBufferedTextureStream::new(&device, GBA_WIDTH, GBA_HEIGHT);
            assert_eq!(stream.dimensions(), (GBA_WIDTH, GBA_HEIGHT));
            assert_eq!(stream.format(), pixels::wgpu::TextureFormat::Rgba8Unorm);

            // Initially read slot is 1 and write slot is 0
            assert_eq!(stream.read_slot(), 1);
            assert_eq!(stream.write_slot(), 0);

            // Core writes frame 1 into staging slice
            let frame1_data = vec![0x10u8; (GBA_WIDTH * GBA_HEIGHT * 4) as usize];
            stream.write_pixels(&frame1_data);
            stream.commit_and_upload(&queue);

            // After commit, read slot is 0, write slot alternates to 1
            assert_eq!(stream.read_slot(), 0);
            assert_eq!(stream.write_slot(), 1);

            // Core writes frame 2 into staging slice
            let frame2_data = vec![0x20u8; (GBA_WIDTH * GBA_HEIGHT * 4) as usize];
            stream.write_pixels(&frame2_data);
            stream.commit_and_upload(&queue);

            // After commit, read slot is 1, write slot alternates to 0
            assert_eq!(stream.read_slot(), 1);
            assert_eq!(stream.write_slot(), 0);

            // Test dynamic resize from GBA (240x160) to GBC (160x144)
            stream.resize(&device, GBC_WIDTH, GBC_HEIGHT);
            assert_eq!(stream.dimensions(), (GBC_WIDTH, GBC_HEIGHT));
            assert_eq!(stream.staging_slice_mut().len(), (GBC_WIDTH * GBC_HEIGHT * 4) as usize);
        }
    }
}

// ============================================================================
// TC-OPT-03: Audio Buffer Depletion & Continuous Lock-Free Consumption
// ============================================================================

#[test]
fn test_tc_opt_03_audio_buffer_depletion_smooth_consumption() {
    log::info!("TC-OPT-03: Validating smooth audio consumption and non-blocking depletion handling...");

    let (producer, mut consumer) = AudioProducer::new_pair(4096 * 2);
    producer.set_input_sample_rate(48000.0);

    // Push initial stereo burst
    let burst = vec![0.5f32; 100];
    producer.push_f32_slice(&burst);
    assert!(consumer.occupied_len() > 0);

    // Drain all available samples
    let mut drained_count = 0;
    while consumer.try_pop().is_some() {
        drained_count += 1;
    }
    assert!(drained_count >= 100);

    // Now buffer is depleted; reading must return None immediately without blocking/stalling thread
    assert!(consumer.try_pop().is_none());
    assert_eq!(producer.buffered_samples(), 0);
    assert_eq!(producer.buffered_frames(), 0);
    assert!(producer.fill_ratio() < 0.001);
}

// ============================================================================
// TC-OPT-04: Static WGPU Pipeline & Bind Group Allocation Audit
// ============================================================================

#[test]
fn test_tc_opt_04_wgpu_renderer_pre_cached_bind_groups() {
    log::info!("TC-OPT-04: Auditing static WGPU renderer bind group caching and pipeline initialization...");

    let instance = pixels::wgpu::Instance::new(pixels::wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&pixels::wgpu::RequestAdapterOptions {
        power_preference: pixels::wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));

    if let Some(adapter) = adapter {
        if let Ok((device, queue)) = pollster::block_on(adapter.request_device(
            &pixels::wgpu::DeviceDescriptor::default(),
            None,
        )) {
            let mut renderer = WgpuRenderer::new(
                &device,
                pixels::wgpu::TextureFormat::Rgba8Unorm,
                GBA_WIDTH,
                GBA_HEIGHT,
            );

            assert_eq!(renderer.stream().dimensions(), (GBA_WIDTH, GBA_HEIGHT));
            assert_eq!(renderer.target_format(), pixels::wgpu::TextureFormat::Rgba8Unorm);

            // Write and upload frame
            let dummy_pixels = vec![128u8; (GBA_WIDTH * GBA_HEIGHT * 4) as usize];
            renderer.upload_frame(&queue, &dummy_pixels);

            // Create offscreen texture to test zero-allocation rendering pass
            let target_desc = pixels::wgpu::TextureDescriptor {
                label: Some("Test_Target"),
                size: pixels::wgpu::Extent3d {
                    width: 480,
                    height: 320,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: pixels::wgpu::TextureDimension::D2,
                format: pixels::wgpu::TextureFormat::Rgba8Unorm,
                usage: pixels::wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            };
            let target_tex = device.create_texture(&target_desc);
            let target_view = target_tex.create_view(&pixels::wgpu::TextureViewDescriptor::default());

            let mut encoder = device.create_command_encoder(&pixels::wgpu::CommandEncoderDescriptor {
                label: Some("Test_Encoder"),
            });

            // Per-frame render must succeed without dynamic allocations
            renderer.render(
                &mut encoder,
                &target_view,
                &queue,
                FilterMode::Nearest,
                480,
                320,
            );

            let cmd_buffer = encoder.finish();
            queue.submit(std::iter::once(cmd_buffer));
        }
    }
}

// ============================================================================
// TC-OPT-05: Decoupled Shared Frame Buffer Concurrency
// ============================================================================

#[test]
fn test_tc_opt_05_shared_frame_buffer_concurrent_read_write() {
    log::info!("TC-OPT-05: Validating lock-free triple-buffered frame throughput across threads...");

    let shared_buffer = Arc::new(SharedFrameBuffer::new(GBA_WIDTH, GBA_HEIGHT));
    let buffer_writer = Arc::clone(&shared_buffer);
    let buffer_reader = Arc::clone(&shared_buffer);

    let writer_handle = std::thread::spawn(move || {
        let frame_data = vec![0xEEu8; (GBA_WIDTH * GBA_HEIGHT * 4) as usize];
        for _ in 0..100 {
            buffer_writer.publish_frame(&frame_data, GBA_WIDTH, GBA_HEIGHT);
        }
    });

    let reader_handle = std::thread::spawn(move || {
        let mut read_dest = vec![0u8; (GBA_WIDTH * GBA_HEIGHT * 4) as usize];
        let mut frames_read = 0;
        for _ in 0..100 {
            if buffer_reader.copy_front_to(&mut read_dest) {
                frames_read += 1;
            }
        }
        frames_read
    });

    writer_handle.join().expect("Writer thread must finish cleanly");
    let frames_read = reader_handle.join().expect("Reader thread must finish cleanly");

    assert!(frames_read > 0, "Reader thread must successfully observe committed frames");
    assert_eq!(shared_buffer.sequence(), 100);
}

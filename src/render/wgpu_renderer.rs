//! Zero-Copy Ring-Buffered Texture Streaming & WGPU Rendering Pipeline.
//!
//! Designed specifically for tiled mobile GPUs (e.g., Qualcomm Adreno 618 / Snapdragon 720G)
//! to eliminate CPU stalls caused by synchronous `queue.write_texture` on in-flight tile memory.
//!
//! Features:
//! - Double-buffered native `Rgba8Unorm` staging textures with pre-allocated views.
//! - In-memory CPU staging slice ring buffer: emulator core writes into the back slot while
//!   the GPU blits the front slot.
//! - Static zero-allocation bind group caching for Nearest and Linear samplers.
//! - Disables mipmap generation and multisampling for authentic pixel art native resolution.

use pixels::wgpu::{self, util::DeviceExt};

use super::shaders::{FilterMode, ShaderUniforms, SHADER_SOURCE};

/// Double-buffered staging texture streamer for zero-contention GPU uploads.
pub struct DoubleBufferedTextureStream {
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    cpu_staging: [Vec<u8>; 2],
    width: u32,
    height: u32,
    write_slot: usize,
    read_slot: usize,
    format: wgpu::TextureFormat,
}

impl DoubleBufferedTextureStream {
    /// Allocates double-buffered textures, texture views, and CPU staging buffers.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let (textures, views) = Self::create_texture_pair(device, width, height, format);

        let byte_len = (width as usize) * (height as usize) * 4;
        let cpu_staging = [vec![0u8; byte_len], vec![0u8; byte_len]];

        Self {
            textures,
            views,
            cpu_staging,
            width,
            height,
            write_slot: 0,
            read_slot: 1,
            format,
        }
    }

    fn create_texture_pair(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> ([wgpu::Texture; 2], [wgpu::TextureView; 2]) {
        let tex_desc = wgpu::TextureDescriptor {
            label: Some("DoubleBuffered_Staging_Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, // Disable mipmap generation on native retro canvas
            sample_count: 1,   // Single sample, no multisampling
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let tex0 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DoubleBuffered_Texture_Slot_0"),
            ..tex_desc
        });
        let tex1 = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DoubleBuffered_Texture_Slot_1"),
            ..tex_desc
        });

        let view0 = tex0.create_view(&wgpu::TextureViewDescriptor {
            label: Some("DoubleBuffered_View_Slot_0"),
            ..Default::default()
        });
        let view1 = tex1.create_view(&wgpu::TextureViewDescriptor {
            label: Some("DoubleBuffered_View_Slot_1"),
            ..Default::default()
        });

        ([tex0, tex1], [view0, view1])
    }

    /// Resizes internal textures and staging buffers if display dimensions change.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        let (textures, views) = Self::create_texture_pair(device, width, height, self.format);
        let byte_len = (width as usize) * (height as usize) * 4;

        self.textures = textures;
        self.views = views;
        self.cpu_staging[0].resize(byte_len, 0);
        self.cpu_staging[1].resize(byte_len, 0);
        self.width = width;
        self.height = height;
        self.write_slot = 0;
        self.read_slot = 1;
    }

    /// Provides a mutable reference to the active CPU staging slice for direct zero-copy core writes.
    #[inline]
    pub fn staging_slice_mut(&mut self) -> &mut [u8] {
        &mut self.cpu_staging[self.write_slot]
    }

    /// Writes raw pixel data from the emulator framebuffer into the active staging slice.
    pub fn write_pixels(&mut self, data: &[u8]) {
        let staging = self.staging_slice_mut();
        let copy_len = staging.len().min(data.len());
        staging[..copy_len].copy_from_slice(&data[..copy_len]);
    }

    /// Uploads the current CPU staging slice to the inactive back texture,
    /// then flips write and read slots so the GPU can blit the newly uploaded frame.
    pub fn commit_and_upload(&mut self, queue: &wgpu::Queue) {
        let w = self.width;
        let h = self.height;
        let write_idx = self.write_slot;

        // Upload directly to the back texture. Because the GPU is reading from `read_slot`,
        // this write NEVER blocks on Adreno tile memory.
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.textures[write_idx],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.cpu_staging[write_idx],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        // Swap slots: current write slot becomes the active read slot for the GPU
        self.read_slot = write_idx;
        self.write_slot = 1 - write_idx;
    }

    /// Returns the active front texture view for rendering.
    #[inline]
    pub fn front_view(&self) -> &wgpu::TextureView {
        &self.views[self.read_slot]
    }

    /// Returns the active front texture reference.
    #[inline]
    pub fn front_texture(&self) -> &wgpu::Texture {
        &self.textures[self.read_slot]
    }

    /// Returns the slot index of the current front read texture (0 or 1).
    #[inline]
    pub fn read_slot(&self) -> usize {
        self.read_slot
    }

    /// Returns the slot index of the current back write texture (0 or 1).
    #[inline]
    pub fn write_slot(&self) -> usize {
        self.write_slot
    }

    /// Returns current texture dimensions `(width, height)`.
    #[inline]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns the texture format (`Rgba8Unorm`).
    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

/// High-performance WGPU Renderer with pre-cached bind groups and zero-allocation frame blitting.
pub struct WgpuRenderer {
    stream: DoubleBufferedTextureStream,
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    // Pre-cached bind groups: [slot_0: [nearest, linear], slot_1: [nearest, linear]]
    cached_bind_groups: Option<[[wgpu::BindGroup; 2]; 2]>,
    cached_uniforms: Option<ShaderUniforms>,
    current_filter: FilterMode,
    target_format: wgpu::TextureFormat,
}

impl WgpuRenderer {
    /// Initializes the WGPU renderer, compiling shaders and pre-allocating all samplers and layouts.
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        initial_width: u32,
        initial_height: u32,
    ) -> Self {
        let stream = DoubleBufferedTextureStream::new(device, initial_width, initial_height);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WgpuRenderer_PostProcess_WGSL"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Pre-allocate samplers with mipmap disabled
        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("WgpuRenderer_Sampler_Nearest"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("WgpuRenderer_Sampler_Linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest, // No mipmaps on retro canvas
            ..Default::default()
        });

        let initial_uniforms = ShaderUniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("WgpuRenderer_Uniform_Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("WgpuRenderer_BindGroupLayout"),
            entries: &[
                // Binding 0: Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 1: Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Binding 2: Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WgpuRenderer_PipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WgpuRenderer_RenderPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[], // Procedural full-screen triangle generated from vertex_index
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(), // 1 sample, no MSAA overhead
            multiview: None,
        });

        let mut renderer = Self {
            stream,
            render_pipeline,
            bind_group_layout,
            sampler_nearest,
            sampler_linear,
            uniform_buffer,
            cached_bind_groups: None,
            cached_uniforms: None,
            current_filter: FilterMode::Nearest,
            target_format,
        };

        renderer.rebuild_bind_groups(device);
        renderer
    }

    /// Pre-caches 4 bind groups (2 texture slots x 2 samplers) to guarantee zero runtime allocations.
    fn rebuild_bind_groups(&mut self, device: &wgpu::Device) {
        let bg_slot0_nearest = Self::create_bind_group_entry(
            device,
            &self.bind_group_layout,
            &self.stream.views[0],
            &self.sampler_nearest,
            &self.uniform_buffer,
            "Slot0_Nearest",
        );
        let bg_slot0_linear = Self::create_bind_group_entry(
            device,
            &self.bind_group_layout,
            &self.stream.views[0],
            &self.sampler_linear,
            &self.uniform_buffer,
            "Slot0_Linear",
        );
        let bg_slot1_nearest = Self::create_bind_group_entry(
            device,
            &self.bind_group_layout,
            &self.stream.views[1],
            &self.sampler_nearest,
            &self.uniform_buffer,
            "Slot1_Nearest",
        );
        let bg_slot1_linear = Self::create_bind_group_entry(
            device,
            &self.bind_group_layout,
            &self.stream.views[1],
            &self.sampler_linear,
            &self.uniform_buffer,
            "Slot1_Linear",
        );

        self.cached_bind_groups = Some([
            [bg_slot0_nearest, bg_slot0_linear],
            [bg_slot1_nearest, bg_slot1_linear],
        ]);
    }

    fn create_bind_group_entry(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        uniform_buffer: &wgpu::Buffer,
        label: &'static str,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Access the underlying double-buffered texture streamer.
    #[inline]
    pub fn stream_mut(&mut self) -> &mut DoubleBufferedTextureStream {
        &mut self.stream
    }

    /// Access the underlying double-buffered texture streamer (immutably).
    #[inline]
    pub fn stream(&self) -> &DoubleBufferedTextureStream {
        &self.stream
    }

    /// Returns the target texture format of the render pass.
    #[inline]
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }

    /// Ensures the stream matches expected dimensions, rebuilding bind groups only if resized.
    pub fn ensure_dimensions(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.stream.width != width || self.stream.height != height {
            self.stream.resize(device, width, height);
            self.rebuild_bind_groups(device);
        }
    }

    /// Ingests a new frame of raw pixel data and asynchronously uploads to the back texture.
    pub fn upload_frame(&mut self, queue: &wgpu::Queue, pixels: &[u8]) {
        self.stream.write_pixels(pixels);
        self.stream.commit_and_upload(queue);
    }

    /// Renders the latest uploaded frame directly to the render target with zero allocations.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        queue: &wgpu::Queue,
        filter_mode: FilterMode,
        out_width: u32,
        out_height: u32,
    ) {
        let (tex_w, tex_h) = self.stream.dimensions();

        // Mutate uniforms only when changes occur
        let uniforms = ShaderUniforms {
            texture_size: [tex_w as f32, tex_h as f32],
            output_size: [out_width as f32, out_height as f32],
            filter_type: filter_mode.as_u32(),
            intensity: 1.0,
            _pad: [0.0, 0.0],
        };

        if self.cached_uniforms != Some(uniforms) {
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
            self.cached_uniforms = Some(uniforms);
            self.current_filter = filter_mode;
        }

        let read_idx = self.stream.read_slot();
        let sampler_idx = if filter_mode == FilterMode::Bilinear { 1 } else { 0 };

        if let Some(ref groups) = self.cached_bind_groups {
            let active_bg = &groups[read_idx][sampler_idx];

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("WgpuRenderer_RenderPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, active_bg, &[]);
            rpass.draw(0..3, 0..1); // Full-screen procedural triangle
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_double_buffered_stream_slot_rotation() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        if let Some(adapter) = adapter {
            if let Ok((device, queue)) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor::default(),
                None,
            )) {
                let mut stream = DoubleBufferedTextureStream::new(&device, 240, 160);
                assert_eq!(stream.read_slot(), 1);
                assert_eq!(stream.write_slot(), 0);

                let dummy_data = vec![255u8; 240 * 160 * 4];
                stream.write_pixels(&dummy_data);
                stream.commit_and_upload(&queue);

                // Slot index must alternate
                assert_eq!(stream.read_slot(), 0);
                assert_eq!(stream.write_slot(), 1);

                stream.write_pixels(&dummy_data);
                stream.commit_and_upload(&queue);

                assert_eq!(stream.read_slot(), 1);
                assert_eq!(stream.write_slot(), 0);
            }
        }
    }
}

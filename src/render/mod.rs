pub mod shaders;

use pixels::wgpu::{self, util::DeviceExt};
pub use shaders::FilterMode;
use shaders::{ShaderUniforms, SHADER_SOURCE};

/// Post-processing Shader Pipeline managing WGSL shader passes, filter modes, and samplers.
pub struct ShaderPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    current_filter: FilterMode,
    current_tex_size: (u32, u32),
}

impl ShaderPipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PixelDrive_PostProcess_WGSL"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler_Nearest"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler_Linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let initial_uniforms = ShaderUniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shader_Uniform_Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PostProcess_BindGroupLayout"),
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
                // Binding 1: Sampler Nearest
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Binding 2: Sampler Linear
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Binding 3: Uniform Buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
            label: Some("PostProcess_PipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PostProcess_RenderPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler_nearest,
            sampler_linear,
            uniform_buffer,
            bind_group: None,
            current_filter: FilterMode::Nearest,
            current_tex_size: (0, 0),
        }
    }

    /// Renders the internal pixel buffer to the swapchain render target using the active WGSL filter.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &pixels::PixelsContext,
        filter_mode: FilterMode,
        tex_width: u32,
        tex_height: u32,
        out_width: u32,
        out_height: u32,
    ) {
        // Recreate bind group if texture or dimension changed
        if self.bind_group.is_none() || self.current_tex_size != (tex_width, tex_height) {
            let texture_view = context.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PostProcess_BindGroup"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                ],
            });
            self.bind_group = Some(bind_group);
            self.current_tex_size = (tex_width, tex_height);
        }

        // Update uniforms
        let uniforms = ShaderUniforms {
            texture_size: [tex_width as f32, tex_height as f32],
            output_size: [out_width as f32, out_height as f32],
            filter_type: filter_mode.as_u32(),
            intensity: 1.0,
            _pad: [0.0, 0.0],
        };
        context.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        self.current_filter = filter_mode;

        // Perform post-processing pass
        if let Some(ref bind_group) = self.bind_group {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PostProcess_RenderPass"),
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
            rpass.set_bind_group(0, bind_group, &[]);
            rpass.draw(0..3, 0..1); // Full-screen triangle!
        }
    }
}

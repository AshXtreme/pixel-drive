//! Home Screen / Library Carousel UI Subsystem
//!
//! Provides the interactive carousel state machine, touch gesture hit-testing,
//! and dedicated WGPU rendering pipeline for the console-grade game library.

use std::fs;
use std::path::Path;
use log::{info, warn};
use pixels::wgpu::{self, util::DeviceExt};

use crate::library::{decode_jpeg, LibraryManager, RomEntry};

/// Embedded WGSL shader source for Home Screen Carousel rendering.
pub const HOME_SCREEN_SHADER_SOURCE: &str = include_str!("../../shaders/home_screen.wgsl");

/// Action dispatched by the Home Screen to the runtime event loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeScreenAction {
    /// Boot the selected game ROM into active emulation.
    LaunchRom(RomEntry),
    /// Open the native document picker to select a new ROM.
    AddGame,
}

/// Uniforms structure passed to `shaders/home_screen.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HomeScreenUniforms {
    pub screen_size: [f32; 2],
    pub aspect_ratio: f32,
    pub scroll_offset: f32,

    pub selected_index: u32,
    pub total_tiles: u32,
    pub anim_time: f32,
    pub is_add_game_selected: u32,

    pub has_thumbnail: u32,
    pub title_len: u32,
    pub _pad0: u32,
    pub _pad1: u32,

    pub title_chars_0: [u32; 4],
    pub title_chars_1: [u32; 4],
    pub title_chars_2: [u32; 4],
    pub title_chars_3: [u32; 4],
}

impl Default for HomeScreenUniforms {
    fn default() -> Self {
        Self {
            screen_size: [1280.0, 720.0],
            aspect_ratio: 1280.0 / 720.0,
            scroll_offset: 0.0,
            selected_index: 0,
            total_tiles: 1,
            anim_time: 0.0,
            is_add_game_selected: 1,
            has_thumbnail: 0,
            title_len: 8,
            _pad0: 0,
            _pad1: 0,
            title_chars_0: [65, 68, 68, 32], // "ADD "
            title_chars_1: [71, 65, 77, 69], // "GAME"
            title_chars_2: [0; 4],
            title_chars_3: [0; 4],
        }
    }
}

/// Interactive state for the Home Screen / Library Carousel.
#[derive(Debug, Clone)]
pub struct HomeScreenState {
    pub entries: Vec<RomEntry>,
    pub selected_index: usize,
    pub scroll_offset: f32,
    pub target_offset: f32,
    pub anim_time: f32,
    touch_start: Option<(f32, f32)>,
    is_dragging: bool,
}

impl Default for HomeScreenState {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeScreenState {
    /// Constructs a new `HomeScreenState` with empty library (only "➕ Add Game" card).
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0.0,
            target_offset: 0.0,
            anim_time: 0.0,
            touch_start: None,
            is_dragging: false,
        }
    }

    /// Initializes home screen state loaded from a `LibraryManager`.
    pub fn from_library(library: &LibraryManager) -> Self {
        let mut state = Self::new();
        state.reload_from_library(library);
        state
    }

    /// Reloads ROM entries from `LibraryManager` keeping selection within bounds.
    pub fn reload_from_library(&mut self, library: &LibraryManager) {
        self.entries = library.recent_entries().to_vec();
        if self.selected_index >= self.total_tiles() {
            self.selected_index = self.total_tiles().saturating_sub(1);
        }
    }

    /// Total number of tiles in carousel (game entries + 1 static "➕ Add Game" tile).
    pub fn total_tiles(&self) -> usize {
        self.entries.len() + 1
    }

    /// Whether the currently focused tile is the "➕ Add Game" card.
    pub fn is_add_game_selected(&self) -> bool {
        self.selected_index >= self.entries.len()
    }

    /// Returns the currently selected `RomEntry`, or None if "➕ Add Game" is focused.
    pub fn selected_entry(&self) -> Option<&RomEntry> {
        if self.selected_index < self.entries.len() {
            Some(&self.entries[self.selected_index])
        } else {
            None
        }
    }

    /// Navigates carousel selection one card to the left.
    pub fn navigate_left(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.target_offset = 0.0;
        }
    }

    /// Navigates carousel selection one card to the right.
    pub fn navigate_right(&mut self) {
        if self.selected_index + 1 < self.total_tiles() {
            self.selected_index += 1;
            self.target_offset = 0.0;
        }
    }

    /// Confirms current selection (via A button, Space, Enter, or card tap).
    pub fn confirm_selection(&self) -> Option<HomeScreenAction> {
        if let Some(entry) = self.selected_entry() {
            Some(HomeScreenAction::LaunchRom(entry.clone()))
        } else {
            Some(HomeScreenAction::AddGame)
        }
    }

    /// Advances carousel animation timers and spring interpolation.
    pub fn update(&mut self, dt: f32) {
        self.anim_time += dt;
        let spring = (15.0 * dt).clamp(0.0, 1.0);
        self.scroll_offset += (self.target_offset - self.scroll_offset) * spring;
    }

    /// Processes touch down event for gestures and card tapping.
    pub fn handle_touch_down(&mut self, norm_x: f32, norm_y: f32) {
        self.touch_start = Some((norm_x, norm_y));
        self.is_dragging = false;
    }

    /// Processes touch drag event to scroll the carousel smoothly.
    pub fn handle_touch_move(&mut self, norm_x: f32, _norm_y: f32) {
        if let Some((start_x, _)) = self.touch_start {
            let dx = norm_x - start_x;
            if dx.abs() > 0.015 {
                self.is_dragging = true;
                self.scroll_offset = dx * 0.45;
            }
        }
    }

    /// Processes touch release event: triggers swipes or card selection.
    pub fn handle_touch_up(&mut self, norm_x: f32, norm_y: f32) -> Option<HomeScreenAction> {
        let result = if let Some((start_x, _start_y)) = self.touch_start.take() {
            let dx = norm_x - start_x;
            if self.is_dragging {
                if dx < -0.06 {
                    self.navigate_right();
                } else if dx > 0.06 {
                    self.navigate_left();
                }
                self.target_offset = 0.0;
                None
            } else {
                // Check if user tapped center card area (Y in 0.25..0.70)
                if norm_y >= 0.25 && norm_y <= 0.70 {
                    if norm_x < 0.30 {
                        self.navigate_left();
                        None
                    } else if norm_x > 0.70 {
                        self.navigate_right();
                        None
                    } else {
                        self.confirm_selection()
                    }
                } else if norm_y > 0.70 && norm_y < 0.85 {
                    // Tapped action prompt pill
                    self.confirm_selection()
                } else {
                    None
                }
            }
        } else {
            None
        };

        self.is_dragging = false;
        self.target_offset = 0.0;
        result
    }

    /// Formats the active title string for display and shader uniform packing.
    pub fn current_title(&self) -> String {
        if let Some(entry) = self.selected_entry() {
            entry.display_name.clone()
        } else {
            "ADD NEW ROM".to_string()
        }
    }
}

/// GPU-accelerated Home Screen Carousel Renderer using WGPU.
pub struct HomeScreenRenderer {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    #[allow(dead_code)]
    placeholder_texture: wgpu::Texture,
    placeholder_view: wgpu::TextureView,
    active_thumb_texture: Option<wgpu::Texture>,
    active_thumb_view: Option<wgpu::TextureView>,
    active_thumb_crc32: Option<String>,
    bind_group: Option<wgpu::BindGroup>,
    cached_uniforms: Option<HomeScreenUniforms>,
}

impl HomeScreenRenderer {
    /// Initializes the dedicated WGPU Home Screen render pipeline.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PixelDrive_HomeScreen_WGSL"),
            source: wgpu::ShaderSource::Wgsl(HOME_SCREEN_SHADER_SOURCE.into()),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("HomeScreen_Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 1x1 Dark fallback texture for missing thumbnails
        let placeholder_texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("HomeScreen_Placeholder_Texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[18, 22, 28, 255],
        );
        let placeholder_view = placeholder_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let initial_uniforms = HomeScreenUniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HomeScreen_Uniform_Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("HomeScreen_BindGroupLayout"),
            entries: &[
                // Binding 0: Thumbnail Texture
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
                // Binding 1: Linear Sampler
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
            label: Some("HomeScreen_PipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HomeScreen_RenderPipeline"),
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
            uniform_buffer,
            sampler,
            placeholder_texture,
            placeholder_view,
            active_thumb_texture: None,
            active_thumb_view: None,
            active_thumb_crc32: None,
            bind_group: None,
            cached_uniforms: None,
        }
    }

    /// Loads thumbnail snapshot texture for the given entry from disk into WGPU.
    pub fn update_active_thumbnail(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        entry_opt: Option<&RomEntry>,
    ) {
        let crc = entry_opt.map(|e| e.crc32.clone());
        if crc == self.active_thumb_crc32 && self.active_thumb_texture.is_some() {
            return;
        }

        self.active_thumb_crc32 = crc.clone();
        self.bind_group = None; // Invalidate cached bind group

        let loaded_rgba = entry_opt.and_then(|entry| {
            entry.thumbnail_path.as_ref().and_then(|p| {
                if Path::new(p).exists() {
                    match fs::read(p) {
                        Ok(bytes) => match decode_jpeg(&bytes) {
                            Ok((rgba, w, h)) => Some((rgba, w, h)),
                            Err(e) => {
                                warn!("Failed to decode thumbnail {:?}: {}", p, e);
                                None
                            }
                        },
                        Err(e) => {
                            warn!("Failed to read thumbnail file {:?}: {}", p, e);
                            None
                        }
                    }
                } else {
                    None
                }
            })
        });

        if let Some((rgba, w, h)) = loaded_rgba {
            let tex = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("HomeScreen_Thumbnail_Texture"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &rgba,
            );
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            self.active_thumb_texture = Some(tex);
            self.active_thumb_view = Some(view);
            info!("HomeScreenRenderer: loaded thumbnail texture ({}x{}) for CRC {:?}", w, h, crc);
        } else {
            self.active_thumb_texture = None;
            self.active_thumb_view = None;
        }
    }

    /// Packs and uploads uniforms matching the current state.
    pub fn update_uniforms(
        &mut self,
        queue: &wgpu::Queue,
        state: &HomeScreenState,
        screen_w: u32,
        screen_h: u32,
    ) {
        let sw = screen_w.max(1) as f32;
        let sh = screen_h.max(1) as f32;
        let aspect = sw / sh;

        let title = state.current_title();
        let mut chars = [[0u32; 4]; 4];
        let title_bytes = title.as_bytes();
        let title_len = title_bytes.len().min(16) as u32;

        for i in 0..title_len as usize {
            chars[i / 4][i % 4] = title_bytes[i] as u32;
        }

        let uniforms = HomeScreenUniforms {
            screen_size: [sw, sh],
            aspect_ratio: aspect,
            scroll_offset: state.scroll_offset,
            selected_index: state.selected_index as u32,
            total_tiles: state.total_tiles() as u32,
            anim_time: state.anim_time,
            is_add_game_selected: if state.is_add_game_selected() { 1 } else { 0 },
            has_thumbnail: if self.active_thumb_view.is_some() { 1 } else { 0 },
            title_len,
            _pad0: 0,
            _pad1: 0,
            title_chars_0: chars[0],
            title_chars_1: chars[1],
            title_chars_2: chars[2],
            title_chars_3: chars[3],
        };

        if self.cached_uniforms != Some(uniforms) {
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
            self.cached_uniforms = Some(uniforms);
        }
    }

    /// Renders the Home Screen Carousel directly into the active WGPU render target.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &pixels::PixelsContext,
        state: &HomeScreenState,
        screen_w: u32,
        screen_h: u32,
    ) {
        self.update_active_thumbnail(&context.device, &context.queue, state.selected_entry());
        self.update_uniforms(&context.queue, state, screen_w, screen_h);

        // Lazily build or rebuild bind group
        if self.bind_group.is_none() {
            let tex_view = self
                .active_thumb_view
                .as_ref()
                .unwrap_or(&self.placeholder_view);

            let bg = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("HomeScreen_BindGroup"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(tex_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                ],
            });
            self.bind_group = Some(bg);
        }

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HomeScreen_RenderPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.025,
                        b: 0.035,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.render_pipeline);
        if let Some(ref bg) = self.bind_group {
            rpass.set_bind_group(0, bg, &[]);
        }
        rpass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_screen_navigation() {
        let mut state = HomeScreenState::new();
        // With 0 entries, total_tiles is 1 (the Add Game tile)
        assert_eq!(state.total_tiles(), 1);
        assert!(state.is_add_game_selected());

        state.entries.push(RomEntry::new(
            "path1.gba".to_string(),
            "FireRed".to_string(),
            "11111111".to_string(),
            None,
        ));
        state.entries.push(RomEntry::new(
            "path2.gbc".to_string(),
            "Crystal".to_string(),
            "22222222".to_string(),
            None,
        ));

        // Now total_tiles is 3 (2 games + Add Game)
        assert_eq!(state.total_tiles(), 3);
        assert_eq!(state.selected_index, 0);
        assert!(!state.is_add_game_selected());
        assert_eq!(state.current_title(), "FireRed");

        // Navigate right
        state.navigate_right();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.current_title(), "Crystal");

        state.navigate_right();
        assert_eq!(state.selected_index, 2);
        assert!(state.is_add_game_selected());

        // Clamped at right edge
        state.navigate_right();
        assert_eq!(state.selected_index, 2);

        // Navigate left
        state.navigate_left();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.confirm_selection(), Some(HomeScreenAction::LaunchRom(state.entries[1].clone())));

        state.navigate_right();
        assert_eq!(state.confirm_selection(), Some(HomeScreenAction::AddGame));
    }

    #[test]
    fn test_home_screen_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<HomeScreenUniforms>(), 112);
        assert_eq!(std::mem::size_of::<HomeScreenUniforms>() % 16, 0);
    }

    #[test]
    fn test_home_screen_wgsl_shader_validity() {
        // Full WGSL parser and validator check via naga
        let module = pixels::wgpu::naga::front::wgsl::parse_str(HOME_SCREEN_SHADER_SOURCE);
        assert!(
            module.is_ok(),
            "home_screen.wgsl failed Naga parser validation: {:?}",
            module.err()
        );
    }
}

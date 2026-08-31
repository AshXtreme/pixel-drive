//! Procedural WGPU Signed Distance Field (SDF) Touch Overlay Renderer.
//!
//! Renders anti-aliased virtual touch buttons, D-pad cross, shoulder triggers,
//! system pills, and glowing active-press states with zero texture memory overhead.

use pixels::wgpu::{self, util::DeviceExt};

use crate::input::TouchInputManager;

/// Embedded WGSL shader source for procedural touch overlay.
pub const OVERLAY_SHADER_SOURCE: &str = include_str!("../../shaders/overlay.wgsl");

/// Uniform buffer structure passed to the WGSL procedural touch overlay shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TouchOverlayUniforms {
    pub screen_size: [f32; 2],
    pub aspect_ratio: f32,
    pub opacity: f32,

    pub pressed_mask: u32,
    pub scale: f32,
    pub dpad_radius: f32,
    pub btn_radius: f32,

    pub dpad_center: [f32; 2],
    pub btn_a_pos: [f32; 2],

    pub btn_b_pos: [f32; 2],
    pub btn_l_pos: [f32; 2],

    pub btn_r_pos: [f32; 2],
    pub btn_start_pos: [f32; 2],

    pub btn_select_pos: [f32; 2],
    pub btn_menu_pos: [f32; 2],

    pub btn_ff_pos: [f32; 2],
    pub btn_qs_pos: [f32; 2],

    pub btn_ql_pos: [f32; 2],
    pub menu_state: u32,
    pub menu_pressed_item: u32,

    pub slot_mask: u32,
    pub theme_index: u32,
    pub settings_values: u32,
    pub _pad: u32,
}

impl Default for TouchOverlayUniforms {
    fn default() -> Self {
        Self {
            screen_size: [1280.0, 720.0],
            aspect_ratio: 1280.0 / 720.0,
            opacity: 0.65,
            pressed_mask: 0,
            scale: 1.0,
            dpad_radius: 0.11,
            btn_radius: 0.055,
            dpad_center: [0.14, 0.76],
            btn_a_pos: [0.90, 0.70],
            btn_b_pos: [0.78, 0.80],
            btn_l_pos: [0.11, 0.075],
            btn_r_pos: [0.89, 0.075],
            btn_start_pos: [0.575, 0.925],
            btn_select_pos: [0.425, 0.925],
            btn_menu_pos: [0.44, 0.07],
            btn_ff_pos: [0.56, 0.07],
            btn_qs_pos: [0.34, 0.07],
            btn_ql_pos: [0.66, 0.07],
            menu_state: 0,
            menu_pressed_item: 0,
            slot_mask: 0,
            theme_index: 0,
            settings_values: 0,
            _pad: 0,
        }
    }
}

/// GPU-accelerated Procedural Touch Overlay Renderer managing WGPU pipeline and SDF shaders.
pub struct TouchOverlayRenderer {
    render_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    cached_uniforms: Option<TouchOverlayUniforms>,
}

impl TouchOverlayRenderer {
    /// Initializes a dedicated WGPU render pipeline for the procedural touch overlay.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PixelDrive_TouchOverlay_WGSL"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER_SOURCE.into()),
        });

        let initial_uniforms = TouchOverlayUniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("TouchOverlay_Uniform_Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TouchOverlay_BindGroupLayout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TouchOverlay_BindGroup"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TouchOverlay_PipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TouchOverlay_RenderPipeline"),
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
            bind_group,
            uniform_buffer,
            cached_uniforms: None,
        }
    }

    /// Extracts and updates uniform buffer values from active TouchInputManager state.
    pub fn update_uniforms(
        &mut self,
        queue: &wgpu::Queue,
        touch_manager: &TouchInputManager,
        screen_w: u32,
        screen_h: u32,
    ) {
        let w = screen_w.max(1) as f32;
        let h = screen_h.max(1) as f32;
        let aspect = w / h;

        let uniforms = TouchOverlayUniforms {
            screen_size: [w, h],
            aspect_ratio: aspect,
            opacity: if touch_manager.visible {
                touch_manager.opacity
            } else {
                0.0
            },
            pressed_mask: touch_manager.pressed_bitmask(),
            scale: touch_manager.scale,
            dpad_radius: touch_manager.dpad.radius,
            btn_radius: touch_manager.btn_a.radius(),
            dpad_center: [
                touch_manager.dpad.center.0,
                touch_manager.dpad.center.1,
            ],
            btn_a_pos: [touch_manager.btn_a.center().0, touch_manager.btn_a.center().1],
            btn_b_pos: [touch_manager.btn_b.center().0, touch_manager.btn_b.center().1],
            btn_l_pos: [touch_manager.btn_l.center().0, touch_manager.btn_l.center().1],
            btn_r_pos: [touch_manager.btn_r.center().0, touch_manager.btn_r.center().1],
            btn_start_pos: [
                touch_manager.btn_start.center().0,
                touch_manager.btn_start.center().1,
            ],
            btn_select_pos: [
                touch_manager.btn_select.center().0,
                touch_manager.btn_select.center().1,
            ],
            btn_menu_pos: [
                touch_manager.btn_menu.center().0,
                touch_manager.btn_menu.center().1,
            ],
            btn_ff_pos: [
                touch_manager.btn_fast_forward.center().0,
                touch_manager.btn_fast_forward.center().1,
            ],
            btn_qs_pos: [
                touch_manager.btn_quick_save.center().0,
                touch_manager.btn_quick_save.center().1,
            ],
            btn_ql_pos: [
                touch_manager.btn_quick_load.center().0,
                touch_manager.btn_quick_load.center().1,
            ],
            menu_state: touch_manager.menu_state().shader_index(),
            menu_pressed_item: match touch_manager.menu_state() {
                crate::ui::menu::MenuState::MainMenu => touch_manager
                    .pressed_menu_item()
                    .map(|it| it.shader_index())
                    .unwrap_or(0),
                crate::ui::menu::MenuState::SaveLoadSlotSelect { .. } => touch_manager
                    .pressed_save_load_item()
                    .map(|it| it.shader_index())
                    .unwrap_or(0),
                crate::ui::menu::MenuState::Settings => touch_manager
                    .pressed_settings_item()
                    .map(|it| it.shader_index())
                    .unwrap_or(0),
                crate::ui::menu::MenuState::LayoutEditor => touch_manager
                    .pressed_editor_toolbar_item()
                    .map(|it| it.shader_index())
                    .unwrap_or(0),
                _ => 0,
            },
            slot_mask: touch_manager.slot_mask(),
            theme_index: touch_manager.theme_index as u32,
            settings_values: {
                let op_idx = if touch_manager.opacity < 0.30 { 1u32 }
                    else if touch_manager.opacity < 0.50 { 2u32 }
                    else if touch_manager.opacity < 0.70 { 3u32 }
                    else if touch_manager.opacity < 0.90 { 4u32 }
                    else { 5u32 };
                let sc_idx = if touch_manager.scale < 0.85 { 1u32 }
                    else if touch_manager.scale < 1.10 { 2u32 }
                    else if touch_manager.scale < 1.35 { 3u32 }
                    else { 4u32 };
                let th_idx = (touch_manager.theme_index as u32) & 0x03;
                op_idx | (sc_idx << 4) | (th_idx << 8)
            },
            _pad: 0,
        };

        if self.cached_uniforms != Some(uniforms) {
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
            self.cached_uniforms = Some(uniforms);
        }
    }

    /// Renders procedural touch overlay directly into an active WGPU RenderPass.
    pub fn render_touch_overlay<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        touch_manager: &'a TouchInputManager,
    ) {
        if !touch_manager.visible || touch_manager.opacity <= 0.001 {
            return;
        }

        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    /// Composites the virtual touch overlay cleanly over the target render texture.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &pixels::PixelsContext,
        touch_manager: &TouchInputManager,
        screen_w: u32,
        screen_h: u32,
    ) {
        if !touch_manager.visible || touch_manager.opacity <= 0.001 {
            return;
        }

        self.update_uniforms(&context.queue, touch_manager, screen_w, screen_h);

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("TouchOverlay_RenderPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.render_touch_overlay(&mut rpass, touch_manager);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_overlay_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<TouchOverlayUniforms>(), 144);
        assert_eq!(std::mem::size_of::<TouchOverlayUniforms>() % 16, 0);

        let uniforms = [TouchOverlayUniforms::default()];
        let bytes: &[u8] = bytemuck::cast_slice(&uniforms);
        assert_eq!(bytes.len(), 144);
    }

    #[test]
    fn test_overlay_wgsl_shader_validity() {
        assert!(OVERLAY_SHADER_SOURCE.contains("@vertex"));
        assert!(OVERLAY_SHADER_SOURCE.contains("fn vs_main"));
        assert!(OVERLAY_SHADER_SOURCE.contains("@fragment"));
        assert!(OVERLAY_SHADER_SOURCE.contains("fn fs_main"));
        assert!(OVERLAY_SHADER_SOURCE.contains("sd_rounded_box"));
        assert!(OVERLAY_SHADER_SOURCE.contains("sd_circle"));
        assert!(OVERLAY_SHADER_SOURCE.contains("render_round_button"));
        assert!(OVERLAY_SHADER_SOURCE.contains("render_pill_button"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_letter_a"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_letter_b"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_fast_forward"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_save_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_back_arrow"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_swap_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_digit"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_load_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_pause_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_play_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_folder_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("draw_reset_icon"));
        assert!(OVERLAY_SHADER_SOURCE.contains("render_modal_row"));

        // Full WGSL parser and validator check via naga
        pixels::wgpu::naga::front::wgsl::parse_str(OVERLAY_SHADER_SOURCE)
            .expect("Touch overlay WGSL shader must parse and validate cleanly without errors");
    }
}

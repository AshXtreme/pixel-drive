//! WGSL Post-Processing Shader definitions and filter modes for PixelDrive.

/// Filter modes supported by the post-processing shader pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    /// Crisp nearest-neighbor integer scaling (authentic pixel art)
    #[default]
    Nearest = 0,
    /// Smooth bilinear interpolation filtering
    Bilinear = 1,
    /// Authentic handheld LCD subpixel grid lines & phosphors
    LcdGrid = 2,
    /// Gamma-compensated GBA-to-sRGB color correction matrix
    ColorCorrection = 3,
    /// Combined LCD Screen Grid + Color Correction
    LcdColor = 4,
}

impl FilterMode {
    /// Returns the human-readable display name for the filter mode.
    pub fn name(&self) -> &'static str {
        match self {
            FilterMode::Nearest => "Nearest (Sharp)",
            FilterMode::Bilinear => "Bilinear (Smooth)",
            FilterMode::LcdGrid => "LCD Screen Grid",
            FilterMode::ColorCorrection => "Color Corrected GBA",
            FilterMode::LcdColor => "LCD + Color Corrected",
        }
    }

    /// Cycles to the next available filter mode.
    pub fn next(&self) -> Self {
        match self {
            FilterMode::Nearest => FilterMode::Bilinear,
            FilterMode::Bilinear => FilterMode::LcdGrid,
            FilterMode::LcdGrid => FilterMode::ColorCorrection,
            FilterMode::ColorCorrection => FilterMode::LcdColor,
            FilterMode::LcdColor => FilterMode::Nearest,
        }
    }

    /// Returns the corresponding shader uniform integer value.
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

/// Uniform structure passed to WGSL post-processing shaders.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShaderUniforms {
    pub texture_size: [f32; 2],
    pub output_size: [f32; 2],
    pub filter_type: u32,
    pub intensity: f32,
    pub _pad: [f32; 2],
}

impl Default for ShaderUniforms {
    fn default() -> Self {
        Self {
            texture_size: [160.0, 144.0],
            output_size: [640.0, 576.0],
            filter_type: FilterMode::Nearest.as_u32(),
            intensity: 1.0,
            _pad: [0.0, 0.0],
        }
    }
}

/// WGSL shader supporting crisp Nearest sampling, Bilinear smoothing, LCD subpixel phosphors, and GBA Color Correction.
pub const SHADER_SOURCE: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

struct ShaderUniforms {
    texture_size: vec2<f32>,
    output_size: vec2<f32>,
    filter_type: u32,
    intensity: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse_nearest: sampler;
@group(0) @binding(2) var s_diffuse_linear: sampler;
@group(0) @binding(3) var<uniform> uniforms: ShaderUniforms;

fn apply_lcd_grid(color: vec3<f32>, uv: vec2<f32>, tex_size: vec2<f32>) -> vec3<f32> {
    let pixel_coord = uv * tex_size;
    let subpixel = fract(pixel_coord);
    
    // Subpixel grid line mask (horizontal & vertical borders)
    let border = 0.09;
    let edge_x = smoothstep(0.0, border, subpixel.x) * smoothstep(1.0, 1.0 - border, subpixel.x);
    let edge_y = smoothstep(0.0, border, subpixel.y) * smoothstep(1.0, 1.0 - border, subpixel.y);
    let grid_mask = mix(0.70, 1.0, edge_x * edge_y);
    
    // Subpixel RGB phosphor striping effect
    let sub_x = fract(pixel_coord.x * 3.0);
    var sub_mask = vec3<f32>(1.0, 1.0, 1.0);
    if (sub_x < 0.33) {
        sub_mask = vec3<f32>(1.08, 0.95, 0.95);
    } else if (sub_x < 0.66) {
        sub_mask = vec3<f32>(0.95, 1.08, 0.95);
    } else {
        sub_mask = vec3<f32>(0.95, 0.95, 1.08);
    }

    return color * grid_mask * sub_mask;
}

fn apply_color_correction(color: vec3<f32>) -> vec3<f32> {
    let r = color.r;
    let g = color.g;
    let b = color.b;

    // GBA to Modern sRGB Color Matrix Transform
    let cr = clamp(0.84 * r + 0.17 * g + 0.00 * b, 0.0, 1.0);
    let cg = clamp(0.08 * r + 0.90 * g + 0.08 * b, 0.0, 1.0);
    let cb = clamp(0.00 * r + 0.16 * g + 0.84 * b, 0.0, 1.0);

    // Subtle gamma compensation for modern backlit displays
    let gamma = 1.12;
    let out_r = pow(cr, gamma);
    let out_g = pow(cg, gamma);
    let out_b = pow(cb, gamma);

    return vec3<f32>(out_r, out_g, out_b);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var raw_sample: vec4<f32>;
    if (uniforms.filter_type == 1u) {
        // Bilinear smooth sampling
        raw_sample = textureSample(t_diffuse, s_diffuse_linear, in.uv);
    } else {
        // Nearest-neighbor texture sample
        raw_sample = textureSample(t_diffuse, s_diffuse_nearest, in.uv);
    }
    
    var rgb = raw_sample.rgb;

    if (uniforms.filter_type == 2u) {
        // LCD Screen Grid
        rgb = apply_lcd_grid(rgb, in.uv, uniforms.texture_size);
    } else if (uniforms.filter_type == 3u) {
        // Color Correction
        rgb = apply_color_correction(rgb);
    } else if (uniforms.filter_type == 4u) {
        // LCD Grid + Color Correction
        rgb = apply_color_correction(rgb);
        rgb = apply_lcd_grid(rgb, in.uv, uniforms.texture_size);
    }

    return vec4<f32>(rgb, raw_sample.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_mode_cycling() {
        assert_eq!(FilterMode::Nearest.next(), FilterMode::Bilinear);
        assert_eq!(FilterMode::Bilinear.next(), FilterMode::LcdGrid);
        assert_eq!(FilterMode::LcdGrid.next(), FilterMode::ColorCorrection);
        assert_eq!(FilterMode::ColorCorrection.next(), FilterMode::LcdColor);
        assert_eq!(FilterMode::LcdColor.next(), FilterMode::Nearest);
    }

    #[test]
    fn test_filter_mode_as_u32() {
        assert_eq!(FilterMode::Nearest.as_u32(), 0);
        assert_eq!(FilterMode::Bilinear.as_u32(), 1);
        assert_eq!(FilterMode::LcdGrid.as_u32(), 2);
        assert_eq!(FilterMode::ColorCorrection.as_u32(), 3);
        assert_eq!(FilterMode::LcdColor.as_u32(), 4);
    }

    #[test]
    fn test_filter_mode_names() {
        assert_eq!(FilterMode::Nearest.name(), "Nearest (Sharp)");
        assert_eq!(FilterMode::Bilinear.name(), "Bilinear (Smooth)");
        assert_eq!(FilterMode::LcdGrid.name(), "LCD Screen Grid");
        assert_eq!(FilterMode::ColorCorrection.name(), "Color Corrected GBA");
        assert_eq!(FilterMode::LcdColor.name(), "LCD + Color Corrected");
    }

    #[test]
    fn test_shader_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<ShaderUniforms>(), 32);
        let uniforms = [ShaderUniforms::default()];
        let bytes: &[u8] = bytemuck::cast_slice(&uniforms);
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_wgsl_shader_source_validity() {
        assert!(SHADER_SOURCE.contains("@vertex"));
        assert!(SHADER_SOURCE.contains("fn vs_main"));
        assert!(SHADER_SOURCE.contains("@fragment"));
        assert!(SHADER_SOURCE.contains("fn fs_main"));
        assert!(SHADER_SOURCE.contains("apply_lcd_grid"));
        assert!(SHADER_SOURCE.contains("apply_color_correction"));
        assert!(SHADER_SOURCE.contains("s_diffuse_linear"));
    }
}

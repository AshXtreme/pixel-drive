// ============================================================================
// PixelDrive v1.2 — Procedural WGPU Signed Distance Field (SDF) Touch Overlay
// High-Performance Zero-Texture Mobile HUD & Virtual Gamepad Shader
// ============================================================================

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

struct TouchOverlayUniforms {
    screen_size: vec2<f32>,
    aspect_ratio: f32,
    opacity: f32,

    pressed_mask: u32,
    scale: f32,
    dpad_radius: f32,
    btn_radius: f32,

    dpad_center: vec2<f32>,
    btn_a_pos: vec2<f32>,

    btn_b_pos: vec2<f32>,
    btn_l_pos: vec2<f32>,

    btn_r_pos: vec2<f32>,
    btn_start_pos: vec2<f32>,

    btn_select_pos: vec2<f32>,
    btn_menu_pos: vec2<f32>,

    btn_ff_pos: vec2<f32>,
    btn_qs_pos: vec2<f32>,

    btn_ql_pos: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: TouchOverlayUniforms;

// --- Bitmask constants matching input/touch.rs ---
const BTN_A: u32 = 1u << 0u;
const BTN_B: u32 = 1u << 1u;
const BTN_SELECT: u32 = 1u << 2u;
const BTN_START: u32 = 1u << 3u;
const DPAD_RIGHT: u32 = 1u << 4u;
const DPAD_LEFT: u32 = 1u << 5u;
const DPAD_UP: u32 = 1u << 6u;
const DPAD_DOWN: u32 = 1u << 7u;
const BTN_R: u32 = 1u << 8u;
const BTN_L: u32 = 1u << 9u;
const BTN_MENU: u32 = 1u << 10u;
const BTN_FAST_FORWARD: u32 = 1u << 11u;
const CHORD_AB: u32 = 1u << 12u;
const BTN_QUICK_SAVE: u32 = 1u << 13u;
const BTN_QUICK_LOAD: u32 = 1u << 14u;

// --- Signed Distance Field (SDF) 2D Primitives ---

fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r, r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - r;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_capsule(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    return sd_segment(p, a, b) - r;
}

fn sd_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0;
    let e1 = p2 - p1;
    let e2 = p0 - p2;
    let v0 = p - p0;
    let v1 = p - p1;
    let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d = min(min(vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                    vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                    vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// Alpha blend helper (compositing source over destination)
fn blend_over(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if (out_a <= 0.001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}

// Procedural Glyphs and Letters
fn draw_letter_a(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.45;
    let leg1 = sd_segment(p, vec2<f32>(-s * 0.6, s), vec2<f32>(0.0, -s)) - size * 0.08;
    let leg2 = sd_segment(p, vec2<f32>(s * 0.6, s), vec2<f32>(0.0, -s)) - size * 0.08;
    let bar = sd_segment(p, vec2<f32>(-s * 0.35, s * 0.2), vec2<f32>(s * 0.35, s * 0.2)) - size * 0.07;
    return min(min(leg1, leg2), bar);
}

fn draw_letter_b(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.45;
    let stem = sd_segment(p, vec2<f32>(-s * 0.4, -s), vec2<f32>(-s * 0.4, s)) - size * 0.08;
    let top_loop = abs(sd_circle(p - vec2<f32>(-s * 0.1, -s * 0.45), s * 0.48)) - size * 0.07;
    let bot_loop = abs(sd_circle(p - vec2<f32>(-s * 0.05, s * 0.45), s * 0.52)) - size * 0.07;
    let mask_x = p.x - (-s * 0.4);
    let loops = max(min(top_loop, bot_loop), -mask_x);
    return min(stem, loops);
}

fn draw_letter_l(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.42;
    let stem = sd_segment(p, vec2<f32>(-s * 0.4, -s), vec2<f32>(-s * 0.4, s)) - size * 0.09;
    let foot = sd_segment(p, vec2<f32>(-s * 0.4, s), vec2<f32>(s * 0.45, s)) - size * 0.09;
    return min(stem, foot);
}

fn draw_letter_r(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.42;
    let stem = sd_segment(p, vec2<f32>(-s * 0.4, -s), vec2<f32>(-s * 0.4, s)) - size * 0.08;
    let top_loop = abs(sd_circle(p - vec2<f32>(-s * 0.1, -s * 0.45), s * 0.48)) - size * 0.07;
    let leg = sd_segment(p, vec2<f32>(-s * 0.1, 0.0), vec2<f32>(s * 0.45, s)) - size * 0.08;
    let mask_x = p.x - (-s * 0.4);
    return min(min(stem, leg), max(top_loop, -mask_x));
}

fn draw_arrow(p: vec2<f32>, dir: vec2<f32>, size: f32) -> f32 {
    let forward = normalize(dir);
    let right = vec2<f32>(-forward.y, forward.x);
    let tip = p - forward * size * 0.45;
    let left_pt = p + forward * size * 0.35 - right * size * 0.40;
    let right_pt = p + forward * size * 0.35 + right * size * 0.40;
    return sd_triangle(vec2<f32>(0.0, 0.0), tip, left_pt, right_pt);
}

fn draw_fast_forward(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.35;
    let c1 = draw_arrow(p - vec2<f32>(-s * 0.45, 0.0), vec2<f32>(1.0, 0.0), s * 1.3);
    let c2 = draw_arrow(p - vec2<f32>(s * 0.45, 0.0), vec2<f32>(1.0, 0.0), s * 1.3);
    return min(c1, c2);
}

fn draw_menu_bars(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.35;
    let w = s * 0.70;
    let th = size * 0.055;
    let b1 = sd_segment(p, vec2<f32>(-w, -s * 0.65), vec2<f32>(w, -s * 0.65)) - th;
    let b2 = sd_segment(p, vec2<f32>(-w, 0.0), vec2<f32>(w, 0.0)) - th;
    let b3 = sd_segment(p, vec2<f32>(-w, s * 0.65), vec2<f32>(w, s * 0.65)) - th;
    return min(min(b1, b2), b3);
}

fn draw_save_icon(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.35;
    let stem = sd_segment(p, vec2<f32>(0.0, -s * 0.7), vec2<f32>(0.0, s * 0.2)) - size * 0.07;
    let arrow = draw_arrow(p - vec2<f32>(0.0, s * 0.35), vec2<f32>(0.0, 1.0), s * 1.1);
    return min(stem, arrow);
}

fn draw_load_icon(p: vec2<f32>, size: f32) -> f32 {
    let s = size * 0.35;
    let stem = sd_segment(p, vec2<f32>(0.0, s * 0.7), vec2<f32>(0.0, -s * 0.2)) - size * 0.07;
    let arrow = draw_arrow(p - vec2<f32>(0.0, -s * 0.35), vec2<f32>(0.0, -1.0), s * 1.1);
    return min(stem, arrow);
}

// Render styling for circular action button
fn render_round_button(
    p: vec2<f32>,
    radius: f32,
    pressed: bool,
    base_color: vec3<f32>,
    highlight_color: vec3<f32>,
    aa: f32,
) -> vec4<f32> {
    let d = sd_circle(p, radius);
    let rim_width = radius * 0.08;
    let d_rim = abs(d + rim_width * 0.5) - rim_width * 0.5;

    // Fill mask
    let fill_alpha = smoothstep(aa, -aa, d);
    // Outer rim stroke mask
    let rim_alpha = smoothstep(aa, -aa, d_rim);

    // Glow falloff when pressed or ambient rim glow
    let glow = exp(-max(d, 0.0) * (20.0 / radius));

    var color: vec4<f32>;
    if (pressed) {
        let fill = vec4<f32>(highlight_color, 0.85 * fill_alpha);
        let rim = vec4<f32>(vec3<f32>(1.0, 1.0, 1.0), 0.95 * rim_alpha);
        let glow_col = vec4<f32>(highlight_color, 0.60 * glow);
        color = blend_over(rim, fill);
        color = blend_over(color, glow_col);
    } else {
        let fill = vec4<f32>(base_color, 0.40 * fill_alpha);
        let rim = vec4<f32>(mix(base_color, vec3<f32>(1.0), 0.5), 0.70 * rim_alpha);
        let glow_col = vec4<f32>(base_color, 0.18 * glow);
        color = blend_over(rim, fill);
        color = blend_over(color, glow_col);
    }

    return color;
}

// Render styling for pill/capsule buttons (Shoulder / Start / Select)
fn render_pill_button(
    p: vec2<f32>,
    half_size: vec2<f32>,
    corner_radius: f32,
    pressed: bool,
    base_color: vec3<f32>,
    highlight_color: vec3<f32>,
    aa: f32,
) -> vec4<f32> {
    let d = sd_rounded_box(p, half_size, corner_radius);
    let rim_width = corner_radius * 0.10;
    let d_rim = abs(d + rim_width * 0.5) - rim_width * 0.5;

    let fill_alpha = smoothstep(aa, -aa, d);
    let rim_alpha = smoothstep(aa, -aa, d_rim);
    let glow = exp(-max(d, 0.0) * (18.0 / corner_radius));

    var color: vec4<f32>;
    if (pressed) {
        let fill = vec4<f32>(highlight_color, 0.85 * fill_alpha);
        let rim = vec4<f32>(vec3<f32>(1.0, 1.0, 1.0), 0.95 * rim_alpha);
        let glow_col = vec4<f32>(highlight_color, 0.55 * glow);
        color = blend_over(rim, fill);
        color = blend_over(color, glow_col);
    } else {
        let fill = vec4<f32>(base_color, 0.40 * fill_alpha);
        let rim = vec4<f32>(mix(base_color, vec3<f32>(1.0), 0.5), 0.65 * rim_alpha);
        let glow_col = vec4<f32>(base_color, 0.15 * glow);
        color = blend_over(rim, fill);
        color = blend_over(color, glow_col);
    }

    return color;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = uniforms.aspect_ratio;
    // Aspect-corrected coordinate space [0..aspect, 0..1]
    let p = vec2<f32>(in.uv.x * aspect, in.uv.y);
    let aa = 1.5 / uniforms.screen_size.y; // Anti-aliasing threshold in UV space

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Color palettes
    let col_dark = vec3<f32>(0.06, 0.09, 0.14);
    let col_cyan_glow = vec3<f32>(0.12, 0.78, 0.96);
    let col_amber_glow = vec3<f32>(0.98, 0.58, 0.18);
    let col_crimson_glow = vec3<f32>(0.96, 0.22, 0.38);
    let col_emerald_glow = vec3<f32>(0.18, 0.92, 0.55);
    let col_purple_glow = vec3<f32>(0.72, 0.35, 0.95);

    let pressed_mask = uniforms.pressed_mask;

    // ========================================================================
    // 1. D-Pad Rendering (Procedural 4-Way Cross + Directional Arms)
    // ========================================================================
    let dpad_c = vec2<f32>(uniforms.dpad_center.x * aspect, uniforms.dpad_center.y);
    let dpad_p = p - dpad_c;
    let dpad_r = uniforms.dpad_radius;

    if (length(dpad_p) < dpad_r * 1.6) {
        let arm_w = dpad_r * 0.38;
        let arm_len = dpad_r * 0.95;
        let corner_r = dpad_r * 0.14;

        // D-Pad cross SDF
        let b1 = sd_rounded_box(dpad_p, vec2<f32>(arm_len, arm_w), corner_r);
        let b2 = sd_rounded_box(dpad_p, vec2<f32>(arm_w, arm_len), corner_r);
        let cross_sdf = min(b1, b2);

        let dpad_fill = smoothstep(aa, -aa, cross_sdf);
        let dpad_rim_sdf = abs(cross_sdf + 0.003) - 0.003;
        let dpad_rim = smoothstep(aa, -aa, dpad_rim_sdf);

        // Center circular indent
        let center_well = sd_circle(dpad_p, dpad_r * 0.22);
        let center_well_alpha = smoothstep(aa, -aa, center_well);

        // Directional active states
        let up_pressed = (pressed_mask & DPAD_UP) != 0u;
        let down_pressed = (pressed_mask & DPAD_DOWN) != 0u;
        let left_pressed = (pressed_mask & DPAD_LEFT) != 0u;
        let right_pressed = (pressed_mask & DPAD_RIGHT) != 0u;
        let any_dpad = up_pressed || down_pressed || left_pressed || right_pressed;

        var dpad_col = vec4<f32>(col_dark, 0.50 * dpad_fill);
        let rim_color = mix(vec3<f32>(0.4, 0.6, 0.8), col_cyan_glow, select(0.0, 1.0, any_dpad));
        dpad_col = blend_over(vec4<f32>(rim_color, 0.75 * dpad_rim), dpad_col);
        dpad_col = blend_over(vec4<f32>(vec3<f32>(0.02, 0.03, 0.06), 0.40 * center_well_alpha), dpad_col);

        // Directional highlight overlays on cross arms
        let up_arm = sd_rounded_box(dpad_p - vec2<f32>(0.0, -dpad_r * 0.52), vec2<f32>(arm_w * 0.85, dpad_r * 0.42), corner_r * 0.8);
        let down_arm = sd_rounded_box(dpad_p - vec2<f32>(0.0, dpad_r * 0.52), vec2<f32>(arm_w * 0.85, dpad_r * 0.42), corner_r * 0.8);
        let left_arm = sd_rounded_box(dpad_p - vec2<f32>(-dpad_r * 0.52, 0.0), vec2<f32>(dpad_r * 0.42, arm_w * 0.85), corner_r * 0.8);
        let right_arm = sd_rounded_box(dpad_p - vec2<f32>(dpad_r * 0.52, 0.0), vec2<f32>(dpad_r * 0.42, arm_w * 0.85), corner_r * 0.8);

        if (up_pressed) {
            dpad_col = blend_over(vec4<f32>(col_cyan_glow, 0.80 * smoothstep(aa, -aa, up_arm)), dpad_col);
        }
        if (down_pressed) {
            dpad_col = blend_over(vec4<f32>(col_cyan_glow, 0.80 * smoothstep(aa, -aa, down_arm)), dpad_col);
        }
        if (left_pressed) {
            dpad_col = blend_over(vec4<f32>(col_cyan_glow, 0.80 * smoothstep(aa, -aa, left_arm)), dpad_col);
        }
        if (right_pressed) {
            dpad_col = blend_over(vec4<f32>(col_cyan_glow, 0.80 * smoothstep(aa, -aa, right_arm)), dpad_col);
        }

        // Directional arrows
        let arrow_sz = dpad_r * 0.18;
        let up_arrow = draw_arrow(dpad_p - vec2<f32>(0.0, -dpad_r * 0.65), vec2<f32>(0.0, -1.0), arrow_sz);
        let down_arrow = draw_arrow(dpad_p - vec2<f32>(0.0, dpad_r * 0.65), vec2<f32>(0.0, 1.0), arrow_sz);
        let left_arrow = draw_arrow(dpad_p - vec2<f32>(-dpad_r * 0.65, 0.0), vec2<f32>(-1.0, 0.0), arrow_sz);
        let right_arrow = draw_arrow(dpad_p - vec2<f32>(dpad_r * 0.65, 0.0), vec2<f32>(1.0, 0.0), arrow_sz);

        let glyph_col_up = select(vec3<f32>(0.7, 0.85, 1.0), vec3<f32>(1.0), up_pressed);
        let glyph_col_down = select(vec3<f32>(0.7, 0.85, 1.0), vec3<f32>(1.0), down_pressed);
        let glyph_col_left = select(vec3<f32>(0.7, 0.85, 1.0), vec3<f32>(1.0), left_pressed);
        let glyph_col_right = select(vec3<f32>(0.7, 0.85, 1.0), vec3<f32>(1.0), right_pressed);

        dpad_col = blend_over(vec4<f32>(glyph_col_up, 0.90 * smoothstep(aa, -aa, up_arrow)), dpad_col);
        dpad_col = blend_over(vec4<f32>(glyph_col_down, 0.90 * smoothstep(aa, -aa, down_arrow)), dpad_col);
        dpad_col = blend_over(vec4<f32>(glyph_col_left, 0.90 * smoothstep(aa, -aa, left_arrow)), dpad_col);
        dpad_col = blend_over(vec4<f32>(glyph_col_right, 0.90 * smoothstep(aa, -aa, right_arrow)), dpad_col);

        final_color = blend_over(dpad_col, final_color);
    }

    // ========================================================================
    // 2. A+B Chord Bridge Region
    // ========================================================================
    let a_c = vec2<f32>(uniforms.btn_a_pos.x * aspect, uniforms.btn_a_pos.y);
    let b_c = vec2<f32>(uniforms.btn_b_pos.x * aspect, uniforms.btn_b_pos.y);
    let chord_pressed = (pressed_mask & CHORD_AB) != 0u;
    let chord_d = sd_capsule(p, a_c, b_c, uniforms.btn_radius * 0.70);

    if (chord_d < 0.05) {
        let chord_fill = smoothstep(aa, -aa, chord_d);
        let chord_glow = exp(-max(chord_d, 0.0) * 35.0);
        if (chord_pressed) {
            let col = vec4<f32>(mix(col_amber_glow, col_crimson_glow, 0.5), 0.75 * chord_fill + 0.40 * chord_glow);
            final_color = blend_over(col, final_color);
        } else {
            let col = vec4<f32>(col_dark, 0.22 * chord_fill);
            final_color = blend_over(col, final_color);
        }
    }

    // ========================================================================
    // 3. Action Buttons A & B
    // ========================================================================
    let btn_r = uniforms.btn_radius;
    let a_pressed = (pressed_mask & BTN_A) != 0u;
    let b_pressed = (pressed_mask & BTN_B) != 0u;

    // Button A (Crimson / Coral Accent)
    let a_p = p - a_c;
    if (length(a_p) < btn_r * 1.5) {
        let btn_a_col = render_round_button(a_p, btn_r, a_pressed, col_dark, col_crimson_glow, aa);
        let letter_a_d = draw_letter_a(a_p, btn_r * 0.95);
        let glyph_alpha = smoothstep(aa, -aa, letter_a_d);
        let glyph_col = vec4<f32>(select(vec3<f32>(0.92, 0.95, 1.0), vec3<f32>(1.0), a_pressed), 0.95 * glyph_alpha);
        final_color = blend_over(blend_over(glyph_col, btn_a_col), final_color);
    }

    // Button B (Amber / Orange Accent)
    let b_p = p - b_c;
    if (length(b_p) < btn_r * 1.5) {
        let btn_b_col = render_round_button(b_p, btn_r, b_pressed, col_dark, col_amber_glow, aa);
        let letter_b_d = draw_letter_b(b_p, btn_r * 0.95);
        let glyph_alpha = smoothstep(aa, -aa, letter_b_d);
        let glyph_col = vec4<f32>(select(vec3<f32>(0.92, 0.95, 1.0), vec3<f32>(1.0), b_pressed), 0.95 * glyph_alpha);
        final_color = blend_over(blend_over(glyph_col, btn_b_col), final_color);
    }

    // ========================================================================
    // 4. Shoulder Triggers L & R (Top Pills)
    // ========================================================================
    let l_c = vec2<f32>(uniforms.btn_l_pos.x * aspect, uniforms.btn_l_pos.y);
    let r_c = vec2<f32>(uniforms.btn_r_pos.x * aspect, uniforms.btn_r_pos.y);
    let shoulder_half = vec2<f32>(0.075 * aspect * uniforms.scale, 0.032 * uniforms.scale);
    let shoulder_corner = 0.030 * uniforms.scale;

    let l_pressed = (pressed_mask & BTN_L) != 0u;
    let r_pressed = (pressed_mask & BTN_R) != 0u;

    // Trigger L
    let l_p = p - l_c;
    if (length(l_p) < shoulder_half.x * 1.6) {
        let l_col = render_pill_button(l_p, shoulder_half, shoulder_corner, l_pressed, col_dark, col_purple_glow, aa);
        let letter_l_d = draw_letter_l(l_p, shoulder_corner * 1.6);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, letter_l_d));
        final_color = blend_over(blend_over(glyph_col, l_col), final_color);
    }

    // Trigger R
    let r_p = p - r_c;
    if (length(r_p) < shoulder_half.x * 1.6) {
        let r_col = render_pill_button(r_p, shoulder_half, shoulder_corner, r_pressed, col_dark, col_purple_glow, aa);
        let letter_r_d = draw_letter_r(r_p, shoulder_corner * 1.6);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, letter_r_d));
        final_color = blend_over(blend_over(glyph_col, r_col), final_color);
    }

    // ========================================================================
    // 5. System Controls: Start & Select (Bottom Center Pills)
    // ========================================================================
    let select_c = vec2<f32>(uniforms.btn_select_pos.x * aspect, uniforms.btn_select_pos.y);
    let start_c = vec2<f32>(uniforms.btn_start_pos.x * aspect, uniforms.btn_start_pos.y);
    let sys_half = vec2<f32>(0.040 * aspect * uniforms.scale, 0.020 * uniforms.scale);
    let sys_corner = 0.018 * uniforms.scale;

    let select_pressed = (pressed_mask & BTN_SELECT) != 0u;
    let start_pressed = (pressed_mask & BTN_START) != 0u;

    // Select
    let sel_p = p - select_c;
    if (length(sel_p) < sys_half.x * 1.8) {
        let sel_col = render_pill_button(sel_p, sys_half, sys_corner, select_pressed, col_dark, col_emerald_glow, aa);
        let bar_d = sd_segment(sel_p, vec2<f32>(-sys_half.x * 0.5, 0.0), vec2<f32>(sys_half.x * 0.5, 0.0)) - 0.003;
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.85 * smoothstep(aa, -aa, bar_d));
        final_color = blend_over(blend_over(glyph_col, sel_col), final_color);
    }

    // Start
    let start_p = p - start_c;
    if (length(start_p) < sys_half.x * 1.8) {
        let start_col = render_pill_button(start_p, sys_half, sys_corner, start_pressed, col_dark, col_emerald_glow, aa);
        let bar_d = sd_segment(start_p, vec2<f32>(-sys_half.x * 0.5, 0.0), vec2<f32>(sys_half.x * 0.5, 0.0)) - 0.003;
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.85 * smoothstep(aa, -aa, bar_d));
        final_color = blend_over(blend_over(glyph_col, start_col), final_color);
    }

    // ========================================================================
    // 6. HUD Quick Actions: Quick Save, Menu, Fast-Forward, Quick Load (Top Center)
    // ========================================================================
    let qs_c = vec2<f32>(uniforms.btn_qs_pos.x * aspect, uniforms.btn_qs_pos.y);
    let menu_c = vec2<f32>(uniforms.btn_menu_pos.x * aspect, uniforms.btn_menu_pos.y);
    let ff_c = vec2<f32>(uniforms.btn_ff_pos.x * aspect, uniforms.btn_ff_pos.y);
    let ql_c = vec2<f32>(uniforms.btn_ql_pos.x * aspect, uniforms.btn_ql_pos.y);
    let hud_r = btn_r * 0.55;

    let qs_pressed = (pressed_mask & BTN_QUICK_SAVE) != 0u;
    let menu_pressed = (pressed_mask & BTN_MENU) != 0u;
    let ff_pressed = (pressed_mask & BTN_FAST_FORWARD) != 0u;
    let ql_pressed = (pressed_mask & BTN_QUICK_LOAD) != 0u;

    // Quick Save Button (Save Icon / Emerald Glow)
    let qs_p = p - qs_c;
    if (length(qs_p) < hud_r * 1.5) {
        let qs_col = render_round_button(qs_p, hud_r, qs_pressed, col_dark, col_emerald_glow, aa);
        let save_d = draw_save_icon(qs_p, hud_r);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, save_d));
        final_color = blend_over(blend_over(glyph_col, qs_col), final_color);
    }

    // Menu Button (3 Bars Icon)
    let menu_p = p - menu_c;
    if (length(menu_p) < hud_r * 1.5) {
        let menu_col = render_round_button(menu_p, hud_r, menu_pressed, col_dark, col_cyan_glow, aa);
        let bars_d = draw_menu_bars(menu_p, hud_r);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, bars_d));
        final_color = blend_over(blend_over(glyph_col, menu_col), final_color);
    }

    // Fast-Forward Button (Double Chevron Icon)
    let ff_p = p - ff_c;
    if (length(ff_p) < hud_r * 1.5) {
        let ff_col = render_round_button(ff_p, hud_r, ff_pressed, col_dark, col_amber_glow, aa);
        let ff_d = draw_fast_forward(ff_p, hud_r);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, ff_d));
        final_color = blend_over(blend_over(glyph_col, ff_col), final_color);
    }

    // Quick Load Button (Load Icon / Purple Glow)
    let ql_p = p - ql_c;
    if (length(ql_p) < hud_r * 1.5) {
        let ql_col = render_round_button(ql_p, hud_r, ql_pressed, col_dark, col_purple_glow, aa);
        let load_d = draw_load_icon(ql_p, hud_r);
        let glyph_col = vec4<f32>(vec3<f32>(1.0), 0.90 * smoothstep(aa, -aa, load_d));
        final_color = blend_over(blend_over(glyph_col, ql_col), final_color);
    }

    // Apply global opacity uniform
    return vec4<f32>(final_color.rgb, final_color.a * uniforms.opacity);
}

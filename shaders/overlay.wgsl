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
    menu_state: u32,
    menu_pressed_item: u32,
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

// ============================================================================
// In-Game Modal Pause Menu Procedural SDF Glyphs & Renderer
// ============================================================================

fn draw_pause_icon(p: vec2<f32>, size: f32) -> f32 {
    let bar_half = vec2<f32>(size * 0.15, size * 0.45);
    let offset_x = size * 0.30;
    let r = size * 0.05;
    let b1 = sd_rounded_box(p - vec2<f32>(-offset_x, 0.0), bar_half, r);
    let b2 = sd_rounded_box(p - vec2<f32>(offset_x, 0.0), bar_half, r);
    return min(b1, b2);
}

fn draw_play_icon(p: vec2<f32>, size: f32) -> f32 {
    let p0 = vec2<f32>(-size * 0.35, -size * 0.45);
    let p1 = vec2<f32>(-size * 0.35, size * 0.45);
    let p2 = vec2<f32>(size * 0.45, 0.0);
    return sd_triangle(p, p0, p1, p2);
}

fn draw_folder_icon(p: vec2<f32>, size: f32) -> f32 {
    let body = sd_rounded_box(p - vec2<f32>(0.0, size * 0.10), vec2<f32>(size * 0.48, size * 0.30), size * 0.06);
    let tab = sd_rounded_box(p - vec2<f32>(-size * 0.20, -size * 0.26), vec2<f32>(size * 0.24, size * 0.10), size * 0.04);
    return min(body, tab);
}

fn draw_reset_icon(p: vec2<f32>, size: f32) -> f32 {
    let r = size * 0.38;
    let ring = abs(length(p) - r) - (size * 0.08);
    let cut = max(-p.x, p.y);
    let arc = max(ring, -cut);
    let head = sd_triangle(p - vec2<f32>(0.0, r), vec2<f32>(-size * 0.22, -size * 0.15), vec2<f32>(0.0, size * 0.18), vec2<f32>(size * 0.22, -size * 0.15));
    return min(arc, head);
}

fn draw_gear_icon(p: vec2<f32>, size: f32) -> f32 {
    let r = size * 0.30;
    let hub = abs(length(p) - r) - (size * 0.06);
    let t1 = sd_rounded_box(p, vec2<f32>(size * 0.46, size * 0.09), size * 0.03);
    let t2 = sd_rounded_box(p, vec2<f32>(size * 0.09, size * 0.46), size * 0.03);
    let p_diag = vec2<f32>(p.x * 0.7071 - p.y * 0.7071, p.x * 0.7071 + p.y * 0.7071);
    let t3 = sd_rounded_box(p_diag, vec2<f32>(size * 0.46, size * 0.09), size * 0.03);
    let t4 = sd_rounded_box(p_diag, vec2<f32>(size * 0.09, size * 0.46), size * 0.03);
    return min(hub, min(min(t1, t2), min(t3, t4)));
}

fn draw_cheats_icon(p: vec2<f32>, size: f32) -> f32 {
    let body = sd_rounded_box(p, vec2<f32>(size * 0.48, size * 0.28), size * 0.08);
    let dpad_m = min(sd_rounded_box(p - vec2<f32>(-size * 0.24, 0.0), vec2<f32>(size * 0.11, size * 0.035), 0.002),
                     sd_rounded_box(p - vec2<f32>(-size * 0.24, 0.0), vec2<f32>(size * 0.035, size * 0.11), 0.002));
    let btn_b = sd_circle(p - vec2<f32>(size * 0.18, size * 0.05), size * 0.045);
    let btn_a = sd_circle(p - vec2<f32>(size * 0.30, -size * 0.05), size * 0.045);
    return min(body, min(dpad_m, min(btn_a, btn_b)));
}

fn render_modal_row(
    p: vec2<f32>,
    half_size: vec2<f32>,
    corner_r: f32,
    pressed: bool,
    base_color: vec3<f32>,
    accent_color: vec3<f32>,
    aa: f32,
) -> vec4<f32> {
    let d = sd_rounded_box(p, half_size, corner_r);
    let rim_width = 0.0025;
    let d_rim = abs(d + rim_width * 0.5) - rim_width * 0.5;

    let fill_alpha = smoothstep(aa, -aa, d);
    let rim_alpha = smoothstep(aa, -aa, d_rim);
    let glow = exp(-max(d, 0.0) * (24.0 / corner_r));

    var color: vec4<f32>;
    if (pressed) {
        let fill = vec4<f32>(mix(base_color, accent_color, 0.65), 0.95 * fill_alpha);
        let rim = vec4<f32>(vec3<f32>(1.0, 1.0, 1.0), 0.98 * rim_alpha);
        let glow_col = vec4<f32>(accent_color, 0.70 * glow);
        color = blend_over(rim, fill);
        color = blend_over(color, glow_col);
    } else {
        let fill = vec4<f32>(base_color, 0.88 * fill_alpha);
        let rim = vec4<f32>(accent_color, 0.75 * rim_alpha);
        let glow_col = vec4<f32>(accent_color, 0.20 * glow);
        let left_bar = sd_rounded_box(p - vec2<f32>(-half_size.x + 0.008, 0.0), vec2<f32>(0.004, half_size.y * 0.70), 0.002);
        let bar_col = vec4<f32>(accent_color, 0.90 * smoothstep(aa, -aa, left_bar));
        color = blend_over(rim, fill);
        color = blend_over(bar_col, color);
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

    if (uniforms.menu_state == 1u) {
        // ====================================================================
        // In-Game Modal Pause Menu Rendering
        // ====================================================================

        // 1. Fullscreen Dark Dimming Backdrop
        let backdrop = vec4<f32>(0.02, 0.03, 0.06, 0.82);
        final_color = backdrop;

        // 2. Centered Glassmorphic Modal Card
        let modal_c = vec2<f32>(0.50 * aspect, 0.50);
        let modal_half = vec2<f32>(0.28 * aspect, 0.40);
        let modal_r = 0.024;
        let card_p = p - modal_c;
        let card_d = sd_rounded_box(card_p, modal_half, modal_r);

        let card_fill_alpha = smoothstep(aa, -aa, card_d);
        let card_rim_d = abs(card_d + 0.003) - 0.003;
        let card_rim_alpha = smoothstep(aa, -aa, card_rim_d);
        let card_glow = exp(-max(card_d, 0.0) * 15.0);

        let card_bg = vec4<f32>(0.07, 0.09, 0.14, 0.96 * card_fill_alpha);
        let card_rim = vec4<f32>(col_cyan_glow, 0.85 * card_rim_alpha);
        let card_glow_col = vec4<f32>(col_cyan_glow, 0.25 * card_glow);

        final_color = blend_over(card_rim, blend_over(card_bg, final_color));
        final_color = blend_over(card_glow_col, final_color);

        // 3. Header: Pause Icon and Title Bar
        let hdr_c = vec2<f32>(0.50 * aspect, 0.155);
        let hdr_p = p - hdr_c;
        if (length(hdr_p) < 0.06) {
            let pause_d = draw_pause_icon(hdr_p, 0.022);
            let pause_col = vec4<f32>(vec3<f32>(1.0, 1.0, 1.0), 0.95 * smoothstep(aa, -aa, pause_d));
            final_color = blend_over(pause_col, final_color);
        }

        // Header Divider Line
        let div_c = vec2<f32>(0.50 * aspect, 0.188);
        let div_p = p - div_c;
        let div_d = sd_segment(div_p, vec2<f32>(-modal_half.x * 0.82, 0.0), vec2<f32>(modal_half.x * 0.82, 0.0)) - 0.0012;
        let div_col = vec4<f32>(col_cyan_glow, 0.50 * smoothstep(aa, -aa, div_d));
        final_color = blend_over(div_col, final_color);

        // 4. Interactive Menu Option Rows
        let row_half = vec2<f32>(0.24 * aspect, 0.041);
        let row_r = 0.016;
        let pressed_item = uniforms.menu_pressed_item;

        // Row 1: Resume Game (Item 1 / Emerald)
        let r1_c = vec2<f32>(0.50 * aspect, 0.246);
        let r1_p = p - r1_c;
        if (abs(r1_p.y) < row_half.y * 1.3 && abs(r1_p.x) < row_half.x * 1.1) {
            let r1_col = render_modal_row(r1_p, row_half, row_r, pressed_item == 1u, col_dark, col_emerald_glow, aa);
            let icon_d = draw_play_icon(r1_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r1_col), final_color);
        }

        // Row 2: Load New ROM (Item 2 / Cyan)
        let r2_c = vec2<f32>(0.50 * aspect, 0.342);
        let r2_p = p - r2_c;
        if (abs(r2_p.y) < row_half.y * 1.3 && abs(r2_p.x) < row_half.x * 1.1) {
            let r2_col = render_modal_row(r2_p, row_half, row_r, pressed_item == 2u, col_dark, col_cyan_glow, aa);
            let icon_d = draw_folder_icon(r2_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r2_col), final_color);
        }

        // Row 3: Save / Load States (Item 3 / Purple)
        let r3_c = vec2<f32>(0.50 * aspect, 0.438);
        let r3_p = p - r3_c;
        if (abs(r3_p.y) < row_half.y * 1.3 && abs(r3_p.x) < row_half.x * 1.1) {
            let r3_col = render_modal_row(r3_p, row_half, row_r, pressed_item == 3u, col_dark, col_purple_glow, aa);
            let icon_d = draw_save_icon(r3_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.020);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r3_col), final_color);
        }

        // Row 4: Reset Game (Item 4 / Amber)
        let r4_c = vec2<f32>(0.50 * aspect, 0.534);
        let r4_p = p - r4_c;
        if (abs(r4_p.y) < row_half.y * 1.3 && abs(r4_p.x) < row_half.x * 1.1) {
            let r4_col = render_modal_row(r4_p, row_half, row_r, pressed_item == 4u, col_dark, col_amber_glow, aa);
            let icon_d = draw_reset_icon(r4_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r4_col), final_color);
        }

        // Row 5: Settings (Item 5 / Steel Blue)
        let r5_c = vec2<f32>(0.50 * aspect, 0.630);
        let r5_p = p - r5_c;
        if (abs(r5_p.y) < row_half.y * 1.3 && abs(r5_p.x) < row_half.x * 1.1) {
            let col_steel = vec3<f32>(0.45, 0.65, 0.95);
            let r5_col = render_modal_row(r5_p, row_half, row_r, pressed_item == 5u, col_dark, col_steel, aa);
            let icon_d = draw_gear_icon(r5_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r5_col), final_color);
        }

        // Row 6: Cheat Codes (Item 6 / Magenta)
        let r6_c = vec2<f32>(0.50 * aspect, 0.726);
        let r6_p = p - r6_c;
        if (abs(r6_p.y) < row_half.y * 1.3 && abs(r6_p.x) < row_half.x * 1.1) {
            let col_magenta = vec3<f32>(0.95, 0.35, 0.70);
            let r6_col = render_modal_row(r6_p, row_half, row_r, pressed_item == 6u, col_dark, col_magenta, aa);
            let icon_d = draw_cheats_icon(r6_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, icon_d));
            final_color = blend_over(blend_over(icon_col, r6_col), final_color);
        }

        return vec4<f32>(final_color.rgb, final_color.a * max(uniforms.opacity, 0.85));
    }

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

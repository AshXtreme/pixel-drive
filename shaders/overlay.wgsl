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

    slot_mask: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
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

// Procedural Left Arrow Icon (← Back)
fn draw_back_arrow(p: vec2<f32>, sz: f32) -> f32 {
    let stem = sd_segment(p, vec2<f32>(-sz * 0.45, 0.0), vec2<f32>(sz * 0.45, 0.0)) - 0.0022;
    let upper = sd_segment(p, vec2<f32>(-sz * 0.45, 0.0), vec2<f32>(-sz * 0.15, sz * 0.35)) - 0.0022;
    let lower = sd_segment(p, vec2<f32>(-sz * 0.45, 0.0), vec2<f32>(-sz * 0.15, -sz * 0.35)) - 0.0022;
    return min(stem, min(upper, lower));
}

// Procedural Swap Mode Icon (⇄ Toggle)
fn draw_swap_icon(p: vec2<f32>, sz: f32) -> f32 {
    let s1 = sd_segment(p, vec2<f32>(-sz * 0.40, sz * 0.22), vec2<f32>(sz * 0.40, sz * 0.22)) - 0.002;
    let a1 = sd_segment(p, vec2<f32>(sz * 0.40, sz * 0.22), vec2<f32>(sz * 0.15, sz * 0.42)) - 0.002;
    let s2 = sd_segment(p, vec2<f32>(-sz * 0.40, -sz * 0.22), vec2<f32>(sz * 0.40, -sz * 0.22)) - 0.002;
    let a2 = sd_segment(p, vec2<f32>(-sz * 0.40, -sz * 0.22), vec2<f32>(-sz * 0.15, -sz * 0.42)) - 0.002;
    return min(min(s1, a1), min(s2, a2));
}

// Procedural Digits 1..=5 for Save State Slot Badges
fn draw_digit(p: vec2<f32>, digit: u32, sz: f32) -> f32 {
    let hw = sz * 0.30;
    let hh = sz * 0.45;
    let th = 0.0022;
    if (digit == 1u) {
        return sd_segment(p, vec2<f32>(0.0, -hh), vec2<f32>(0.0, hh)) - th;
    } else if (digit == 2u) {
        let t = sd_segment(p, vec2<f32>(-hw, -hh), vec2<f32>(hw, -hh)) - th;
        let r = sd_segment(p, vec2<f32>(hw, -hh), vec2<f32>(hw, 0.0)) - th;
        let m = sd_segment(p, vec2<f32>(-hw, 0.0), vec2<f32>(hw, 0.0)) - th;
        let l = sd_segment(p, vec2<f32>(-hw, 0.0), vec2<f32>(-hw, hh)) - th;
        let b = sd_segment(p, vec2<f32>(-hw, hh), vec2<f32>(hw, hh)) - th;
        return min(min(t, r), min(min(m, l), b));
    } else if (digit == 3u) {
        let t = sd_segment(p, vec2<f32>(-hw, -hh), vec2<f32>(hw, -hh)) - th;
        let m = sd_segment(p, vec2<f32>(-hw, 0.0), vec2<f32>(hw, 0.0)) - th;
        let b = sd_segment(p, vec2<f32>(-hw, hh), vec2<f32>(hw, hh)) - th;
        let r = sd_segment(p, vec2<f32>(hw, -hh), vec2<f32>(hw, hh)) - th;
        return min(min(t, m), min(b, r));
    } else if (digit == 4u) {
        let l = sd_segment(p, vec2<f32>(-hw, -hh), vec2<f32>(-hw, 0.0)) - th;
        let m = sd_segment(p, vec2<f32>(-hw, 0.0), vec2<f32>(hw, 0.0)) - th;
        let r = sd_segment(p, vec2<f32>(hw, -hh), vec2<f32>(hw, hh)) - th;
        return min(min(l, m), r);
    } else {
        let t = sd_segment(p, vec2<f32>(-hw, -hh), vec2<f32>(hw, -hh)) - th;
        let l = sd_segment(p, vec2<f32>(-hw, -hh), vec2<f32>(-hw, 0.0)) - th;
        let m = sd_segment(p, vec2<f32>(-hw, 0.0), vec2<f32>(hw, 0.0)) - th;
        let r = sd_segment(p, vec2<f32>(hw, 0.0), vec2<f32>(hw, hh)) - th;
        let b = sd_segment(p, vec2<f32>(-hw, hh), vec2<f32>(hw, hh)) - th;
        return min(min(t, l), min(min(m, r), b));
    }
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
    let rim_width = 0.0018;
    let d_rim = abs(d + rim_width * 0.5) - rim_width * 0.5;

    let fill_alpha = smoothstep(aa, -aa, d);
    let rim_alpha = smoothstep(aa, -aa, d_rim);

    var color: vec4<f32>;
    if (pressed) {
        let fill = vec4<f32>(vec3<f32>(0.25, 0.28, 0.34), 1.0 * fill_alpha);
        let rim = vec4<f32>(vec3<f32>(0.85, 0.88, 0.94), 0.95 * rim_alpha);
        color = blend_over(rim, fill);
    } else {
        let fill = vec4<f32>(base_color, 1.0 * fill_alpha);
        let rim = vec4<f32>(accent_color, 0.70 * rim_alpha);
        let left_bar = sd_rounded_box(p - vec2<f32>(-half_size.x + 0.006, 0.0), vec2<f32>(0.0025, half_size.y * 0.65), 0.0012);
        let bar_col = vec4<f32>(accent_color, 0.95 * smoothstep(aa, -aa, left_bar));
        color = blend_over(rim, fill);
        color = blend_over(bar_col, color);
    }
    return color;
}

// ============================================================================
// Modern Vector SDF Typography Engine for High-Legibility Button Labels
// ============================================================================

fn get_glyph_16seg(c: u32) -> u32 {
    if (c == 65u) { return 975u;   } // A
    if (c == 66u) { return 3647u;  } // B
    if (c == 67u) { return 243u;   } // C
    if (c == 68u) { return 3135u;  } // D
    if (c == 69u) { return 1011u;  } // E
    if (c == 70u) { return 963u;   } // F
    if (c == 71u) { return 763u;   } // G
    if (c == 72u) { return 972u;   } // H
    if (c == 73u) { return 3123u;  } // I
    if (c == 75u) { return 41408u; } // K
    if (c == 76u) { return 240u;   } // L
    if (c == 77u) { return 12492u; } // M
    if (c == 78u) { return 37068u; } // N
    if (c == 79u) { return 255u;   } // O
    if (c == 80u) { return 967u;   } // P
    if (c == 82u) { return 33735u; } // R
    if (c == 83u) { return 955u;   } // S
    if (c == 84u) { return 3075u;  } // T
    if (c == 85u) { return 252u;   } // U
    if (c == 86u) { return 24768u; } // V
    if (c == 87u) { return 49356u; } // W
    if (c == 89u) { return 14336u; } // Y
    if (c == 48u) { return 255u;   } // 0
    if (c == 49u) { return 3072u;  } // 1
    if (c == 50u) { return 887u;   } // 2
    if (c == 51u) { return 831u;   } // 3
    if (c == 52u) { return 908u;   } // 4
    if (c == 53u) { return 955u;   } // 5
    if (c == 47u) { return 24576u; } // /
    if (c == 45u) { return 768u;   } // -
    return 0u;
}

fn draw_vector_char(p: vec2<f32>, c: u32, sz: vec2<f32>, stroke_w: f32) -> f32 {
    let mask = get_glyph_16seg(c);
    if (mask == 0u) {
        return 1e5;
    }
    let w = sz.x * 0.44;
    let h = sz.y * 0.46;

    let tl = vec2<f32>(-w, -h);
    let tc = vec2<f32>(0.0, -h);
    let tr = vec2<f32>(w, -h);
    let ml = vec2<f32>(-w, 0.0);
    let mc = vec2<f32>(0.0, 0.0);
    let mr = vec2<f32>(w, 0.0);
    let bl = vec2<f32>(-w, h);
    let bc = vec2<f32>(0.0, h);
    let br = vec2<f32>(w, h);

    var d = 1e5;
    if ((mask & 1u) != 0u)     { d = min(d, sd_segment(p, tl, tc)); }
    if ((mask & 2u) != 0u)     { d = min(d, sd_segment(p, tc, tr)); }
    if ((mask & 4u) != 0u)     { d = min(d, sd_segment(p, tr, mr)); }
    if ((mask & 8u) != 0u)     { d = min(d, sd_segment(p, mr, br)); }
    if ((mask & 16u) != 0u)    { d = min(d, sd_segment(p, br, bc)); }
    if ((mask & 32u) != 0u)    { d = min(d, sd_segment(p, bc, bl)); }
    if ((mask & 64u) != 0u)    { d = min(d, sd_segment(p, bl, ml)); }
    if ((mask & 128u) != 0u)   { d = min(d, sd_segment(p, ml, tl)); }
    if ((mask & 256u) != 0u)   { d = min(d, sd_segment(p, ml, mc)); }
    if ((mask & 512u) != 0u)   { d = min(d, sd_segment(p, mc, mr)); }
    if ((mask & 1024u) != 0u)  { d = min(d, sd_segment(p, tc, mc)); }
    if ((mask & 2048u) != 0u)  { d = min(d, sd_segment(p, mc, bc)); }
    if ((mask & 4096u) != 0u)  { d = min(d, sd_segment(p, tl, mc)); }
    if ((mask & 8192u) != 0u)  { d = min(d, sd_segment(p, tr, mc)); }
    if ((mask & 16384u) != 0u) { d = min(d, sd_segment(p, mc, bl)); }
    if ((mask & 32768u) != 0u) { d = min(d, sd_segment(p, mc, br)); }

    return d - stroke_w;
}

fn draw_vector_string(
    p: vec2<f32>,
    start_x: f32,
    sz: vec2<f32>,
    spacing: f32,
    stroke_w: f32,
    c0: u32, c1: u32, c2: u32, c3: u32, c4: u32, c5: u32,
    c6: u32, c7: u32, c8: u32, c9: u32, c10: u32, len: u32
) -> f32 {
    let rel_x = p.x - start_x + spacing * 0.5;
    if (rel_x < 0.0) {
        return 1e5;
    }
    let idx = u32(floor(rel_x / spacing));
    if (idx >= len) {
        return 1e5;
    }
    let char_center_x = start_x + f32(idx) * spacing;
    let local_p = vec2<f32>(p.x - char_center_x, p.y);

    var code = 0u;
    if (idx == 0u) { code = c0; }
    else if (idx == 1u) { code = c1; }
    else if (idx == 2u) { code = c2; }
    else if (idx == 3u) { code = c3; }
    else if (idx == 4u) { code = c4; }
    else if (idx == 5u) { code = c5; }
    else if (idx == 6u) { code = c6; }
    else if (idx == 7u) { code = c7; }
    else if (idx == 8u) { code = c8; }
    else if (idx == 9u) { code = c9; }
    else if (idx == 10u) { code = c10; }

    return draw_vector_char(local_p, code, sz, stroke_w);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = uniforms.aspect_ratio;
    // Aspect-corrected coordinate space [0..aspect, 0..1]
    let p = vec2<f32>(in.uv.x * aspect, in.uv.y);
    let aa = 1.5 / uniforms.screen_size.y; // Anti-aliasing threshold in UV space

    var final_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    // Color palettes for virtual in-game controls
    let col_dark = vec3<f32>(0.06, 0.09, 0.14);
    let col_cyan_glow = vec3<f32>(0.12, 0.78, 0.96);
    let col_amber_glow = vec3<f32>(0.98, 0.58, 0.18);
    let col_crimson_glow = vec3<f32>(0.96, 0.22, 0.38);
    let col_emerald_glow = vec3<f32>(0.18, 0.92, 0.55);
    let col_purple_glow = vec3<f32>(0.72, 0.35, 0.95);

    // Clean neutral dark color palette (No harsh neon glows)
    let col_btn_bg = vec3<f32>(0.16, 0.18, 0.22);
    let col_card_bg = vec4<f32>(0.11, 0.12, 0.14, 1.0);
    let col_card_rim = vec4<f32>(0.28, 0.31, 0.36, 0.95);
    let col_glyph = vec4<f32>(vec3<f32>(0.92, 0.94, 0.97), 0.95);

    // Muted semantic accents
    let col_accent_resume = vec3<f32>(0.35, 0.65, 0.48);
    let col_accent_rom = vec3<f32>(0.42, 0.55, 0.72);
    let col_accent_state = vec3<f32>(0.55, 0.48, 0.72);
    let col_accent_reset = vec3<f32>(0.72, 0.45, 0.42);
    let col_accent_settings = vec3<f32>(0.45, 0.50, 0.58);
    let col_accent_cheats = vec3<f32>(0.58, 0.48, 0.55);

    if (uniforms.menu_state == 1u) {
        // ====================================================================
        // In-Game Main Pause Menu Rendering (Opaque Dark UI)
        // ====================================================================

        // 1. Fullscreen Dark Dimming Backdrop
        let backdrop = vec4<f32>(0.03, 0.04, 0.05, 0.95);
        final_color = backdrop;

        // 2. Centered Solid Opaque Modal Card
        let modal_c = vec2<f32>(0.50 * aspect, 0.50);
        let modal_half = vec2<f32>(0.28 * aspect, 0.40);
        let modal_r = 0.024;
        let card_p = p - modal_c;
        let card_d = sd_rounded_box(card_p, modal_half, modal_r);

        let card_fill_alpha = smoothstep(aa, -aa, card_d);
        let card_rim_d = abs(card_d + 0.002) - 0.002;
        let card_rim_alpha = smoothstep(aa, -aa, card_rim_d);

        let card_bg = vec4<f32>(col_card_bg.rgb, col_card_bg.a * card_fill_alpha);
        let card_rim = vec4<f32>(col_card_rim.rgb, col_card_rim.a * card_rim_alpha);

        final_color = blend_over(card_rim, blend_over(card_bg, final_color));

        // 3. Header: Pause Icon and Title Bar
        let hdr_c = vec2<f32>(0.50 * aspect, 0.155);
        let hdr_p = p - hdr_c;
        if (length(hdr_p) < 0.06) {
            let pause_d = draw_pause_icon(hdr_p, 0.022);
            let pause_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, pause_d));
            final_color = blend_over(pause_col, final_color);
        }

        // Header Divider Line
        let div_c = vec2<f32>(0.50 * aspect, 0.188);
        let div_p = p - div_c;
        let div_d = sd_segment(div_p, vec2<f32>(-modal_half.x * 0.82, 0.0), vec2<f32>(modal_half.x * 0.82, 0.0)) - 0.0010;
        let div_col = vec4<f32>(0.24, 0.27, 0.32, 0.90 * smoothstep(aa, -aa, div_d));
        final_color = blend_over(div_col, final_color);

        // 4. Interactive Menu Option Rows
        let row_half = vec2<f32>(0.24 * aspect, 0.041);
        let row_r = 0.016;
        let pressed_item = uniforms.menu_pressed_item;
        let text_start_x = -row_half.x + 0.078;
        let char_sz = vec2<f32>(0.013, 0.022);
        let spacing = 0.0175;
        let stroke_w = 0.0016;

        // Row 1: Resume Game (Item 1 / Muted Mint)
        let r1_c = vec2<f32>(0.50 * aspect, 0.246);
        let r1_p = p - r1_c;
        if (abs(r1_p.y) < row_half.y * 1.3 && abs(r1_p.x) < row_half.x * 1.1) {
            let r1_col = render_modal_row(r1_p, row_half, row_r, pressed_item == 1u, col_btn_bg, col_accent_resume, aa);
            let icon_d = draw_play_icon(r1_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r1_p, text_start_x, char_sz, spacing, stroke_w, 82u, 69u, 83u, 85u, 77u, 69u, 32u, 71u, 65u, 77u, 69u, 11u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r1_col)), final_color);
        }

        // Row 2: Load New ROM (Item 2 / Muted Slate Blue)
        let r2_c = vec2<f32>(0.50 * aspect, 0.342);
        let r2_p = p - r2_c;
        if (abs(r2_p.y) < row_half.y * 1.3 && abs(r2_p.x) < row_half.x * 1.1) {
            let r2_col = render_modal_row(r2_p, row_half, row_r, pressed_item == 2u, col_btn_bg, col_accent_rom, aa);
            let icon_d = draw_folder_icon(r2_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r2_p, text_start_x, char_sz, spacing, stroke_w, 76u, 79u, 65u, 68u, 32u, 82u, 79u, 77u, 0u, 0u, 0u, 8u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r2_col)), final_color);
        }

        // Row 3: Save / Load States (Item 3 / Muted Slate Lavender)
        let r3_c = vec2<f32>(0.50 * aspect, 0.438);
        let r3_p = p - r3_c;
        if (abs(r3_p.y) < row_half.y * 1.3 && abs(r3_p.x) < row_half.x * 1.1) {
            let r3_col = render_modal_row(r3_p, row_half, row_r, pressed_item == 3u, col_btn_bg, col_accent_state, aa);
            let icon_d = draw_save_icon(r3_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.020);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r3_p, text_start_x, char_sz, spacing, stroke_w, 83u, 65u, 86u, 69u, 32u, 47u, 32u, 76u, 79u, 65u, 68u, 11u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r3_col)), final_color);
        }

        // Row 4: Reset Game (Item 4 / Muted Terracotta)
        let r4_c = vec2<f32>(0.50 * aspect, 0.534);
        let r4_p = p - r4_c;
        if (abs(r4_p.y) < row_half.y * 1.3 && abs(r4_p.x) < row_half.x * 1.1) {
            let r4_col = render_modal_row(r4_p, row_half, row_r, pressed_item == 4u, col_btn_bg, col_accent_reset, aa);
            let icon_d = draw_reset_icon(r4_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r4_p, text_start_x, char_sz, spacing, stroke_w, 82u, 69u, 83u, 69u, 84u, 32u, 71u, 65u, 77u, 69u, 0u, 10u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r4_col)), final_color);
        }

        // Row 5: Settings (Item 5 / Muted Steel Gray)
        let r5_c = vec2<f32>(0.50 * aspect, 0.630);
        let r5_p = p - r5_c;
        if (abs(r5_p.y) < row_half.y * 1.3 && abs(r5_p.x) < row_half.x * 1.1) {
            let r5_col = render_modal_row(r5_p, row_half, row_r, pressed_item == 5u, col_btn_bg, col_accent_settings, aa);
            let icon_d = draw_gear_icon(r5_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r5_p, text_start_x, char_sz, spacing, stroke_w, 83u, 69u, 84u, 84u, 73u, 78u, 71u, 83u, 0u, 0u, 0u, 8u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r5_col)), final_color);
        }

        // Row 6: Cheat Codes (Item 6 / Muted Mauve)
        let r6_c = vec2<f32>(0.50 * aspect, 0.726);
        let r6_p = p - r6_c;
        if (abs(r6_p.y) < row_half.y * 1.3 && abs(r6_p.x) < row_half.x * 1.1) {
            let r6_col = render_modal_row(r6_p, row_half, row_r, pressed_item == 6u, col_btn_bg, col_accent_cheats, aa);
            let icon_d = draw_cheats_icon(r6_p - vec2<f32>(-row_half.x + 0.035, 0.0), 0.024);
            let icon_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, icon_d));
            let txt_d = draw_vector_string(r6_p, text_start_x, char_sz, spacing, stroke_w, 67u, 72u, 69u, 65u, 84u, 83u, 0u, 0u, 0u, 0u, 0u, 6u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(icon_col, r6_col)), final_color);
        }

        return final_color;
    }

    if (uniforms.menu_state == 2u || uniforms.menu_state == 3u) {
        // ====================================================================
        // Multi-Slot Save / Load State Manager Modal (Opaque Dark UI)
        // ====================================================================
        let is_save_mode = (uniforms.menu_state == 2u);
        let header_accent = select(col_accent_rom, col_accent_resume, is_save_mode);

        // 1. Fullscreen Dark Dimming Backdrop
        let backdrop = vec4<f32>(0.03, 0.04, 0.05, 0.95);
        final_color = backdrop;

        // 2. Centered Solid Opaque Modal Card
        let modal_c = vec2<f32>(0.50 * aspect, 0.50);
        let modal_half = vec2<f32>(0.28 * aspect, 0.40);
        let modal_r = 0.024;
        let card_p = p - modal_c;
        let card_d = sd_rounded_box(card_p, modal_half, modal_r);

        let card_fill_alpha = smoothstep(aa, -aa, card_d);
        let card_rim_d = abs(card_d + 0.002) - 0.002;
        let card_rim_alpha = smoothstep(aa, -aa, card_rim_d);

        let card_bg = vec4<f32>(col_card_bg.rgb, col_card_bg.a * card_fill_alpha);
        let card_rim = vec4<f32>(col_card_rim.rgb, col_card_rim.a * card_rim_alpha);

        final_color = blend_over(card_rim, blend_over(card_bg, final_color));

        // 3. Header: Save or Load Icon + Title Indicator
        let hdr_c = vec2<f32>(0.50 * aspect, 0.155);
        let hdr_p = p - hdr_c;
        if (length(hdr_p) < 0.08) {
            let icon_d = select(draw_load_icon(hdr_p - vec2<f32>(-0.085, 0.0), 0.022), draw_save_icon(hdr_p - vec2<f32>(-0.085, 0.0), 0.020), is_save_mode);
            let icon_col = vec4<f32>(header_accent, 0.95 * smoothstep(aa, -aa, icon_d));

            let hdr_txt_d = select(
                draw_vector_string(hdr_p, -0.058, vec2<f32>(0.011, 0.018), 0.015, 0.0014, 76u, 79u, 65u, 68u, 32u, 83u, 84u, 65u, 84u, 69u, 0u, 10u),
                draw_vector_string(hdr_p, -0.058, vec2<f32>(0.011, 0.018), 0.015, 0.0014, 83u, 65u, 86u, 69u, 32u, 83u, 84u, 65u, 84u, 69u, 0u, 10u),
                is_save_mode
            );
            let hdr_txt_col = vec4<f32>(header_accent, 0.95 * smoothstep(aa, -aa, hdr_txt_d));
            final_color = blend_over(blend_over(hdr_txt_col, icon_col), final_color);
        }

        // Header Divider Line
        let div_c = vec2<f32>(0.50 * aspect, 0.188);
        let div_p = p - div_c;
        let div_d = sd_segment(div_p, vec2<f32>(-modal_half.x * 0.82, 0.0), vec2<f32>(modal_half.x * 0.82, 0.0)) - 0.0010;
        let div_col = vec4<f32>(0.24, 0.27, 0.32, 0.90 * smoothstep(aa, -aa, div_d));
        final_color = blend_over(div_col, final_color);

        // 4. 5 Interactive Slot Rows
        let row_half = vec2<f32>(0.24 * aspect, 0.041);
        let row_r = 0.016;
        let pressed_item = uniforms.menu_pressed_item;
        let slot_mask = uniforms.slot_mask;
        let text_start_x = -row_half.x + 0.078;
        let char_sz = vec2<f32>(0.013, 0.022);
        let spacing = 0.0175;
        let stroke_w = 0.0016;

        // Render Slots 1..=5
        for (var i = 1u; i <= 5u; i = i + 1u) {
            let row_y = 0.246 + f32(i - 1u) * 0.096;
            let row_c = vec2<f32>(0.50 * aspect, row_y);
            let row_p = p - row_c;

            if (abs(row_p.y) < row_half.y * 1.3 && abs(row_p.x) < row_half.x * 1.1) {
                let is_occupied = (slot_mask & (1u << (i - 1u))) != 0u;
                let row_accent = select(vec3<f32>(0.25, 0.28, 0.33), header_accent, is_occupied || is_save_mode);
                let row_col = render_modal_row(row_p, row_half, row_r, pressed_item == i, col_btn_bg, row_accent, aa);

                // Slot digit badge on left
                let digit_d = draw_digit(row_p - vec2<f32>(-row_half.x + 0.035, 0.0), i, 0.022);
                let digit_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, digit_d));

                // Slot text "SLOT 1" .. "SLOT 5"
                let slot_txt_d = draw_vector_string(row_p, text_start_x, char_sz, spacing, stroke_w, 83u, 76u, 79u, 84u, 32u, 48u + i, 0u, 0u, 0u, 0u, 0u, 6u);
                let slot_txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, slot_txt_d));

                // Status indicator / Action pill on right
                let pill_half = vec2<f32>(0.042 * aspect, 0.022);
                let pill_c = vec2<f32>(row_half.x - 0.052, 0.0);
                let pill_d = sd_rounded_box(row_p - pill_c, pill_half, 0.010);
                let pill_bg = vec4<f32>(row_accent, select(0.35, 0.85, is_occupied || is_save_mode) * smoothstep(aa, -aa, pill_d));

                let status_txt_d = select(
                    draw_vector_string(row_p - pill_c, -0.024, vec2<f32>(0.008, 0.014), 0.011, 0.0012, 69u, 77u, 80u, 84u, 89u, 0u, 0u, 0u, 0u, 0u, 0u, 5u),
                    draw_vector_string(row_p - pill_c, -0.024, vec2<f32>(0.008, 0.014), 0.011, 0.0012, 83u, 65u, 86u, 69u, 68u, 0u, 0u, 0u, 0u, 0u, 0u, 5u),
                    is_occupied
                );
                let status_txt_col = vec4<f32>(vec3<f32>(1.0), 0.95 * smoothstep(aa, -aa, status_txt_d));

                var combined_row = blend_over(blend_over(slot_txt_col, blend_over(digit_col, row_col)), final_color);
                combined_row = blend_over(status_txt_col, blend_over(pill_bg, combined_row));
                final_color = combined_row;
            }
        }

        // 5. Bottom Action Bar: [ ← Back ] and [ ⇄ Toggle Mode ]
        let btm_y = 0.7575;
        let btn_half = vec2<f32>(0.1125 * aspect, 0.0325);
        let btn_r = 0.014;

        // Back Button (Item 6)
        let back_c = vec2<f32>(0.3725 * aspect, btm_y);
        let back_p = p - back_c;
        if (abs(back_p.y) < btn_half.y * 1.3 && abs(back_p.x) < btn_half.x * 1.1) {
            let back_col = render_modal_row(back_p, btn_half, btn_r, pressed_item == 6u, col_btn_bg, col_accent_settings, aa);
            let arrow_d = draw_back_arrow(back_p - vec2<f32>(-btn_half.x + 0.028, 0.0), 0.024);
            let arrow_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, arrow_d));
            let txt_d = draw_vector_string(back_p, -btn_half.x + 0.048, vec2<f32>(0.011, 0.018), 0.015, 0.0014, 66u, 65u, 67u, 75u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 4u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(arrow_col, back_col)), final_color);
        }

        // Mode Toggle Button (Item 7)
        let toggle_c = vec2<f32>(0.6275 * aspect, btm_y);
        let toggle_p = p - toggle_c;
        if (abs(toggle_p.y) < btn_half.y * 1.3 && abs(toggle_p.x) < btn_half.x * 1.1) {
            let toggle_col = render_modal_row(toggle_p, btn_half, btn_r, pressed_item == 7u, col_btn_bg, header_accent, aa);
            let swap_d = draw_swap_icon(toggle_p - vec2<f32>(-btn_half.x + 0.028, 0.0), 0.024);
            let swap_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, swap_d));
            let txt_d = draw_vector_string(toggle_p, -btn_half.x + 0.048, vec2<f32>(0.011, 0.018), 0.015, 0.0014, 77u, 79u, 68u, 69u, 0u, 0u, 0u, 0u, 0u, 0u, 0u, 4u);
            let txt_col = vec4<f32>(col_glyph.rgb, col_glyph.a * smoothstep(aa, -aa, txt_d));
            final_color = blend_over(blend_over(txt_col, blend_over(swap_col, toggle_col)), final_color);
        }

        return final_color;
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

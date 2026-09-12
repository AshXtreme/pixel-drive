//! Home Screen / Library Carousel Shader
//!
//! Procedurally renders the console library carousel, ambient vignette,
//! snapshot thumbnail textures, selection halo, vector typography, and
//! pagination indicators.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    let pos = positions[in_vertex_index];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = vec2<f32>((pos.x + 1.0) * 0.5, (1.0 - pos.y) * 0.5);
    return out;
}

struct HomeScreenUniforms {
    screen_size: vec2<f32>,
    aspect_ratio: f32,
    scroll_offset: f32,

    selected_index: u32,
    total_tiles: u32,
    anim_time: f32,
    is_add_game_selected: u32,

    has_thumbnail: u32,
    title_len: u32,
    thumb_width: u32,
    thumb_height: u32,

    title_chars_0: vec4<u32>,
    title_chars_1: vec4<u32>,
    title_chars_2: vec4<u32>,
    title_chars_3: vec4<u32>,
};

@group(0) @binding(0) var thumb_texture: texture_2d<f32>;
@group(0) @binding(1) var thumb_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: HomeScreenUniforms;

// ============================================================================
// Distance Field Math Utilities
// ============================================================================

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r, r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn sd_circle(p: vec2<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn blend_over(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if (out_a <= 0.0001) {
        return vec4<f32>(0.0);
    }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}

// Procedural Plus Icon ("➕") for the Add Game Tile
fn draw_plus_icon(p: vec2<f32>, sz: f32) -> f32 {
    let th = 0.0035;
    let arm = sz * 0.45;
    let h = sd_segment(p, vec2<f32>(-arm, 0.0), vec2<f32>(arm, 0.0)) - th;
    let v = sd_segment(p, vec2<f32>(0.0, -arm), vec2<f32>(0.0, arm)) - th;
    return min(h, v);
}

// Procedural Game Cartridge Silhouette Placeholder Icon
fn draw_cartridge_icon(p: vec2<f32>, sz: f32) -> f32 {
    let th = 0.0020;
    let body = sd_rounded_box(p, vec2<f32>(sz * 0.40, sz * 0.50), sz * 0.06);
    let rim = abs(body + 0.001) - th;
    let notch = sd_segment(p, vec2<f32>(-sz * 0.25, -sz * 0.20), vec2<f32>(sz * 0.25, -sz * 0.20)) - th;
    let grip1 = sd_segment(p, vec2<f32>(-sz * 0.25, sz * 0.15), vec2<f32>(sz * 0.25, sz * 0.15)) - th;
    let grip2 = sd_segment(p, vec2<f32>(-sz * 0.25, sz * 0.28), vec2<f32>(sz * 0.25, sz * 0.28)) - th;
    return min(rim, min(notch, min(grip1, grip2)));
}

// ============================================================================
// 16-Segment Vector Typography Engine
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
    if (c == 74u) { return 124u;   } // J
    if (c == 75u) { return 41408u; } // K
    if (c == 76u) { return 240u;   } // L
    if (c == 77u) { return 12492u; } // M
    if (c == 78u) { return 37068u; } // N
    if (c == 79u) { return 255u;   } // O
    if (c == 80u) { return 967u;   } // P
    if (c == 81u) { return 33023u; } // Q
    if (c == 82u) { return 33735u; } // R
    if (c == 83u) { return 955u;   } // S
    if (c == 84u) { return 3075u;  } // T
    if (c == 85u) { return 252u;   } // U
    if (c == 86u) { return 24768u; } // V
    if (c == 87u) { return 49356u; } // W
    if (c == 88u) { return 61440u; } // X
    if (c == 89u) { return 14336u; } // Y
    if (c == 90u) { return 24627u; } // Z
    if (c == 48u) { return 255u;   } // 0
    if (c == 49u) { return 3072u;  } // 1
    if (c == 50u) { return 887u;   } // 2
    if (c == 51u) { return 831u;   } // 3
    if (c == 52u) { return 908u;   } // 4
    if (c == 53u) { return 955u;   } // 5
    if (c == 54u) { return 1019u;  } // 6
    if (c == 55u) { return 24579u; } // 7
    if (c == 56u) { return 1023u;  } // 8
    if (c == 57u) { return 959u;   } // 9
    if (c == 47u) { return 24576u; } // /
    if (c == 45u) { return 768u;   } // -
    if (c == 91u) { return 243u;   } // [
    if (c == 93u) { return 60u;    } // ]
    if (c == 43u) { return 3072u | 768u; } // +
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

fn draw_vector_string_12(
    p: vec2<f32>,
    start_x: f32,
    sz: vec2<f32>,
    spacing: f32,
    stroke_w: f32,
    c0: u32, c1: u32, c2: u32, c3: u32,
    c4: u32, c5: u32, c6: u32, c7: u32,
    c8: u32, c9: u32, c10: u32, c11: u32,
    len: u32
) -> f32 {
    let raw_idx = (p.x - start_x + spacing * 0.5) / spacing;
    if (raw_idx < 0.0 || raw_idx >= f32(len)) {
        return 1e5;
    }
    let idx = u32(floor(raw_idx));
    var code = 0u;
    switch idx {
        case 0u:  { code = c0;  }
        case 1u:  { code = c1;  }
        case 2u:  { code = c2;  }
        case 3u:  { code = c3;  }
        case 4u:  { code = c4;  }
        case 5u:  { code = c5;  }
        case 6u:  { code = c6;  }
        case 7u:  { code = c7;  }
        case 8u:  { code = c8;  }
        case 9u:  { code = c9;  }
        case 10u: { code = c10; }
        case 11u: { code = c11; }
        default:  { code = 0u;  }
    }
    if (code == 0u || code == 32u) {
        return 1e5;
    }
    let char_center_x = start_x + f32(idx) * spacing;
    let local_p = vec2<f32>(p.x - char_center_x, p.y);
    return draw_vector_char(local_p, code, sz, stroke_w);
}

// ============================================================================
// Main Fragment Shader
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let aspect = uniforms.aspect_ratio;
    let p = vec2<f32>(in.uv.x * aspect, in.uv.y);
    let aa = 1.0 / uniforms.screen_size.y;

    // 1. Ambient Slate Vignette Backdrop
    let center_dist = length(in.uv - vec2<f32>(0.5, 0.5));
    let col_center = vec3<f32>(0.055, 0.075, 0.105);
    let col_edge = vec3<f32>(0.015, 0.020, 0.028);
    let bg_rgb = mix(col_center, col_edge, smoothstep(0.15, 0.85, center_dist));
    var final_color = vec4<f32>(bg_rgb, 1.0);

    // Subtle ambient glow behind active card
    let halo_c = vec2<f32>(0.50 * aspect, 0.46);
    let halo_d = length(p - halo_c);
    let halo_alpha = 0.22 * smoothstep(0.45 * aspect, 0.05, halo_d);
    final_color = blend_over(vec4<f32>(0.15, 0.45, 0.55, halo_alpha), final_color);

    // 2. Top Header: Title and Divider
    let hdr_y = 0.10;
    let title_start_x = 0.50 * aspect - 0.080;
    let title_sz = vec2<f32>(0.012, 0.020);
    let title_spacing = 0.016;
    let title_stroke = 0.0016;

    // "PIXELDRIVE"
    let logo_d = draw_vector_string_12(
        p - vec2<f32>(0.0, hdr_y),
        title_start_x,
        title_sz,
        title_spacing,
        title_stroke,
        80u, 73u, 88u, 69u, 76u, 68u, 82u, 73u, 86u, 69u, 0u, 0u,
        10u
    );
    let logo_col = vec4<f32>(0.92, 0.96, 1.0, 0.95 * smoothstep(aa, -aa, logo_d));
    final_color = blend_over(logo_col, final_color);

    // "GAMES LIBRARY" subtitle
    let sub_y = 0.142;
    let sub_start_x = 0.50 * aspect - 0.076;
    let sub_d = draw_vector_string_12(
        p - vec2<f32>(0.0, sub_y),
        sub_start_x,
        vec2<f32>(0.008, 0.013),
        0.012,
        0.0013,
        76u, 73u, 66u, 82u, 65u, 82u, 89u, 0u, 0u, 0u, 0u, 0u,
        7u
    );
    let sub_col = vec4<f32>(0.45, 0.58, 0.70, 0.85 * smoothstep(aa, -aa, sub_d));
    final_color = blend_over(sub_col, final_color);

    // Horizontal header divider rule
    let div_p = p - vec2<f32>(0.50 * aspect, 0.170);
    let div_d = sd_segment(div_p, vec2<f32>(-0.32 * aspect, 0.0), vec2<f32>(0.32 * aspect, 0.0)) - 0.0008;
    let div_col = vec4<f32>(0.20, 0.28, 0.38, 0.75 * smoothstep(aa, -aa, div_d));
    final_color = blend_over(div_col, final_color);

    // 3. Carousel Cards Layout
    // Isotropic card dimensions preserving a 3:2 aspect ratio (0.285 / 0.190 = 1.50)
    let card_w = 0.285;
    let card_h = 0.190;
    let card_half = vec2<f32>(card_w, card_h);
    let card_r = 0.016;
    let center_y = 0.46;
    let card_spacing = card_w * 2.22; // ~0.632 isotropic spacing

    let center_c = vec2<f32>(0.50 * aspect + uniforms.scroll_offset, center_y);

    // Previous neighbor card (offset left)
    if (uniforms.selected_index > 0u) {
        let prev_c = vec2<f32>(center_c.x - card_spacing, center_y);
        let prev_p = p - prev_c;
        let prev_half = card_half * 0.82;
        let prev_d = sd_rounded_box(prev_p, prev_half, card_r * 0.82);
        let prev_fill = vec4<f32>(0.06, 0.07, 0.09, 0.45 * smoothstep(aa, -aa, prev_d));
        let prev_rim = vec4<f32>(0.25, 0.30, 0.38, 0.40 * smoothstep(aa, -aa, abs(prev_d + 0.0015) - 0.0015));
        final_color = blend_over(prev_rim, blend_over(prev_fill, final_color));
    }

    // Next neighbor card (offset right)
    if (uniforms.selected_index < uniforms.total_tiles - 1u) {
        let next_c = vec2<f32>(center_c.x + card_spacing, center_y);
        let next_p = p - next_c;
        let next_half = card_half * 0.82;
        let next_d = sd_rounded_box(next_p, next_half, card_r * 0.82);
        let next_fill = vec4<f32>(0.06, 0.07, 0.09, 0.45 * smoothstep(aa, -aa, next_d));
        let next_rim = vec4<f32>(0.25, 0.30, 0.38, 0.40 * smoothstep(aa, -aa, abs(next_d + 0.0015) - 0.0015));
        final_color = blend_over(next_rim, blend_over(next_fill, final_color));
    }

    // Center Active Card
    let card_p = p - center_c;
    let card_d = sd_rounded_box(card_p, card_half, card_r);

    // Soft drop shadow below active card
    let shadow_p = card_p - vec2<f32>(0.0, 0.014);
    let shadow_d = sd_rounded_box(shadow_p, card_half + vec2<f32>(0.006, 0.006), card_r + 0.012);
    let shadow_alpha = 0.50 * smoothstep(0.045, -0.005, shadow_d);
    final_color = blend_over(vec4<f32>(0.0, 0.0, 0.0, shadow_alpha), final_color);

    if (card_d < 0.02) {
        let card_alpha = smoothstep(aa, -aa, card_d);

        // Animated neon focus ring
        let pulse = 0.85 + 0.15 * sin(uniforms.anim_time * 3.5);
        let rim_thickness = 0.0032;
        let rim_d = abs(card_d + rim_thickness * 0.5) - rim_thickness * 0.5;
        let rim_alpha = smoothstep(aa, -aa, rim_d);
        let rim_col = vec4<f32>(0.32, 0.85, 0.78, pulse * rim_alpha);

        // Card Interior Rendering
        var interior_col = vec4<f32>(0.04, 0.05, 0.07, 0.98 * card_alpha);

        if (uniforms.is_add_game_selected != 0u) {
            // "➕ Add Game" Tile
            let plus_d = draw_plus_icon(card_p, 0.048);
            let plus_col = vec4<f32>(0.35, 0.85, 0.75, 0.95 * smoothstep(aa, -aa, plus_d));
            interior_col = blend_over(plus_col, interior_col);

            let add_txt_d = draw_vector_string_12(
                card_p - vec2<f32>(0.0, 0.055),
                -0.054,
                vec2<f32>(0.009, 0.015),
                0.014,
                0.0014,
                65u, 68u, 68u, 32u, 71u, 65u, 77u, 69u, 0u, 0u, 0u, 0u,
                8u
            );
            let add_txt_col = vec4<f32>(0.85, 0.92, 0.98, 0.90 * smoothstep(aa, -aa, add_txt_d));
            interior_col = blend_over(add_txt_col, interior_col);
        } else {
            // Game ROM Tile: Render Thumbnail Texture or Placeholder
            if (uniforms.has_thumbnail != 0u) {
                var thumb_ar = 1.5;
                if (uniforms.thumb_width > 0u && uniforms.thumb_height > 0u) {
                    thumb_ar = f32(uniforms.thumb_width) / f32(uniforms.thumb_height);
                }

                let inner_margin = 0.008;
                let inner_half = card_half - vec2<f32>(inner_margin, inner_margin);
                let inner_ar = inner_half.x / inner_half.y;

                var fit_half = inner_half;
                if (thumb_ar >= inner_ar) {
                    fit_half = vec2<f32>(inner_half.x, inner_half.x / thumb_ar);
                } else {
                    fit_half = vec2<f32>(inner_half.y * thumb_ar, inner_half.y);
                }

                let fit_d = sd_rounded_box(card_p, fit_half, 0.008);
                if (fit_d <= 0.002) {
                    let tex_uv = (card_p / (fit_half * 2.0)) + vec2<f32>(0.5, 0.5);
                    let sampled_col = textureSampleLevel(thumb_texture, thumb_sampler, clamp(tex_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
                    let thumb_alpha = smoothstep(aa, -aa, fit_d) * card_alpha;
                    let thumb_col = vec4<f32>(sampled_col.rgb, sampled_col.a * thumb_alpha);
                    interior_col = blend_over(thumb_col, interior_col);

                    // Crisp 1px inner bezel around thumbnail
                    let rim_d = abs(fit_d + 0.0010) - 0.0010;
                    let rim_c = vec4<f32>(0.35, 0.45, 0.55, 0.50 * smoothstep(aa, -aa, rim_d) * card_alpha);
                    interior_col = blend_over(rim_c, interior_col);
                }
            } else {
                // Stylish Dark Placeholder Cartridge
                let cart_d = draw_cartridge_icon(card_p, 0.052);
                let cart_col = vec4<f32>(0.45, 0.55, 0.65, 0.80 * smoothstep(aa, -aa, cart_d));
                interior_col = blend_over(cart_col, interior_col);
            }
        }

        // Composite interior and neon glowing rim
        final_color = blend_over(rim_col, blend_over(interior_col, final_color));
    }

    // 4. Selected Game Title Label (Centered below active card)
    let label_y = 0.72;
    let char_sz = vec2<f32>(0.011, 0.018);
    let spacing = 0.015;
    let stroke_w = 0.0016;
    let title_total_w = f32(uniforms.title_len) * spacing;
    let label_start_x = 0.50 * aspect - title_total_w * 0.5 + spacing * 0.5;

    let title_p = p - vec2<f32>(0.0, label_y);
    let title_d = draw_vector_string_12(
        title_p,
        label_start_x,
        char_sz,
        spacing,
        stroke_w,
        uniforms.title_chars_0.x, uniforms.title_chars_0.y,
        uniforms.title_chars_0.z, uniforms.title_chars_0.w,
        uniforms.title_chars_1.x, uniforms.title_chars_1.y,
        uniforms.title_chars_1.z, uniforms.title_chars_1.w,
        uniforms.title_chars_2.x, uniforms.title_chars_2.y,
        uniforms.title_chars_2.z, uniforms.title_chars_2.w,
        min(uniforms.title_len, 12u)
    );
    let title_col = vec4<f32>(1.0, 1.0, 1.0, 0.95 * smoothstep(aa, -aa, title_d));
    final_color = blend_over(title_col, final_color);

    // 5. Action Prompt Pill
    let prompt_y = 0.77;
    let prompt_p = p - vec2<f32>(0.50 * aspect, prompt_y);
    let prompt_half = vec2<f32>(0.09 * aspect, 0.020);
    let prompt_d = sd_rounded_box(prompt_p, prompt_half, 0.010);
    let prompt_bg = vec4<f32>(0.12, 0.16, 0.22, 0.75 * smoothstep(aa, -aa, prompt_d));
    let prompt_rim = vec4<f32>(0.28, 0.40, 0.52, 0.65 * smoothstep(aa, -aa, abs(prompt_d + 0.0012) - 0.0012));
    final_color = blend_over(prompt_rim, blend_over(prompt_bg, final_color));

    // Prompt text: "[A] PLAY" or "[+] LOAD"
    let prompt_txt_d = select(
        draw_vector_string_12(prompt_p, -0.040, vec2<f32>(0.008, 0.013), 0.0115, 0.0012, 91u, 65u, 93u, 32u, 80u, 76u, 65u, 89u, 0u, 0u, 0u, 0u, 8u),
        draw_vector_string_12(prompt_p, -0.040, vec2<f32>(0.008, 0.013), 0.0115, 0.0012, 91u, 43u, 93u, 32u, 76u, 79u, 65u, 68u, 0u, 0u, 0u, 0u, 8u),
        uniforms.is_add_game_selected != 0u
    );
    let prompt_txt_col = vec4<f32>(0.85, 0.95, 0.90, 0.95 * smoothstep(aa, -aa, prompt_txt_d));
    final_color = blend_over(prompt_txt_col, final_color);

    // 6. Navigation Dots (Pagination)
    let dot_y = 0.84;
    let total_dots = min(uniforms.total_tiles, 10u);
    let dot_spacing = 0.022;
    let dot_start_x = 0.50 * aspect - (f32(total_dots - 1u) * dot_spacing) * 0.5;

    for (var i = 0u; i < total_dots; i = i + 1u) {
        let dot_c = vec2<f32>(dot_start_x + f32(i) * dot_spacing, dot_y);
        let is_active = (i == uniforms.selected_index);
        let dot_r = select(0.0035, 0.0055, is_active);
        let dot_d = sd_circle(p - dot_c, dot_r);
        let dot_color = select(vec4<f32>(0.35, 0.42, 0.52, 0.50), vec4<f32>(0.35, 0.85, 0.78, 0.95), is_active);
        final_color = blend_over(dot_color * smoothstep(aa, -aa, dot_d), final_color);
    }

    return final_color;
}

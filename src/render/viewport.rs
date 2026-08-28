//! Dynamic Viewport and Framebuffer Aspect-Ratio Calculation for PixelDrive.

/// Core display dimensions and aspect ratios for supported consoles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportConfig {
    pub tex_width: u32,
    pub tex_height: u32,
    pub output_width: u32,
    pub output_height: u32,
}

/// Viewport destination rectangle within the physical window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ViewportConfig {
    pub const GBA_WIDTH: u32 = 240;
    pub const GBA_HEIGHT: u32 = 160;
    pub const GBC_WIDTH: u32 = 160;
    pub const GBC_HEIGHT: u32 = 144;

    /// Create standard GBA viewport configuration (240x160)
    pub fn new_gba(output_width: u32, output_height: u32) -> Self {
        Self {
            tex_width: Self::GBA_WIDTH,
            tex_height: Self::GBA_HEIGHT,
            output_width,
            output_height,
        }
    }

    /// Create standard GBC viewport configuration (160x144)
    pub fn new_gbc(output_width: u32, output_height: u32) -> Self {
        Self {
            tex_width: Self::GBC_WIDTH,
            tex_height: Self::GBC_HEIGHT,
            output_width,
            output_height,
        }
    }

    /// Compute aspect ratio of native game framebuffer (Width / Height)
    pub fn target_aspect(&self) -> f32 {
        self.tex_width as f32 / self.tex_height.max(1) as f32
    }

    /// Compute aspect ratio of physical display output (Width / Height)
    pub fn output_aspect(&self) -> f32 {
        self.output_width as f32 / self.output_height.max(1) as f32
    }

    /// Calculate centered letterbox/pillarbox viewport destination rectangle
    pub fn calculate_viewport_rect(&self) -> ViewportRect {
        let out_w = self.output_width as f32;
        let out_h = self.output_height as f32;

        let target_asp = self.target_aspect();
        let screen_asp = self.output_aspect();

        if screen_asp > target_asp {
            // Screen is wider than game: Pillarboxing (black bars left/right)
            let draw_w = out_h * target_asp;
            let offset_x = (out_w - draw_w) * 0.5;
            ViewportRect {
                x: offset_x,
                y: 0.0,
                width: draw_w,
                height: out_h,
            }
        } else {
            // Screen is taller than game: Letterboxing (black bars top/bottom)
            let draw_h = out_w / target_asp;
            let offset_y = (out_h - draw_h) * 0.5;
            ViewportRect {
                x: 0.0,
                y: offset_y,
                width: out_w,
                height: draw_h,
            }
        }
    }

    /// Map physical window coordinates to normalized game texture UV [0.0, 1.0]
    pub fn window_to_texture_uv(&self, win_x: f32, win_y: f32) -> Option<(f32, f32)> {
        let rect = self.calculate_viewport_rect();
        if win_x < rect.x
            || win_x > rect.x + rect.width
            || win_y < rect.y
            || win_y > rect.y + rect.height
        {
            return None;
        }

        let u = (win_x - rect.x) / rect.width;
        let v = (win_y - rect.y) / rect.height;
        Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gba_aspect_ratio() {
        let vp = ViewportConfig::new_gba(1920, 1080);
        assert_eq!(vp.tex_width, 240);
        assert_eq!(vp.tex_height, 160);
        assert!((vp.target_aspect() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_gbc_aspect_ratio() {
        let vp = ViewportConfig::new_gbc(1920, 1080);
        assert_eq!(vp.tex_width, 160);
        assert_eq!(vp.tex_height, 144);
        assert!((vp.target_aspect() - (160.0 / 144.0)).abs() < 1e-5);
    }

    #[test]
    fn test_pillarboxing_on_widescreen() {
        let vp = ViewportConfig::new_gba(1920, 1080);
        let rect = vp.calculate_viewport_rect();
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.height, 1080.0);
        assert!((rect.width - 1620.0).abs() < 1e-3); // 1080 * 1.5 = 1620
        assert!((rect.x - 150.0).abs() < 1e-3); // (1920 - 1620) / 2 = 150
    }

    #[test]
    fn test_letterboxing_on_portrait() {
        let vp = ViewportConfig::new_gba(1080, 1920);
        let rect = vp.calculate_viewport_rect();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.width, 1080.0);
        assert!((rect.height - 720.0).abs() < 1e-3); // 1080 / 1.5 = 720
        assert!((rect.y - 600.0).abs() < 1e-3); // (1920 - 720) / 2 = 600
    }

    #[test]
    fn test_window_to_texture_uv_mapping() {
        let vp = ViewportConfig::new_gba(1920, 1080);
        // Center of window is center of texture UV
        let uv = vp.window_to_texture_uv(960.0, 540.0);
        assert!(uv.is_some());
        let (u, v) = uv.unwrap();
        assert!((u - 0.5).abs() < 1e-4);
        assert!((v - 0.5).abs() < 1e-4);

        // Outside letterbox/pillarbox area returns None
        assert!(vp.window_to_texture_uv(50.0, 540.0).is_none());
    }
}

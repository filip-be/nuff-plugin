//! Tray and window icons drawn in code (RGBA, straight alpha): a pulse ring,
//! plus a red badge with the alert count.

const ACCENT: [u8; 3] = [0x2F, 0x9E, 0xD8];
const BADGE: [u8; 3] = [0xD9, 0x30, 0x25];
const NEEDS: [u8; 3] = [0xF5, 0x9E, 0x0B];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];

/// 3×5 digits, one row per byte, high bit of the low three bits = left.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b011, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
const PLUS: [u8; 5] = [0b000, 0b010, 0b111, 0b010, 0b000];

struct Canvas {
    size: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(size: usize) -> Canvas {
        Canvas { size, px: vec![0; size * size * 4] }
    }

    /// Blends `color` at coverage `a` (0..1) over pixel (x, y).
    fn blend(&mut self, x: usize, y: usize, color: [u8; 3], a: f32) {
        if x >= self.size || y >= self.size || a <= 0.0 {
            return;
        }
        let i = (y * self.size + x) * 4;
        let a = a.min(1.0);
        let da = self.px[i + 3] as f32 / 255.0;
        let out_a = a + da * (1.0 - a);
        for c in 0..3 {
            let src = color[c] as f32;
            let dst = self.px[i + c] as f32;
            self.px[i + c] = ((src * a + dst * da * (1.0 - a)) / out_a.max(1e-6)).round() as u8;
        }
        self.px[i + 3] = (out_a * 255.0).round() as u8;
    }

    /// Anti-aliased disc (`inner` > 0 makes a ring).
    fn disc(&mut self, cx: f32, cy: f32, r: f32, inner: f32, color: [u8; 3]) {
        for y in 0..self.size {
            for x in 0..self.size {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let outer = (r - d + 0.5).clamp(0.0, 1.0);
                let inside = if inner > 0.0 { (d - inner + 0.5).clamp(0.0, 1.0) } else { 1.0 };
                self.blend(x, y, color, outer * inside);
            }
        }
    }

    /// A 3×5 glyph scaled by `cell` pixels, top-left at (x0, y0).
    fn glyph(&mut self, rows: &[u8; 5], x0: f32, y0: f32, cell: f32, color: [u8; 3]) {
        for (ry, bits) in rows.iter().enumerate() {
            for rx in 0..3 {
                if bits & (0b100 >> rx) == 0 {
                    continue;
                }
                let (gx, gy) = (x0 + rx as f32 * cell, y0 + ry as f32 * cell);
                for y in gy.floor() as i64..(gy + cell).ceil() as i64 {
                    for x in gx.floor() as i64..(gx + cell).ceil() as i64 {
                        if x < 0 || y < 0 {
                            continue;
                        }
                        let cov_x = ((x as f32 + 1.0).min(gx + cell) - (x as f32).max(gx)).clamp(0.0, 1.0);
                        let cov_y = ((y as f32 + 1.0).min(gy + cell) - (y as f32).max(gy)).clamp(0.0, 1.0);
                        self.blend(x as usize, y as usize, color, cov_x * cov_y);
                    }
                }
            }
        }
    }
}

/// The icon at `size`×`size` with a badge showing `alerts` (none when 0;
/// `+` above 9). `lit` false is the other blink frame: an amber ring and no
/// badge, so the blink shows even when every alert was seen.
pub fn render(size: u32, alerts: usize, lit: bool) -> Vec<u8> {
    let s = size.max(8) as usize;
    let f = s as f32;
    let mut c = Canvas::new(s);
    // Pulse ring with a dot in the middle.
    let ring = if lit { ACCENT } else { NEEDS };
    c.disc(f / 2.0, f / 2.0, f * 0.46, f * 0.30, ring);
    c.disc(f / 2.0, f / 2.0, f * 0.16, 0.0, ring);
    if alerts > 0 && lit {
        let r = f * 0.30;
        let (cx, cy) = (f - r, f - r);
        c.disc(cx, cy, r + f * 0.04, 0.0, WHITE);
        c.disc(cx, cy, r, 0.0, BADGE);
        let glyph = if alerts > 9 { &PLUS } else { &DIGITS[alerts] };
        let cell = (r * 1.2 / 5.0).max(1.0);
        c.glyph(glyph, cx - 1.5 * cell, cy - 2.5 * cell, cell, WHITE);
    }
    c.px
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(px: &[u8], size: usize, x: usize, y: usize) -> u8 {
        px[(y * size + x) * 4 + 3]
    }

    #[test]
    fn draws_ring_and_badge() {
        let plain = render(32, 0, true);
        assert_eq!(plain.len(), 32 * 32 * 4);
        assert_eq!(alpha(&plain, 32, 0, 0), 0, "corner is transparent");
        assert_eq!(alpha(&plain, 32, 16, 16), 255, "centre dot");
        let badged = render(32, 3, true);
        // The badge covers the bottom-right corner area.
        let i = (27 * 32 + 27) * 4;
        assert_eq!(&badged[i..i + 4], &[BADGE[0], BADGE[1], BADGE[2], 255]);
        assert_ne!(badged, plain);
        let dark = render(32, 3, false);
        assert_ne!(dark, plain, "the other blink frame differs even without alerts");
        assert_eq!(dark, render(32, 0, false), "and has no badge");
        assert_ne!(render(16, 12, true), render(16, 0, true));
    }
}

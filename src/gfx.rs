//! Colour helpers and the tiny software rasteriser used for all art in the game.
//! Every texture and sprite is generated at start-up; there are no asset files.

pub type Color = u32; // 0xAARRGGBB, 0 in the alpha byte means "transparent"

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub const fn alpha(c: Color) -> u8 {
    (c >> 24) as u8
}

pub fn red(c: Color) -> u8 {
    ((c >> 16) & 0xFF) as u8
}

pub fn green(c: Color) -> u8 {
    ((c >> 8) & 0xFF) as u8
}

pub fn blue(c: Color) -> u8 {
    (c & 0xFF) as u8
}

/// Multiply a colour's RGB by `f` (0..=1), preserving alpha.
pub fn shade(c: Color, f: f32) -> Color {
    if alpha(c) == 0 {
        return 0;
    }
    let f = f.clamp(0.0, 1.0);
    let r = (red(c) as f32 * f) as u32;
    let g = (green(c) as f32 * f) as u32;
    let b = (blue(c) as f32 * f) as u32;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

/// Linear blend of two opaque colours.
pub fn blend(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    rgb(
        mix(red(a), red(b)),
        mix(green(a), green(b)),
        mix(blue(a), blue(b)),
    )
}

/// Draw `src` over `dst` using source alpha for translucency.
pub fn over(dst: Color, src: Color) -> Color {
    let a = alpha(src) as f32 / 255.0;
    if a <= 0.0 {
        dst
    } else if a >= 1.0 {
        src
    } else {
        blend(dst, src, a)
    }
}

/// A square power-of-two texture (walls, sprites, weapon art).
#[derive(Clone)]
pub struct Tex {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Color>,
}

impl Tex {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            px: vec![0; w * h],
        }
    }

    pub fn filled(w: usize, h: usize, c: Color) -> Self {
        Self {
            w,
            h,
            px: vec![c; w * h],
        }
    }

    pub fn set(&mut self, x: i32, y: i32, c: Color) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Color {
        self.px[y * self.w + x]
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        let x0 = (cx - rx).floor() as i32;
        let x1 = (cx + rx).ceil() as i32;
        let y0 = (cy - ry).floor() as i32;
        let y1 = (cy + ry).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, c);
                }
            }
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.set(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Cheap lighting: multiply everything below/above a horizon.
    pub fn vertical_gradient(&mut self, top: f32, bottom: f32) {
        for y in 0..self.h {
            let t = y as f32 / (self.h - 1) as f32;
            let f = top + (bottom - top) * t;
            for x in 0..self.w {
                let c = self.px[y * self.w + x];
                if alpha(c) != 0 {
                    self.px[y * self.w + x] = shade(c, f);
                }
            }
        }
    }
}

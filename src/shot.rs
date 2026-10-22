//! Headless frame capture, used for testing the renderer without a window.
//! Writes a 24-bit BMP so it needs no image crate.

use crate::game::{Game, Input};
use crate::math::Vec2;
use crate::render::{H, Renderer, W};

pub struct ShotOptions {
    pub at: Option<Vec2>,
    pub angle: Option<f32>,
    pub steps: usize,
    pub fire: bool,
    pub show_map: bool,
    pub use_key: bool,
}

pub fn capture(path: &str, opts: ShotOptions) -> std::io::Result<()> {
    let mut game = Game::new();
    if let Some(pos) = opts.at {
        game.player.pos = pos;
    }
    if let Some(angle) = opts.angle {
        game.player.angle = angle;
    }
    if opts.use_key {
        let input = Input {
            use_pressed: true,
            ..Default::default()
        };
        game.update(1.0 / 60.0, &input);
    }
    let mut held = Input {
        fire: opts.fire,
        ..Default::default()
    };
    for _ in 0..opts.steps.min(1200) {
        game.update(1.0 / 60.0, &held);
    }
    if opts.fire {
        held.fire = true;
        game.update(1.0 / 60.0, &held);
    }
    if opts.show_map {
        game.map_open = true;
    }

    let mut renderer = Renderer::new();
    renderer.render(&game);
    save_bmp(path, W, H, &renderer.frame)
}

fn save_bmp(path: &str, w: usize, h: usize, px: &[u32]) -> std::io::Result<()> {
    let padding = (4 - (w * 3) % 4) % 4;
    let row_size = w * 3 + padding;
    let data_size = row_size * h;
    let file_size = 54 + data_size;
    let mut out = Vec::with_capacity(file_size);

    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..h).rev() {
        for x in 0..w {
            let c = px[y * w + x];
            out.push((c & 0xFF) as u8);
            out.push(((c >> 8) & 0xFF) as u8);
            out.push(((c >> 16) & 0xFF) as u8);
        }
        out.extend(std::iter::repeat_n(0u8, padding));
    }
    std::fs::write(path, out)
}

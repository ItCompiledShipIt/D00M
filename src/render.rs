//! Software renderer: a textured raycasting engine in the Wolfenstein/Doom
//! lineage, plus the HUD, weapon sprite and minimap drawn straight into a
//! 32-bit framebuffer.

use crate::font;
use crate::game::{Game, GameState, Sprite};
use crate::gfx::{Tex, alpha, over, rgb, shade};
use crate::map::{Cell, Map};
use crate::math::Vec2;
use crate::textures::Textures;

pub const W: usize = 480;
pub const H: usize = 300;
pub const VIEW_H: usize = 252;
pub const HUD_H: usize = H - VIEW_H;

const FOV_PLANE: f32 = 0.66;
const HUD_ORANGE: u32 = 0x00E8_6A2C;
const HUD_RED: u32 = 0x00F0_3C28;

pub struct Renderer {
    pub frame: Vec<u32>,
    zbuf: Vec<f32>,
    pub tex: Textures,
}

struct WallHit {
    dist: f32,
    tex: u8,
    side: u8,
    wall_x: f32,
}

struct DoorHit {
    dist: f32,
    tex: u8,
    side: u8,
    wall_x: f32,
    open: f32,
}

fn fog(dist: f32) -> f32 {
    (1.0 / (1.0 + dist * 0.11 + dist * dist * 0.008)).max(0.10)
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            frame: vec![0; W * H],
            zbuf: vec![f32::MAX; W],
            tex: Textures::generate(),
        }
    }

    pub fn render(&mut self, game: &Game) {
        let dir = Vec2::from_angle(game.player.angle);
        let plane = dir.perp() * FOV_PLANE;
        let horizon = VIEW_H as f32 * 0.5 + game.player.bob - game.player.fire_anim * 2.0;

        self.draw_flat(game.player.pos, dir, plane, horizon);
        self.draw_walls(&game.map, game.player.pos, dir, plane, horizon);
        self.draw_sprites(game, dir, plane, horizon);
        self.draw_crosshair(horizon);
        self.draw_weapon(game);
        self.draw_hud(game);
        self.draw_hurt_flash(game);
        if game.map_open {
            self.draw_minimap(game);
        }
        self.draw_state_overlay(game);
    }

    /// Floor and ceiling, one screen row at a time (the classic affine trick).
    fn draw_flat(&mut self, origin: Vec2, dir: Vec2, plane: Vec2, horizon: f32) {
        let w = W;
        let proj = VIEW_H as f32;
        let ray_l = dir - plane;
        let ray_r = dir + plane;
        let frame = &mut self.frame;
        let floor = &self.tex.floor;
        let ceiling = &self.tex.ceiling;

        for y in 0..VIEW_H {
            let dy = y as f32 - horizon;
            let is_floor = dy >= 0.0;
            let denom = dy.abs().max(0.75);
            let row_dist = 0.5 * proj / denom;
            let tex = if is_floor { floor } else { ceiling };
            let light = fog(row_dist) * if is_floor { 0.95 } else { 0.8 };

            if row_dist > 40.0 {
                let c = shade(rgb(8, 8, 10), 1.0);
                for x in 0..w {
                    frame[y * w + x] = c;
                }
                continue;
            }

            let step_x = row_dist * (ray_r.x - ray_l.x) / w as f32;
            let step_y = row_dist * (ray_r.y - ray_l.y) / w as f32;
            let mut wx = origin.x + row_dist * ray_l.x;
            let mut wy = origin.y + row_dist * ray_l.y;
            // Darken outdoor-ish edges of the map.
            for x in 0..w {
                let tx = ((wx * tex.w as f32) as i32).rem_euclid(tex.w as i32) as usize;
                let ty = ((wy * tex.h as f32) as i32).rem_euclid(tex.h as i32) as usize;
                let texel = tex.px[ty * tex.w + tx];
                frame[y * w + x] = shade(texel, light);
                wx += step_x;
                wy += step_y;
            }
        }
    }

    fn draw_walls(&mut self, map: &Map, origin: Vec2, dir: Vec2, plane: Vec2, horizon: f32) {
        let w = W;
        for x in 0..w {
            let camera = 2.0 * x as f32 / w as f32 - 1.0;
            let ray = dir + plane * camera;
            let (wall, door) = cast(map, origin, ray);
            if let Some(hit) = wall {
                self.draw_wall_column(x, &hit, horizon);
            }
            if let Some(d) = door {
                self.draw_door_column(x, &d, horizon);
                self.zbuf[x] = self.zbuf[x].min(d.dist);
            }
        }
    }

    fn draw_wall_column(&mut self, x: usize, hit: &WallHit, horizon: f32) {
        let proj = VIEW_H as f32;
        let line_h = proj / hit.dist.max(0.05);
        let top = horizon - line_h * 0.5;
        let tex = self.tex.wall(hit.tex);
        let tex_x = ((hit.wall_x * tex.w as f32).clamp(0.0, tex.w as f32 - 1.0)) as usize;
        let light = fog(hit.dist) * if hit.side == 0 { 1.0 } else { 0.7 };
        let y0 = top.max(0.0) as usize;
        let y1 = ((top + line_h).min(VIEW_H as f32)) as usize;
        for y in y0..y1 {
            let ty = (((y as f32 - top) / line_h) * tex.h as f32).clamp(0.0, tex.h as f32 - 1.0);
            let texel = tex.px[ty as usize * tex.w + tex_x];
            self.frame[y * W + x] = shade(texel, light);
        }
        self.zbuf[x] = hit.dist;
    }

    /// A door is a panel that slides up into the ceiling, leaving a gap below.
    fn draw_door_column(&mut self, x: usize, d: &DoorHit, horizon: f32) {
        let visible = 1.0 - d.open;
        if visible <= 0.02 {
            return;
        }
        let proj = VIEW_H as f32;
        let line_h = proj / d.dist.max(0.05);
        // The panel is anchored to the top of the doorway and slides up into the
        // ceiling, so its bottom edge rises as `open` grows.
        let top = horizon - line_h * 0.5;
        let panel_h = line_h * visible;
        let bottom = top + panel_h;
        let tex = self.tex.wall(d.tex);
        let tex_x = ((d.wall_x * tex.w as f32).clamp(0.0, tex.w as f32 - 1.0)) as usize;
        let light = fog(d.dist) * if d.side == 0 { 0.96 } else { 0.68 };
        let y0 = top.max(0.0) as usize;
        let y1 = bottom.min(VIEW_H as f32) as usize;
        for y in y0..y1 {
            let v = (y as f32 - top) / panel_h;
            let ty = ((d.open + v * visible) * tex.h as f32).clamp(0.0, tex.h as f32 - 1.0);
            let texel = tex.px[ty as usize * tex.w + tex_x];
            self.frame[y * W + x] = shade(texel, light);
        }
    }

    fn draw_sprites(&mut self, game: &Game, dir: Vec2, plane: Vec2, horizon: f32) {
        let mut sprites: Vec<Sprite> = game.sprites();
        let origin = game.player.pos;
        sprites.sort_by(|a, b| {
            b.pos
                .dist(origin)
                .partial_cmp(&a.pos.dist(origin))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let proj = VIEW_H as f32;
        let inv = 1.0 / (plane.x * dir.y - dir.x * plane.y);
        let frame = &mut self.frame;
        let zbuf = &self.zbuf;
        let texs = &self.tex.sprites;

        for s in &sprites {
            let rel = s.pos - origin;
            let tx = inv * (dir.y * rel.x - dir.x * rel.y);
            let ty = inv * (-plane.y * rel.x + plane.x * rel.y);
            if ty <= 0.08 {
                continue;
            }
            let tex = &texs[s.tex.min(texs.len() - 1)];
            let screen_x = W as f32 * 0.5 * (1.0 + tx / ty);
            let scale = proj / ty;
            let sw = s.w * scale;
            let sh = s.h * scale;
            let bottom = horizon + 0.5 * scale - s.z * scale;
            let top = bottom - sh;
            let left = screen_x - sw * 0.5;

            let x0 = left.floor().max(0.0) as usize;
            let x1 = (left + sw).ceil().min(W as f32) as usize;
            if x1 <= x0 || sw <= 0.0 {
                continue;
            }
            let y0 = top.floor().max(0.0) as usize;
            let y1 = bottom.ceil().min(VIEW_H as f32) as usize;
            if y1 <= y0 {
                continue;
            }
            let light = fog(ty);

            for x in x0..x1 {
                if ty >= zbuf[x] {
                    continue;
                }
                let u = (((x as f32 + 0.5 - left) / sw) * tex.w as f32) as usize;
                let u = u.min(tex.w - 1);
                for y in y0..y1 {
                    let v = (((y as f32 + 0.5 - top) / sh) * tex.h as f32) as usize;
                    let v = v.min(tex.h - 1);
                    let c = tex.px[v * tex.w + u];
                    if alpha(c) != 0 {
                        frame[y * W + x] = shade(c, light);
                    }
                }
            }
        }
    }

    fn draw_crosshair(&mut self, horizon: f32) {
        let cx = W as i32 / 2;
        let cy = horizon as i32;
        for i in 0..5 {
            for (dx, dy) in [(i, 0), (-i, 0), (0, i), (0, -i)] {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < VIEW_H {
                    self.frame[y as usize * W + x as usize] = 0x0098_D8A0;
                }
            }
        }
    }

    fn draw_weapon(&mut self, game: &Game) {
        let firing = game.player.muzzle > 0.0;
        let idx = game.player.weapon.tex(firing);
        let tex = &self.tex.weapons[idx];
        let sway = game.player.sway_phase().sin() * 4.0;
        let bob = game.player.bob.abs() * 1.5;
        let recoil = game.player.fire_anim * 10.0;
        let x = (W as f32 - tex.w as f32) * 0.5 + sway;
        let y = VIEW_H as f32 - tex.h as f32 + 4.0 + bob + recoil;
        blit(&mut self.frame, W, H, tex, x as i32, y as i32, 1.0);
    }

    fn draw_hud(&mut self, game: &Game) {
        let frame = &mut self.frame;
        let top = VIEW_H;
        let top_i = top as i32;
        // Metal status bar with a bevel.
        for y in top..H {
            let t = (y - top) as f32 / HUD_H as f32;
            let base = 26.0 + t * 6.0;
            for x in 0..W {
                let n = ((x * 7 + y * 13) % 5) as f32;
                let c = rgb((base + n) as u8, (base + n) as u8, (base + 2.0 + n) as u8);
                frame[y * W + x] = c;
            }
        }
        for x in 0..W {
            frame[top * W + x] = 0x0080_808A;
            frame[(top + 1) * W + x] = 0x0048_4850;
        }
        panel(frame, W, H, 10, top_i + 7, 140, 34);
        panel(frame, W, H, 168, top_i + 7, 144, 34);
        panel(frame, W, H, 330, top_i + 7, 140, 34);

        // Ammo (current weapon).
        font::draw(frame, W, H, 18, top_i + 11, "AMMO", 0x00A0_A0A8, 1);
        let ammo = game.player.ammo().to_string();
        font::draw(frame, W, H, 18, top_i + 20, &ammo, HUD_ORANGE, 3);
        font::draw(
            frame,
            W,
            H,
            96,
            top_i + 12,
            &format!("B{}", game.player.bullets),
            0x0090_9098,
            1,
        );
        font::draw(
            frame,
            W,
            H,
            96,
            top_i + 22,
            &format!("S{}", game.player.shells),
            0x0090_9098,
            1,
        );

        // Health.
        font::draw(frame, W, H, 176, top_i + 11, "HEALTH", 0x00A0_A0A8, 1);
        let health_color = if game.player.health <= 30 {
            HUD_RED
        } else {
            HUD_ORANGE
        };
        font::draw(
            frame,
            W,
            H,
            176,
            top_i + 20,
            &game.player.health.to_string(),
            health_color,
            3,
        );
        font::draw(
            frame,
            W,
            H,
            258,
            top_i + 26,
            &format!("KILLS {}", game.kills),
            0x0090_9098,
            1,
        );
        if game.total_enemies > 0 {
            font::draw(
                frame,
                W,
                H,
                258,
                top_i + 36,
                &format!("OF {}", game.total_enemies),
                0x0070_7078,
                1,
            );
        }

        // Weapon and controls.
        font::draw(frame, W, H, 340, top_i + 11, "WEAPON", 0x00A0_A0A8, 1);
        font::draw(
            frame,
            W,
            H,
            340,
            top_i + 20,
            game.player.weapon.name(),
            0x00D0_D0D8,
            2,
        );
        font::draw(
            frame,
            W,
            H,
            340,
            top_i + 37,
            "E USE  1/2 SWAP",
            0x0078_7880,
            1,
        );

        // Messages sit just above the bar.
        if let Some((text, ttl)) = game
            .messages
            .first()
            .filter(|_| game.state == GameState::Playing)
        {
            let alpha = (ttl / 0.6).min(1.0);
            let color = if alpha > 0.4 {
                0x00E8_E0B0
            } else {
                0x00A8_A080
            };
            font::draw_centered(frame, W, H, W as i32 / 2, top_i - 16, text, color, 2);
        }
    }

    fn draw_hurt_flash(&mut self, game: &Game) {
        let hurt = game.player.hurt;
        if hurt <= 0.01 {
            return;
        }
        let tint = rgb(200, 24, 16);
        for y in 0..VIEW_H {
            for x in 0..W {
                let i = y * W + x;
                self.frame[i] = over(self.frame[i], set_alpha(tint, (hurt * 190.0) as u8));
            }
        }
    }

    fn draw_minimap(&mut self, game: &Game) {
        let map = &game.map;
        let cell = 4usize;
        let mw = map.w * cell;
        let mh = map.h * cell;
        let ox = W as i32 - mw as i32 - 8;
        let oy = 8i32;
        let frame = &mut self.frame;

        for y in 0..mh + 8 {
            for x in 0..mw + 8 {
                let px = ox - 4 + x as i32;
                let py = oy - 4 + y as i32;
                if px >= 0 && py >= 0 && (px as usize) < W && (py as usize) < VIEW_H {
                    let i = py as usize * W + px as usize;
                    frame[i] = over(frame[i], 0xC010_1014);
                }
            }
        }
        for y in 0..map.h {
            for x in 0..map.w {
                let (r, g, b) = game.cell_color(map.cell(x as i32, y as i32));
                let color = rgb(r, g, b);
                for cy in 0..cell {
                    for cx in 0..cell {
                        let px = ox + x as i32 * cell as i32 + cx as i32;
                        let py = oy + y as i32 * cell as i32 + cy as i32;
                        if (px as usize) < W && (py as usize) < VIEW_H {
                            frame[py as usize * W + px as usize] = color;
                        }
                    }
                }
            }
        }
        // Pickups.
        for p in &game.pickups {
            if p.taken {
                continue;
            }
            let px = ox + (p.pos.x * cell as f32) as i32;
            let py = oy + (p.pos.y * cell as f32) as i32;
            if (px as usize) < W && (py as usize) < VIEW_H {
                frame[py as usize * W + px as usize] = rgb(230, 230, 210);
            }
        }
        // Enemies.
        for e in &game.enemies {
            if !e.alive {
                continue;
            }
            let px = ox + (e.pos.x * cell as f32) as i32 - 1;
            let py = oy + (e.pos.y * cell as f32) as i32 - 1;
            for dy in 0..2 {
                for dx in 0..2 {
                    let x = px + dx;
                    let y = py + dy;
                    if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < VIEW_H {
                        frame[y as usize * W + x as usize] = rgb(240, 60, 50);
                    }
                }
            }
        }
        // Player with a direction tick.
        let px = ox + (game.player.pos.x * cell as f32) as i32;
        let py = oy + (game.player.pos.y * cell as f32) as i32;
        let dir = Vec2::from_angle(game.player.angle);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let x = px + dx;
                let y = py + dy;
                if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < VIEW_H {
                    frame[y as usize * W + x as usize] = rgb(255, 255, 240);
                }
            }
        }
        for i in 1..7 {
            let x = px + (dir.x * i as f32) as i32;
            let y = py + (dir.y * i as f32) as i32;
            if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < VIEW_H {
                frame[y as usize * W + x as usize] = rgb(255, 220, 120);
            }
        }
    }

    fn draw_state_overlay(&mut self, game: &Game) {
        match game.state {
            GameState::Playing => {}
            GameState::Dead => {
                for i in 0..VIEW_H * W {
                    self.frame[i] = over(self.frame[i], 0xB060_0A08);
                }
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    92,
                    "YOU DIED",
                    0x00F0_4A3A,
                    4,
                );
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    140,
                    "THE DEMONS WIN THIS ROUND",
                    0x00C0_A090,
                    2,
                );
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    176,
                    "PRESS R TO RESTART",
                    0x00E0_E0D0,
                    2,
                );
            }
            GameState::Won => {
                for i in 0..VIEW_H * W {
                    self.frame[i] = over(self.frame[i], 0x9040_8040);
                }
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    80,
                    "LEVEL COMPLETE",
                    0x0080_E8A0,
                    4,
                );
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    130,
                    &format!("DEMONS SLAIN {}", game.kills),
                    0x00D8_E0C0,
                    2,
                );
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    160,
                    &format!("TIME {} SECONDS", game.time as i32),
                    0x00A0_C0A0,
                    2,
                );
                font::draw_centered(
                    &mut self.frame,
                    W,
                    H,
                    W as i32 / 2,
                    196,
                    "PRESS R TO PLAY AGAIN",
                    0x00E0_E0D0,
                    2,
                );
            }
        }
    }

    /// Nearest-neighbour upscale into the window's pixel buffer.
    pub fn blit_to(&self, dst: &mut [u32], dw: usize, dh: usize) {
        if dw == 0 || dh == 0 {
            return;
        }
        for y in 0..dh {
            let sy = (y * H / dh).min(H - 1);
            for x in 0..dw {
                let sx = (x * W / dw).min(W - 1);
                dst[y * dw + x] = self.frame[sy * W + sx];
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

/// DDA grid traversal. Doors are transparent to the ray so we can see the wall
/// behind an opening door; the first door found is reported separately.
fn cast(map: &Map, origin: Vec2, dir: Vec2) -> (Option<WallHit>, Option<DoorHit>) {
    let mut map_x = origin.x.floor() as i32;
    let mut map_y = origin.y.floor() as i32;
    let delta_x = if dir.x.abs() < 1e-6 {
        1e30
    } else {
        (1.0 / dir.x).abs()
    };
    let delta_y = if dir.y.abs() < 1e-6 {
        1e30
    } else {
        (1.0 / dir.y).abs()
    };
    let (step_x, mut side_x) = if dir.x < 0.0 {
        (-1, (origin.x - map_x as f32) * delta_x)
    } else {
        (1, (map_x as f32 + 1.0 - origin.x) * delta_x)
    };
    let (step_y, mut side_y) = if dir.y < 0.0 {
        (-1, (origin.y - map_y as f32) * delta_y)
    } else {
        (1, (map_y as f32 + 1.0 - origin.y) * delta_y)
    };

    let mut door_hit = None;

    for _ in 0..256 {
        let (side, perp) = if side_x < side_y {
            side_x += delta_x;
            map_x += step_x;
            (0u8, side_x - delta_x)
        } else {
            side_y += delta_y;
            map_y += step_y;
            (1u8, side_y - delta_y)
        };
        let cell = map.cell(map_x, map_y);
        let is_wall = matches!(cell, Cell::Wall(_));
        let is_door = matches!(cell, Cell::Door(_));
        if !is_wall && !is_door {
            continue;
        }
        let mut wall_x = if side == 0 {
            origin.y + perp * dir.y
        } else {
            origin.x + perp * dir.x
        };
        wall_x -= wall_x.floor();
        // Without this the textures come out mirrored on two of the four faces.
        if (side == 0 && dir.x < 0.0) || (side == 1 && dir.y > 0.0) {
            wall_x = 1.0 - wall_x;
        }
        match cell {
            Cell::Wall(tex) => {
                return (
                    Some(WallHit {
                        dist: perp,
                        tex,
                        side,
                        wall_x,
                    }),
                    door_hit,
                );
            }
            Cell::Door(tex) => {
                let open = map.door_open(map_x, map_y);
                if door_hit.is_none() && open < 0.98 {
                    door_hit = Some(DoorHit {
                        dist: perp,
                        tex,
                        side,
                        wall_x,
                        open,
                    });
                }
            }
            Cell::Empty => {}
        }
    }
    (
        Some(WallHit {
            dist: 64.0,
            tex: 1,
            side: 0,
            wall_x: 0.0,
        }),
        door_hit,
    )
}

fn set_alpha(c: u32, a: u8) -> u32 {
    (c & 0x00FF_FFFF) | ((a as u32) << 24)
}

/// Draw a sprite with its top-left corner at (x, y), skipping transparent pixels.
fn blit(dst: &mut [u32], dw: usize, dh: usize, tex: &Tex, x: i32, y: i32, light: f32) {
    for ty in 0..tex.h {
        for tx in 0..tex.w {
            let c = tex.px[ty * tex.w + tx];
            if alpha(c) == 0 {
                continue;
            }
            let px = x + tx as i32;
            let py = y + ty as i32;
            if px >= 0 && py >= 0 && (px as usize) < dw && (py as usize) < dh {
                dst[py as usize * dw + px as usize] = shade(c, light);
            }
        }
    }
}

/// Recessed metal panel used by the HUD.
fn panel(dst: &mut [u32], dw: usize, dh: usize, x: i32, y: i32, w: i32, h: i32) {
    let put = |dst: &mut [u32], x: i32, y: i32, c: u32| {
        if x >= 0 && y >= 0 && (x as usize) < dw && (y as usize) < dh {
            dst[y as usize * dw + x as usize] = c;
        }
    };
    for py in y..y + h {
        for px in x..x + w {
            put(dst, px, py, rgb(22, 22, 25));
        }
    }
    // Light top/left edges, dark bottom/right edges.
    for px in x..x + w {
        put(dst, px, y, rgb(58, 58, 64));
        put(dst, px, y + h - 1, rgb(12, 12, 14));
    }
    for py in y..y + h {
        put(dst, x, py, rgb(62, 62, 68));
        put(dst, x + w - 1, py, rgb(10, 10, 12));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_produces_a_frame() {
        let game = Game::new();
        let mut r = Renderer::new();
        r.render(&game);
        let non_black = r.frame.iter().filter(|&&p| p != 0).count();
        assert!(
            non_black > W * H / 2,
            "frame looks empty: {non_black} lit pixels"
        );
        // The HUD row at the very bottom is opaque.
        assert!(r.frame[(H - 1) * W + 10] != 0);
    }

    #[test]
    fn upscale_fills_the_destination() {
        let game = Game::new();
        let mut r = Renderer::new();
        r.render(&game);
        let mut dst = vec![0u32; 1920 * 1200];
        r.blit_to(&mut dst, 1920, 1200);
        assert!(dst.iter().any(|&p| p != 0));
    }
}

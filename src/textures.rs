//! Procedurally generated art: walls, floor, ceiling, billboard sprites and weapons.
//! Nothing is loaded from disk, so the game is a single self-contained binary.

use crate::font;
use crate::gfx::{Tex, rgb, shade};
use crate::math::noise;

// --- Wall texture ids (stored in the map grid) -----------------------------
pub const TEX_BRICK: u8 = 1;
pub const TEX_TECH: u8 = 2;
pub const TEX_HELL: u8 = 3;
pub const TEX_DOOR: u8 = 4;
pub const TEX_EXIT: u8 = 5;
pub const TEX_MARBLE: u8 = 6;
pub const TEX_CRATE: u8 = 7;

// --- Sprite texture ids ----------------------------------------------------
pub const SPR_IMP_WALK0: usize = 0;
pub const SPR_IMP_WALK1: usize = 1;
pub const SPR_IMP_WALK2: usize = 2;
pub const SPR_IMP_WALK3: usize = 3;
pub const SPR_IMP_ATTACK: usize = 4;
pub const SPR_IMP_PAIN: usize = 5;
pub const SPR_IMP_DIE0: usize = 6;
pub const SPR_IMP_DIE1: usize = 7;
pub const SPR_IMP_CORPSE: usize = 8;
pub const SPR_BARREL: usize = 9;
pub const SPR_MEDKIT: usize = 10;
pub const SPR_AMMO: usize = 11;
pub const SPR_SHELLS: usize = 12;
pub const SPR_SHOTGUN_PICKUP: usize = 13;
pub const SPR_FIREBALL: usize = 14;
// Second monster: the charging demon.
pub const SPR_DEMON_WALK0: usize = 15;
pub const SPR_DEMON_WALK1: usize = 16;
pub const SPR_DEMON_ATTACK: usize = 17;
pub const SPR_DEMON_PAIN: usize = 18;
pub const SPR_DEMON_CORPSE: usize = 19;
// Third monster: the hell knight.
pub const SPR_KNIGHT_WALK0: usize = 20;
pub const SPR_KNIGHT_WALK1: usize = 21;
pub const SPR_KNIGHT_ATTACK: usize = 22;
pub const SPR_KNIGHT_PAIN: usize = 23;
pub const SPR_KNIGHT_CORPSE: usize = 24;
// Projectiles and pickups.
pub const SPR_HELLFIRE: usize = 25;
pub const SPR_ROCKET: usize = 26;
pub const SPR_CHAINGUN_PICKUP: usize = 27;
pub const SPR_LAUNCHER_PICKUP: usize = 28;
pub const SPR_ROCKETS_PICKUP: usize = 29;
// Explosion frames: flash, fireball, smoke.
pub const SPR_BOOM0: usize = 30;
pub const SPR_BOOM1: usize = 31;
pub const SPR_BOOM2: usize = 32;

// The Gatekeeper.
pub const SPR_BOSS_WALK0: usize = 33;
pub const SPR_BOSS_WALK1: usize = 34;
pub const SPR_BOSS_ATTACK: usize = 35;
pub const SPR_BOSS_PAIN: usize = 36;
pub const SPR_BOSS_CORPSE: usize = 37;

pub const SPR_ARMOUR_PICKUP: usize = 38;

// --- Weapon texture ids ----------------------------------------------------
pub const WEAPON_PISTOL: usize = 0;
pub const WEAPON_PISTOL_FIRE: usize = 1;
pub const WEAPON_SHOTGUN: usize = 2;
pub const WEAPON_SHOTGUN_FIRE: usize = 3;
pub const WEAPON_CHAINGUN: usize = 4;
pub const WEAPON_CHAINGUN_FIRE: usize = 5;
pub const WEAPON_LAUNCHER: usize = 6;
pub const WEAPON_LAUNCHER_FIRE: usize = 7;

pub struct Textures {
    pub walls: Vec<Tex>,
    pub floor: Tex,
    pub ceiling: Tex,
    pub sprites: Vec<Tex>,
    pub weapons: Vec<Tex>,
}

impl Textures {
    pub fn generate() -> Self {
        let walls = vec![
            brick(),
            tech(),
            hell(),
            door(),
            exit_switch(),
            marble(),
            crate_wall(),
        ];

        let mut sprites = Vec::with_capacity(33);
        for w in 0..4 {
            sprites.push(imp_walk(w));
        }
        sprites.push(imp_attack());
        sprites.push(imp_pain());
        sprites.push(imp_die(0));
        sprites.push(imp_die(1));
        sprites.push(imp_corpse());
        sprites.push(barrel());
        sprites.push(medkit());
        sprites.push(ammo_box());
        sprites.push(shells());
        sprites.push(shotgun_pickup());
        sprites.push(fireball());
        for w in 0..2 {
            sprites.push(demon_walk(w));
        }
        sprites.push(demon_attack());
        sprites.push(demon_pain());
        sprites.push(demon_corpse());
        for w in 0..2 {
            sprites.push(knight_walk(w));
        }
        sprites.push(knight_attack());
        sprites.push(knight_pain());
        sprites.push(knight_corpse());
        sprites.push(hellfire());
        sprites.push(rocket());
        sprites.push(chaingun_pickup());
        sprites.push(launcher_pickup());
        sprites.push(rockets_pickup());
        for stage in 0..3 {
            sprites.push(explosion_frame(stage));
        }
        for w in 0..2 {
            sprites.push(boss_walk(w));
        }
        sprites.push(boss_attack());
        sprites.push(boss_pain());
        sprites.push(boss_corpse());
        sprites.push(armour_pickup());

        Self {
            walls,
            floor: floor_tex(),
            ceiling: ceiling_tex(),
            sprites,
            weapons: vec![
                pistol(false),
                pistol(true),
                shotgun_weapon(false),
                shotgun_weapon(true),
                chaingun(false),
                chaingun(true),
                launcher(false),
                launcher(true),
            ],
        }
    }

    pub fn wall(&self, id: u8) -> &Tex {
        let idx = (id.max(1) - 1) as usize;
        &self.walls[idx.min(self.walls.len() - 1)]
    }
}

fn brick() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(36, 30, 28));
    for row in 0..4 {
        let offset = if row % 2 == 0 { 0 } else { 16 };
        for col in -1..3 {
            let x = col * 32 + offset;
            let y = row * 16;
            let n = noise(col, row, 11);
            let base = rgb(104 + n / 5, 52 + n / 7, 38 + n / 9);
            t.rect(x + 1, y + 1, 30, 14, base);
            for xx in x + 1..x + 31 {
                t.set(xx, y + 1, shade(base, 1.22));
                t.set(xx, y + 14, shade(base, 0.68));
            }
            for k in 0..26 {
                let sx = x + 2 + (noise(k, row, 7) as i32 % 28);
                let sy = y + 2 + (noise(k, col, 9) as i32 % 12);
                let f = 0.82 + (noise(sx, sy, 3) as f32 / 255.0) * 0.3;
                t.set(sx, sy, shade(base, f));
            }
        }
    }
    // Grime along the bottom.
    for x in 0..64 {
        for y in 58..64 {
            let c = t.get(x as usize, y as usize);
            t.set(x, y, shade(c, 0.85 - (y as f32 - 58.0) * 0.03));
        }
    }
    t
}

fn tech() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(56, 62, 70));
    for y in 0..64 {
        for x in 0..64 {
            if noise(x, y, 3) > 205 {
                t.set(x, y, rgb(70, 77, 86));
            }
        }
    }
    for (px, py) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
        t.rect(px + 2, py + 2, 28, 28, rgb(70, 78, 88));
        for i in 0..28 {
            t.set(px + 2 + i, py + 2, rgb(88, 96, 108));
            t.set(px + 2 + i, py + 29, rgb(42, 47, 54));
        }
        for i in 0..28 {
            t.set(px + 2, py + 2 + i, rgb(84, 92, 102));
            t.set(px + 29, py + 2 + i, rgb(44, 49, 56));
        }
        for (rx, ry) in [
            (px + 6, py + 6),
            (px + 26, py + 6),
            (px + 6, py + 26),
            (px + 26, py + 26),
        ] {
            t.ellipse(rx as f32, ry as f32, 2.0, 2.0, rgb(38, 42, 48));
            t.set(rx, ry - 1, rgb(120, 130, 142));
        }
        // Panel vents.
        for i in 0..4 {
            t.rect(px + 10, py + 14 + i * 3, 12, 1, rgb(48, 53, 60));
        }
    }
    // Emergency strip light.
    t.rect(58, 4, 3, 56, rgb(22, 62, 44));
    for y in 6..58 {
        t.set(59, y, rgb(110, 235, 150));
        t.set(60, y, rgb(40, 120, 78));
    }
    t
}

fn hell() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(56, 16, 16));
    for y in 0..64 {
        for x in 0..64 {
            let n = noise(x, y, 41) as f32 / 255.0;
            let base = rgb(56, 16, 16);
            let c = shade(base, 0.7 + n * 0.75);
            t.set(x, y, c);
        }
    }
    // Veins and cartilage ridges.
    for i in 0..14 {
        let mut x = (noise(i, 0, 51) % 64) as i32;
        let mut y = 0i32;
        while y < 64 {
            let f = 0.45 + (noise(x, y, 61) as f32 / 255.0) * 0.5;
            t.set(x, y, shade(rgb(120, 34, 30), f));
            if noise(x, y, 71) > 150 {
                t.set(x + 1, y, shade(rgb(96, 26, 24), f));
            }
            y += 1;
            x += (noise(x, y, 81) as i32 % 3) - 1;
        }
    }
    // Wet highlights.
    for i in 0..60 {
        let x = (noise(i, 3, 91) % 62) as i32 + 1;
        let y = (noise(i, 7, 97) % 62) as i32 + 1;
        t.set(x, y, rgb(168, 62, 52));
    }
    t
}

fn door() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(52, 55, 60));
    for x in 0..64 {
        for y in 0..64 {
            let n = noise(x, y, 101) as f32 / 255.0;
            t.set(x, y, shade(rgb(64, 68, 74), 0.9 + n * 0.2));
        }
    }
    // Side frames.
    t.rect(0, 0, 4, 64, rgb(34, 37, 41));
    t.rect(60, 0, 4, 64, rgb(34, 37, 41));
    t.rect(4, 0, 1, 64, rgb(88, 94, 102));
    t.rect(59, 0, 1, 64, rgb(28, 31, 35));
    // Hazard stripes across the middle.
    t.rect(4, 26, 56, 12, rgb(28, 28, 30));
    let mut x = 4;
    while x < 60 {
        for k in 0..3 {
            for y in 26..38 {
                let offset = (y - 26 + k) % 8;
                if offset < 3 {
                    t.set(x + k, y, rgb(214, 176, 34));
                }
            }
        }
        x += 8;
    }
    // Central seam and rivets.
    t.rect(31, 0, 2, 64, rgb(26, 28, 32));
    for y in [8, 46, 54] {
        for x in [10, 52] {
            t.ellipse(x as f32, y as f32, 3.0, 3.0, rgb(38, 41, 46));
            t.ellipse(x as f32 - 0.5, y as f32 - 1.0, 2.0, 1.5, rgb(130, 138, 150));
            t.set(x, y + 2, rgb(24, 26, 30));
        }
    }
    // Door status light.
    t.rect(28, 3, 8, 4, rgb(30, 34, 38));
    t.rect(29, 4, 6, 2, rgb(226, 58, 46));
    t
}

fn exit_switch() -> Tex {
    let mut t = tech();
    t.rect(6, 10, 52, 44, rgb(24, 28, 30));
    t.rect(8, 12, 48, 40, rgb(18, 52, 32));
    for i in 0..48 {
        t.set(8 + i, 12, rgb(90, 220, 130));
        t.set(8 + i, 51, rgb(38, 120, 70));
    }
    for i in 0..40 {
        t.set(8, 12 + i, rgb(90, 220, 130));
        t.set(55, 12 + i, rgb(38, 120, 70));
    }
    font::draw_centered(&mut t.px, t.w, t.h, 32, 22, "EXIT", rgb(150, 255, 180), 2);
    font::draw_centered(&mut t.px, t.w, t.h, 32, 40, "OPEN", rgb(80, 180, 110), 1);
    t
}

fn marble() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(44, 50, 70));
    for y in 0..64 {
        for x in 0..64 {
            let n = noise(x, y, 111) as f32 / 255.0;
            t.set(x, y, shade(rgb(50, 56, 78), 0.85 + n * 0.35));
        }
    }
    for i in 0..10 {
        let mut x = (noise(i, 4, 121) % 64) as i32;
        let mut y = (noise(i, 8, 131) % 64) as i32;
        for _ in 0..40 {
            t.set(x, y, rgb(116, 124, 152));
            if noise(x, y, 141) > 128 {
                t.set(x + 1, y, rgb(78, 84, 106));
            }
            x += (noise(x, y, 151) as i32 % 3) - 1;
            y += (noise(x, y, 161) as i32 % 3) - 1;
            x = x.clamp(0, 63);
            y = y.clamp(0, 63);
        }
    }
    // Block joints.
    t.rect(0, 0, 64, 2, rgb(20, 22, 32));
    t.rect(0, 62, 64, 2, rgb(20, 22, 32));
    t.rect(30, 0, 2, 64, rgb(20, 22, 32));
    t
}

fn crate_wall() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(74, 50, 28));
    for i in 0..8 {
        let y = i * 8;
        let n = noise(i, 0, 171);
        let base = rgb(96 + n / 4, 64 + n / 6, 34 + n / 8);
        t.rect(2, y + 1, 60, 6, base);
        t.set(2, y + 1, shade(base, 1.3));
        t.set(2, y + 7, shade(base, 0.6));
    }
    // Metal corner brackets.
    for (x, y) in [(0, 0), (56, 0), (0, 56), (56, 56)] {
        t.rect(x, y, 8, 8, rgb(58, 58, 62));
        t.rect(x + 1, y + 1, 6, 6, rgb(78, 78, 84));
        t.ellipse((x + 4) as f32, (y + 4) as f32, 1.6, 1.6, rgb(36, 36, 40));
    }
    t.rect(0, 0, 64, 1, rgb(44, 30, 18));
    t.rect(0, 63, 64, 1, rgb(44, 30, 18));
    t
}

fn floor_tex() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(44, 44, 48));
    for y in 0..64 {
        for x in 0..64 {
            let n = noise(x, y, 201) as f32 / 255.0;
            t.set(x, y, shade(rgb(46, 46, 50), 0.85 + n * 0.3));
        }
    }
    // 32x32 plates with bevelled edges.
    for (px, py) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
        for i in 0..32 {
            t.set(px + i, py, rgb(64, 64, 70));
            t.set(px + i, py + 31, rgb(28, 28, 32));
            t.set(px, py + i, rgb(62, 62, 68));
            t.set(px + 31, py + i, rgb(30, 30, 34));
        }
        for i in 0..6 {
            t.rect(px + 4, py + 12 + i * 2, 10, 1, rgb(38, 38, 42));
        }
        t.ellipse((px + 25) as f32, (py + 7) as f32, 1.5, 1.5, rgb(30, 30, 34));
        t.set(px + 25, py + 6, rgb(84, 84, 90));
    }
    t
}

fn ceiling_tex() -> Tex {
    let mut t = Tex::filled(64, 64, rgb(38, 36, 34));
    for y in 0..64 {
        for x in 0..64 {
            let n = noise(x, y, 211) as f32 / 255.0;
            let m = noise(x / 4, y / 4, 217) as f32 / 255.0;
            t.set(x, y, shade(rgb(40, 38, 36), 0.8 + n * 0.25 + m * 0.1));
        }
    }
    t.rect(0, 0, 64, 1, rgb(26, 25, 24));
    t.rect(0, 0, 1, 64, rgb(26, 25, 24));
    t.rect(0, 63, 64, 1, rgb(48, 46, 44));
    t.rect(63, 0, 1, 64, rgb(48, 46, 44));
    t
}

// --- Enemies ---------------------------------------------------------------

fn imp_walk(frame: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let bob = [0.0, -1.0, 0.0, 1.0][frame % 4];
    let leg = [0.0, 2.0, 0.0, -2.0][frame % 4];
    let skin = rgb(136, 64, 46);
    let dark = rgb(84, 38, 28);
    let light = rgb(178, 98, 68);

    // Legs and feet.
    t.ellipse(27.0, 51.0 + leg, 5.5, 9.0, dark);
    t.ellipse(37.0, 51.0 - leg, 5.5, 9.0, dark);
    t.ellipse(26.0, 59.5, 6.0, 2.5, rgb(48, 44, 44));
    t.ellipse(38.0, 59.5, 6.0, 2.5, rgb(48, 44, 44));
    // Torso.
    t.ellipse(32.0, 39.0 + bob, 13.0, 15.0, skin);
    t.ellipse(32.0, 37.0 + bob, 8.0, 10.0, light);
    t.ellipse(32.0, 45.0 + bob, 6.0, 4.0, dark);
    // Arms.
    t.ellipse(17.5, 37.0 + bob, 5.0, 11.0, dark);
    t.ellipse(46.5, 37.0 + bob, 5.0, 11.0, dark);
    t.ellipse(16.0, 47.0 + bob, 3.5, 4.0, skin);
    t.ellipse(48.0, 47.0 + bob, 3.5, 4.0, skin);
    // Head.
    t.ellipse(32.0, 21.0 + bob, 10.0, 9.0, skin);
    t.ellipse(32.0, 24.0 + bob, 7.0, 5.0, dark);
    // Horns.
    t.line(24, 15, 19, 4, rgb(210, 198, 172));
    t.line(25, 15, 21, 5, rgb(176, 164, 140));
    t.line(40, 15, 45, 4, rgb(210, 198, 172));
    t.line(39, 15, 43, 5, rgb(176, 164, 140));
    // Eyes and teeth.
    t.rect(26, 19 + bob as i32, 4, 2, rgb(255, 206, 62));
    t.rect(35, 19 + bob as i32, 4, 2, rgb(255, 206, 62));
    t.set(27, 19 + bob as i32, rgb(90, 40, 10));
    t.set(36, 19 + bob as i32, rgb(90, 40, 10));
    for i in 0..4 {
        t.rect(28 + i * 3, 25 + bob as i32, 1, 2, rgb(232, 222, 196));
    }
    t.vertical_gradient(1.06, 0.82);
    t
}

fn imp_attack() -> Tex {
    let mut t = imp_walk(0);
    // Arms thrust forward, fireball held low so the face stays readable.
    t.ellipse(19.0, 34.0, 5.0, 9.0, rgb(76, 34, 26));
    t.ellipse(45.0, 34.0, 5.0, 9.0, rgb(76, 34, 26));
    t.ellipse(32.0, 45.0, 7.0, 7.0, rgb(255, 136, 30));
    t.ellipse(32.0, 45.0, 4.5, 4.5, rgb(255, 206, 70));
    t.ellipse(32.0, 45.0, 2.2, 2.2, rgb(255, 252, 220));
    t
}

fn imp_pain() -> Tex {
    let mut t = imp_walk(2);
    // Head thrown back, eyes wide and white.
    t.ellipse(32.0, 20.0, 10.0, 9.0, rgb(150, 76, 54));
    t.ellipse(32.0, 23.0, 6.5, 4.5, rgb(60, 20, 18));
    t.rect(26, 18, 4, 3, rgb(250, 250, 240));
    t.rect(35, 18, 4, 3, rgb(250, 250, 240));
    t.vertical_gradient(1.12, 0.9);
    t
}

fn imp_die(stage: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let (cy, ry, rx) = if stage == 0 {
        (46.0, 11.0, 15.0)
    } else {
        (53.0, 8.0, 18.0)
    };
    t.ellipse(32.0, cy, rx, ry, rgb(96, 44, 34));
    t.ellipse(32.0, cy + 2.0, rx * 0.6, ry * 0.6, rgb(72, 30, 24));
    // Head lolling.
    t.ellipse(32.0, cy - ry - 2.0, 7.0, 6.0, rgb(110, 50, 38));
    t.line(
        26,
        (cy - ry - 4.0) as i32,
        21,
        (cy - ry - 9.0) as i32,
        rgb(200, 190, 168),
    );
    t.line(
        38,
        (cy - ry - 4.0) as i32,
        43,
        (cy - ry - 9.0) as i32,
        rgb(200, 190, 168),
    );
    t.ellipse(32.0, 60.0, 22.0, 3.5, rgb(62, 14, 14));
    t.vertical_gradient(1.0, 0.85);
    t
}

fn imp_corpse() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 58.0, 24.0, 5.0, rgb(54, 12, 12));
    t.ellipse(32.0, 55.0, 18.0, 5.0, rgb(88, 38, 30));
    t.ellipse(24.0, 54.0, 8.0, 4.0, rgb(110, 48, 38));
    t.ellipse(40.0, 56.0, 7.0, 3.0, rgb(72, 28, 22));
    t.line(14, 57, 8, 58, rgb(190, 180, 160));
    t.line(50, 57, 57, 58, rgb(190, 180, 160));
    t.set(30, 54, rgb(230, 210, 180));
    t.set(33, 55, rgb(230, 210, 180));
    t
}

// --- Props and pickups -----------------------------------------------------

fn barrel() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 59.0, 14.0, 3.0, rgb(20, 22, 20));
    t.rect(19, 18, 26, 41, rgb(64, 78, 52));
    t.ellipse(32.0, 18.0, 13.0, 5.0, rgb(82, 98, 66));
    t.ellipse(32.0, 18.0, 10.0, 3.5, rgb(54, 66, 44));
    // Ribs.
    for y in [24, 36, 48] {
        t.rect(19, y, 26, 3, rgb(48, 58, 40));
        t.rect(19, y, 26, 1, rgb(96, 112, 76));
    }
    // Highlight and shadow.
    t.rect(22, 20, 3, 39, rgb(96, 112, 76));
    t.rect(41, 20, 3, 39, rgb(38, 46, 32));
    // Hazard label.
    t.rect(27, 28, 10, 8, rgb(200, 172, 40));
    t.rect(30, 29, 4, 6, rgb(40, 36, 20));
    t.vertical_gradient(1.05, 0.85);
    t
}

fn medkit() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 56.0, 18.0, 4.0, rgb(16, 16, 16));
    t.rect(16, 34, 32, 22, rgb(218, 218, 212));
    t.rect(16, 34, 32, 2, rgb(250, 250, 246));
    t.rect(16, 54, 32, 2, rgb(160, 160, 156));
    t.rect(29, 38, 6, 14, rgb(206, 40, 34));
    t.rect(25, 42, 14, 6, rgb(206, 40, 34));
    t.rect(30, 34, 4, 3, rgb(140, 140, 136));
    t
}

fn armour_pickup() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 56.0, 18.0, 4.0, rgb(16, 16, 16));
    // A green flak vest on a stand: shoulders, chest plates, straps.
    let vest = rgb(56, 122, 66);
    let shade = rgb(38, 84, 46);
    let strap = rgb(30, 30, 30);
    t.ellipse(32.0, 34.0, 17.0, 15.0, vest);
    t.rect(15, 34, 34, 20, vest);
    t.rect(15, 34, 34, 3, rgb(96, 168, 104));
    t.rect(29, 34, 6, 20, strap);
    t.rect(15, 43, 34, 3, shade);
    t.rect(15, 50, 34, 2, shade);
    t.rect(20, 37, 6, 5, shade);
    t.rect(38, 37, 6, 5, shade);
    t.rect(24, 22, 16, 5, shade);
    t
}

fn ammo_box() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 57.0, 16.0, 4.0, rgb(16, 14, 12));
    t.rect(18, 38, 28, 18, rgb(86, 66, 36));
    t.rect(18, 38, 28, 2, rgb(122, 96, 54));
    t.rect(18, 54, 28, 2, rgb(54, 40, 22));
    for i in 0..4 {
        t.rect(22 + i * 6, 42, 4, 8, rgb(198, 168, 60));
        t.rect(22 + i * 6, 42, 4, 2, rgb(238, 214, 120));
    }
    t
}

fn shells() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 56.0, 16.0, 4.0, rgb(18, 14, 12));
    for i in 0..4 {
        let x = 20 + i * 7;
        t.rect(x, 40, 5, 15, rgb(168, 42, 34));
        t.rect(x, 40, 5, 2, rgb(210, 76, 54));
        t.rect(x, 52, 5, 3, rgb(206, 172, 74));
    }
    t
}

fn shotgun_pickup() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 54.0, 26.0, 4.0, rgb(16, 14, 12));
    // Barrel.
    t.rect(10, 38, 44, 6, rgb(66, 68, 74));
    t.rect(10, 38, 44, 2, rgb(112, 116, 124));
    t.rect(10, 43, 44, 1, rgb(36, 38, 42));
    // Receiver and stock.
    t.rect(30, 44, 18, 8, rgb(78, 54, 32));
    t.rect(52, 42, 8, 10, rgb(92, 64, 38));
    t.rect(12, 44, 16, 5, rgb(96, 68, 40));
    t
}

fn fireball() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 32.0, 15.0, 15.0, rgb(188, 58, 18));
    t.ellipse(32.0, 32.0, 11.0, 11.0, rgb(244, 122, 26));
    t.ellipse(32.0, 32.0, 7.0, 7.0, rgb(255, 202, 82));
    t.ellipse(32.0, 32.0, 3.0, 3.0, rgb(255, 250, 224));
    // Ragged edges.
    for i in 0..40 {
        let a = i as f32 / 40.0 * std::f32::consts::TAU;
        let r = 14.0 + noise(i, 3, 233) as f32 / 255.0 * 5.0;
        let x = 32.0 + a.cos() * r;
        let y = 32.0 + a.sin() * r;
        t.ellipse(x, y, 2.2, 2.2, rgb(214, 82, 22));
    }
    t
}

// --- First person weapons --------------------------------------------------

fn pistol(firing: bool) -> Tex {
    let mut t = Tex::new(160, 120);
    let skin = rgb(188, 140, 108);
    let skin_dark = rgb(142, 98, 72);
    // Forearm coming up from the bottom of the screen.
    t.ellipse(92.0, 142.0, 27.0, 48.0, skin_dark);
    // Grip and hand around it.
    t.rect(70, 82, 20, 34, rgb(50, 46, 42));
    t.rect(70, 82, 4, 34, rgb(76, 70, 64));
    t.ellipse(84.0, 106.0, 17.0, 14.0, skin);
    for i in 0..3 {
        t.ellipse(69.0, 94.0 + i as f32 * 7.0, 7.0, 4.5, skin);
    }
    t.ellipse(97.0, 96.0, 6.0, 9.0, skin);
    // Slide and frame.
    t.rect(62, 48, 36, 36, rgb(62, 62, 68));
    t.rect(62, 48, 36, 4, rgb(108, 108, 116));
    t.rect(62, 80, 36, 4, rgb(34, 34, 40));
    t.rect(66, 56, 28, 4, rgb(44, 44, 50));
    // Barrel and sights.
    t.rect(74, 22, 12, 28, rgb(74, 74, 80));
    t.rect(74, 22, 3, 28, rgb(120, 120, 128));
    t.rect(83, 22, 3, 28, rgb(42, 42, 48));
    t.rect(77, 16, 6, 7, rgb(52, 52, 58));
    t.rect(79, 18, 2, 3, rgb(146, 146, 154));
    if firing {
        muzzle_flash(&mut t, 80.0, 12.0, 1.0);
    }
    t
}

fn shotgun_weapon(firing: bool) -> Tex {
    let mut t = Tex::new(160, 120);
    let skin = rgb(188, 140, 108);
    let skin_dark = rgb(142, 98, 72);
    // Forearm and grip hand.
    t.ellipse(104.0, 142.0, 26.0, 44.0, skin_dark);
    t.ellipse(94.0, 110.0, 16.0, 13.0, skin);
    // Support hand under the pump.
    t.ellipse(52.0, 114.0, 19.0, 12.0, skin_dark);
    t.ellipse(58.0, 104.0, 13.0, 10.0, skin);
    // Stock and receiver.
    t.rect(80, 94, 26, 26, rgb(84, 56, 32));
    t.rect(82, 94, 4, 26, rgb(112, 78, 44));
    t.rect(52, 66, 56, 34, rgb(58, 58, 64));
    t.rect(52, 66, 56, 4, rgb(98, 98, 106));
    t.rect(52, 96, 56, 4, rgb(32, 32, 38));
    t.rect(56, 74, 48, 3, rgb(44, 44, 50));
    // Pump with grooves.
    t.rect(42, 82, 78, 14, rgb(98, 68, 38));
    for i in 0..9 {
        t.rect(46 + i * 8, 82, 2, 14, rgb(70, 48, 28));
    }
    // Twin barrels.
    t.rect(58, 14, 23, 54, rgb(64, 64, 70));
    t.rect(89, 14, 23, 54, rgb(64, 64, 70));
    t.rect(58, 14, 5, 54, rgb(116, 116, 124));
    t.rect(89, 14, 5, 54, rgb(116, 116, 124));
    t.rect(81, 14, 8, 54, rgb(34, 34, 40));
    t.ellipse(69.5, 15.0, 11.0, 4.0, rgb(22, 22, 26));
    t.ellipse(100.5, 15.0, 11.0, 4.0, rgb(22, 22, 26));
    if firing {
        muzzle_flash(&mut t, 84.0, 8.0, 1.5);
    }
    t
}

fn explosion_frame(stage: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let c = 32.0f32;
    match stage {
        // White-hot flash with orange petals.
        0 => {
            for i in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let n = noise(i, 3, 331) as f32 / 255.0;
                let r = 7.0 + n * 7.0;
                t.ellipse(
                    c + a.cos() * r,
                    c + a.sin() * r,
                    12.0,
                    12.0,
                    rgb(238, 118, 26),
                );
            }
            t.ellipse(c, c, 17.0, 17.0, rgb(255, 168, 40));
            t.ellipse(c, c, 12.0, 12.0, rgb(255, 222, 108));
            t.ellipse(c, c, 7.0, 7.0, rgb(255, 252, 228));
        }
        // Expanding fireball, cooling from the outside in.
        1 => {
            t.ellipse(c, c, 18.0, 18.0, rgb(150, 44, 18));
            t.ellipse(c, c, 12.0, 12.0, rgb(206, 92, 26));
            for i in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let n = noise(i, 7, 337) as f32 / 255.0;
                let r = 12.0 + n * 8.0;
                t.ellipse(
                    c + a.cos() * r,
                    c + a.sin() * r,
                    11.0,
                    11.0,
                    rgb(176, 54, 20),
                );
            }
            for i in 0..14 {
                let a = i as f32 / 14.0 * std::f32::consts::TAU;
                let n = noise(i, 11, 343) as f32 / 255.0;
                let r = 8.0 + n * 8.0;
                t.ellipse(
                    c + a.cos() * r,
                    c + a.sin() * r,
                    5.5,
                    5.5,
                    rgb(226, 122, 34),
                );
            }
            for i in 0..7 {
                let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.4;
                let n = noise(i, 13, 347) as f32 / 255.0;
                let r = 3.0 + n * 5.0;
                t.ellipse(c + a.cos() * r, c + a.sin() * r, 3.5, 3.5, rgb(80, 52, 44));
            }
        }
        // Drifting soot and the last embers.
        _ => {
            t.ellipse(c, c, 16.0, 15.0, rgb(54, 50, 54));
            t.ellipse(c, c - 1.0, 10.0, 9.0, rgb(72, 68, 72));
            for i in 0..16 {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                let n = noise(i, 17, 353) as f32 / 255.0;
                let r = 8.0 + n * 18.0;
                t.ellipse(
                    c + a.cos() * r,
                    c + a.sin() * r - 2.0,
                    6.0,
                    6.0,
                    rgb(58, 55, 58),
                );
            }
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU + 1.1;
                let n = noise(i, 19, 359) as f32 / 255.0;
                let r = 4.0 + n * 14.0;
                t.ellipse(c + a.cos() * r, c + a.sin() * r, 2.0, 2.0, rgb(196, 88, 36));
            }
        }
    }
    t
}

fn muzzle_flash(t: &mut Tex, cx: f32, cy: f32, scale: f32) {
    let outer = rgb(255, 158, 32);
    let mid = rgb(255, 216, 92);
    let core = rgb(255, 252, 226);
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let r = 20.0 * scale * if i % 2 == 0 { 1.0 } else { 0.55 };
        t.line(
            cx as i32,
            cy as i32,
            (cx + a.cos() * r) as i32,
            (cy + a.sin() * r) as i32,
            outer,
        );
    }
    t.ellipse(cx, cy, 12.0 * scale, 12.0 * scale, outer);
    t.ellipse(cx, cy, 8.0 * scale, 8.0 * scale, mid);
    t.ellipse(cx, cy, 4.0 * scale, 4.0 * scale, core);
}

// --- The charging demon ----------------------------------------------------

fn demon_walk(frame: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let bob = [0.0, -1.5][frame % 2];
    let leg = [0.0, 3.0][frame % 2];
    // Charred, ash-grey hide: deliberately nothing like the brick-red imp.
    let hide = rgb(78, 64, 62);
    let dark = rgb(44, 36, 36);
    let light = rgb(108, 92, 88);
    let ember = rgb(206, 74, 34);

    // Legs and feet.
    t.ellipse(24.0, 50.0 + leg, 7.5, 10.0, dark);
    t.ellipse(40.0, 50.0 - leg, 7.5, 10.0, dark);
    t.ellipse(22.0, 60.0, 8.0, 3.0, rgb(52, 46, 44));
    t.ellipse(42.0, 60.0, 8.0, 3.0, rgb(52, 46, 44));
    // Hunched, heavy torso.
    t.ellipse(32.0, 36.0 + bob, 19.0, 16.0, hide);
    t.ellipse(32.0, 31.0 + bob, 12.0, 9.0, light);
    // Shoulders and arms.
    t.ellipse(13.0, 32.0 + bob, 8.5, 12.0, dark);
    t.ellipse(51.0, 32.0 + bob, 8.5, 12.0, dark);
    t.ellipse(12.0, 44.0 + bob, 6.0, 6.0, hide);
    t.ellipse(52.0, 44.0 + bob, 6.0, 6.0, hide);
    // Ember cracks splitting the charred chest.
    for (x, y, dx, dy) in [
        (26, 34, 3, 5),
        (32, 33, -2, 6),
        (38, 35, 4, 4),
        (23, 41, 2, 4),
    ] {
        t.line(x, y + bob as i32, x + dx, y + dy + bob as i32, ember);
    }
    // Low head with a maw of teeth.
    t.ellipse(32.0, 18.0 + bob, 12.0, 9.0, hide);
    t.ellipse(32.0, 22.0 + bob, 9.0, 5.0, rgb(56, 14, 14));
    for i in 0..5 {
        t.rect(25 + i * 3, 19 + bob as i32, 2, 4, rgb(240, 234, 212));
    }
    for i in 0..4 {
        t.rect(27 + i * 3, 26 + bob as i32, 2, 3, rgb(222, 214, 192));
    }
    // Pale, blank eyes: the imp's are yellow.
    t.rect(23, 13 + bob as i32, 4, 2, rgb(214, 234, 240));
    t.rect(37, 13 + bob as i32, 4, 2, rgb(214, 234, 240));
    // Stubby horns.
    t.line(22, 11, 16, 2, rgb(216, 204, 178));
    t.line(42, 11, 48, 2, rgb(216, 204, 178));
    t.vertical_gradient(1.08, 0.82);
    t
}

fn demon_attack() -> Tex {
    let mut t = demon_walk(0);
    // Arms reaching out with claws.
    t.ellipse(14.0, 40.0, 7.0, 9.0, rgb(56, 46, 44));
    t.ellipse(50.0, 40.0, 7.0, 9.0, rgb(56, 46, 44));
    for i in 0..3 {
        t.line(10, 46 - i * 3, 6, 50 - i * 4, rgb(244, 238, 214));
        t.line(54, 46 - i * 3, 58, 50 - i * 4, rgb(244, 238, 214));
    }
    // Maw wide open.
    t.ellipse(32.0, 22.0, 10.0, 7.0, rgb(120, 20, 18));
    for i in 0..5 {
        t.rect(25 + i * 3, 17, 2, 4, rgb(248, 244, 224));
        t.rect(26 + i * 3, 27, 2, 3, rgb(248, 244, 224));
    }
    t
}

fn demon_pain() -> Tex {
    let mut t = demon_walk(1);
    t.ellipse(32.0, 18.0, 12.0, 9.0, rgb(96, 80, 76));
    t.line(23, 11, 27, 15, rgb(255, 250, 236));
    t.line(27, 11, 23, 15, rgb(255, 250, 236));
    t.line(37, 11, 41, 15, rgb(255, 250, 236));
    t.line(41, 11, 37, 15, rgb(255, 250, 236));
    t.vertical_gradient(1.16, 0.9);
    t
}

fn demon_corpse() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 57.0, 26.0, 6.0, rgb(70, 16, 12));
    t.ellipse(32.0, 54.0, 20.0, 6.0, rgb(66, 54, 50));
    t.ellipse(22.0, 53.0, 8.0, 4.0, rgb(88, 74, 70));
    t.ellipse(44.0, 55.0, 7.0, 3.5, rgb(48, 40, 38));
    for i in 0..5 {
        t.set(24 + i * 4, 52, rgb(232, 224, 200));
    }
    t.line(12, 58, 5, 59, rgb(196, 186, 164));
    t.line(52, 58, 59, 59, rgb(196, 186, 164));
    t
}

// --- The hell knight --------------------------------------------------------

fn knight_walk(frame: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let bob = [0.0, -1.2][frame % 2];
    let leg = [0.0, 2.5][frame % 2];
    let armor = rgb(96, 108, 96);
    let dark = rgb(52, 60, 54);
    let glow = rgb(150, 240, 150);

    // Legs.
    t.ellipse(27.0, 48.0 + leg, 6.0, 13.0, dark);
    t.ellipse(37.0, 48.0 - leg, 6.0, 13.0, dark);
    t.ellipse(25.0, 60.5, 7.0, 2.5, rgb(40, 44, 42));
    t.ellipse(39.0, 60.5, 7.0, 2.5, rgb(40, 44, 42));
    // Chest with plate lines.
    t.ellipse(32.0, 33.0 + bob, 15.0, 17.0, armor);
    t.ellipse(32.0, 29.0 + bob, 9.0, 10.0, rgb(126, 138, 122));
    t.rect(22, 31 + bob as i32, 20, 2, dark);
    t.rect(23, 39 + bob as i32, 18, 2, dark);
    t.rect(24, 46 + bob as i32, 16, 2, dark);
    // Shoulder pads and arms.
    t.ellipse(14.0, 25.0 + bob, 9.0, 7.0, dark);
    t.ellipse(50.0, 25.0 + bob, 9.0, 7.0, dark);
    t.ellipse(14.0, 24.0 + bob, 6.0, 4.0, rgb(122, 134, 118));
    t.ellipse(50.0, 24.0 + bob, 6.0, 4.0, rgb(122, 134, 118));
    t.ellipse(11.0, 38.0 + bob, 5.0, 11.0, armor);
    t.ellipse(53.0, 38.0 + bob, 5.0, 11.0, armor);
    t.ellipse(11.0, 49.0 + bob, 4.0, 4.0, dark);
    t.ellipse(53.0, 49.0 + bob, 4.0, 4.0, dark);
    // Skull head with burning eyes.
    t.ellipse(32.0, 15.0 + bob, 9.0, 8.0, dark);
    t.rect(24, 14 + bob as i32, 16, 3, rgb(38, 44, 40));
    t.rect(26, 16 + bob as i32, 4, 2, glow);
    t.rect(34, 16 + bob as i32, 4, 2, glow);
    t.rect(28, 21 + bob as i32, 8, 1, rgb(20, 24, 22));
    // Horns.
    t.line(26, 8, 20, 0, rgb(206, 214, 196));
    t.line(38, 8, 44, 0, rgb(206, 214, 196));
    t.vertical_gradient(1.05, 0.88);
    t
}

fn knight_attack() -> Tex {
    let mut t = knight_walk(0);
    // Both hands cupped around green hellfire.
    t.ellipse(19.0, 36.0, 6.0, 10.0, rgb(58, 66, 58));
    t.ellipse(45.0, 36.0, 6.0, 10.0, rgb(58, 66, 58));
    t.ellipse(32.0, 42.0, 8.0, 8.0, rgb(46, 176, 92));
    t.ellipse(32.0, 42.0, 5.0, 5.0, rgb(126, 244, 150));
    t.ellipse(32.0, 42.0, 2.4, 2.4, rgb(230, 255, 236));
    t
}

fn knight_pain() -> Tex {
    let mut t = knight_walk(1);
    t.ellipse(32.0, 15.0, 9.0, 8.0, rgb(112, 122, 112));
    t.rect(25, 13, 5, 3, rgb(250, 250, 240));
    t.rect(34, 13, 5, 3, rgb(250, 250, 240));
    t.vertical_gradient(1.15, 0.9);
    t
}

fn knight_corpse() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 57.0, 24.0, 5.0, rgb(30, 40, 32));
    t.ellipse(32.0, 54.0, 18.0, 5.5, rgb(84, 96, 84));
    t.rect(20, 51, 10, 2, rgb(52, 60, 54));
    t.rect(36, 53, 9, 2, rgb(52, 60, 54));
    t.ellipse(44.0, 55.0, 6.0, 3.0, rgb(96, 108, 96));
    t.ellipse(24.0, 56.0, 3.0, 2.0, rgb(120, 220, 130));
    t.line(14, 58, 7, 59, rgb(190, 200, 184));
    t.line(50, 58, 57, 59, rgb(190, 200, 184));
    t
}

// --- The Gatekeeper ----------------------------------------------------------

fn boss_walk(frame: usize) -> Tex {
    let mut t = Tex::new(64, 64);
    let bob = [0.0, -1.5][frame % 2];
    let leg = [0.0, 3.0][frame % 2];
    // Basalt violet-grey hide, a crown of bone, and a molten heart.
    let hide = rgb(64, 52, 78);
    let dark = rgb(38, 30, 50);
    let light = rgb(98, 82, 114);
    let bone = rgb(216, 204, 178);
    let molten = rgb(255, 138, 42);
    let core = rgb(255, 196, 84);

    // Massive legs and feet.
    t.ellipse(23.0, 50.0 + leg, 8.5, 11.0, dark);
    t.ellipse(41.0, 50.0 - leg, 8.5, 11.0, dark);
    t.ellipse(21.0, 60.0, 9.5, 3.2, rgb(48, 40, 58));
    t.ellipse(43.0, 60.0, 9.5, 3.2, rgb(48, 40, 58));
    // A torso like a furnace door.
    t.ellipse(32.0, 35.0 + bob, 21.0, 16.5, hide);
    t.ellipse(32.0, 29.0 + bob, 13.0, 9.5, light);
    // The molten core, with cracks running out of it.
    t.ellipse(32.0, 36.0 + bob, 5.0, 5.5, molten);
    t.ellipse(32.0, 36.0 + bob, 2.4, 2.6, core);
    for (x, y, dx, dy) in [(27, 33, -4, -3), (37, 33, 4, -3), (28, 40, -3, 4), (36, 40, 3, 4)] {
        t.line(x, y + bob as i32, x + dx, y + dy + bob as i32, molten);
    }
    // Shoulders with bone spikes, arms like siege timbers.
    t.ellipse(11.0, 27.0 + bob, 10.0, 13.0, dark);
    t.ellipse(53.0, 27.0 + bob, 10.0, 13.0, dark);
    t.line(8, 16 + bob as i32, 3, 6, bone);
    t.line(56, 16 + bob as i32, 61, 6, bone);
    t.ellipse(9.0, 43.0 + bob, 7.0, 8.0, hide);
    t.ellipse(55.0, 43.0 + bob, 7.0, 8.0, hide);
    for i in 0..3 {
        t.line(5 + i * 3, 49 + bob as i32, 3 + i * 3, 54 + bob as i32, bone);
        t.line(53 + i * 3, 49 + bob as i32, 55 + i * 3, 54 + bob as i32, bone);
    }
    // Crowned head: three horns, hellfire eyes, a jaw full of teeth.
    t.ellipse(32.0, 14.0 + bob, 11.5, 8.5, hide);
    t.line(24, 8 + bob as i32, 18, 0, bone);
    t.line(32, 6 + bob as i32, 32, 0, bone);
    t.line(40, 8 + bob as i32, 46, 0, bone);
    t.rect(21, 11 + bob as i32, 22, 2, dark);
    t.rect(25, 13 + bob as i32, 4, 2, rgb(126, 255, 150));
    t.rect(35, 13 + bob as i32, 4, 2, rgb(126, 255, 150));
    t.ellipse(32.0, 19.0 + bob, 8.5, 4.5, rgb(52, 10, 12));
    for i in 0..5 {
        t.rect(25 + i * 3, 16 + bob as i32, 2, 3, rgb(240, 234, 212));
    }
    t.vertical_gradient(1.08, 0.82);
    t
}

fn boss_attack() -> Tex {
    let mut t = boss_walk(0);
    // Arms hurled up, the core flaring to conjure hellfire.
    t.ellipse(8.0, 18.0, 7.5, 10.0, rgb(50, 40, 64));
    t.ellipse(56.0, 18.0, 7.5, 10.0, rgb(50, 40, 64));
    for i in 0..3 {
        t.line(4 + i * 3, 10, 2 + i * 3, 4, rgb(216, 204, 178));
        t.line(54 + i * 3, 10, 56 + i * 3, 4, rgb(216, 204, 178));
    }
    t.ellipse(32.0, 36.0, 7.0, 7.5, rgb(255, 138, 42));
    t.ellipse(32.0, 36.0, 3.6, 3.8, rgb(255, 226, 150));
    // Maw wide open.
    t.ellipse(32.0, 19.0, 9.5, 5.5, rgb(96, 16, 14));
    t
}

fn boss_pain() -> Tex {
    let mut t = boss_walk(1);
    t.ellipse(32.0, 14.0, 11.5, 8.5, rgb(112, 94, 128));
    t.line(24, 10, 28, 14, rgb(255, 250, 236));
    t.line(28, 10, 24, 14, rgb(255, 250, 236));
    t.line(36, 10, 40, 14, rgb(255, 250, 236));
    t.line(40, 10, 36, 14, rgb(255, 250, 236));
    t.vertical_gradient(1.16, 0.9);
    t
}

fn boss_corpse() -> Tex {
    let mut t = Tex::new(64, 64);
    // A collapsed mountain: the core gone dark, one horn snapped beside it.
    t.ellipse(32.0, 57.0, 27.0, 6.0, rgb(70, 16, 12));
    t.ellipse(32.0, 54.0, 21.0, 6.5, rgb(52, 42, 62));
    t.ellipse(20.0, 53.0, 9.0, 4.5, rgb(74, 62, 88));
    t.ellipse(44.0, 55.0, 8.0, 4.0, rgb(38, 30, 50));
    t.ellipse(32.0, 53.0, 4.5, 3.0, rgb(110, 56, 18));
    t.line(10, 58, 3, 56, rgb(196, 186, 164));
    t.line(12, 60, 5, 61, rgb(196, 186, 164));
    for i in 0..5 {
        t.set(24 + i * 4, 51, rgb(232, 224, 200));
    }
    t
}

// --- Projectiles and weapon pickups ----------------------------------------

fn hellfire() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 32.0, 15.0, 15.0, rgb(20, 128, 58));
    t.ellipse(32.0, 32.0, 11.0, 11.0, rgb(72, 208, 104));
    t.ellipse(32.0, 32.0, 7.0, 7.0, rgb(160, 250, 176));
    t.ellipse(32.0, 32.0, 3.0, 3.0, rgb(240, 255, 242));
    for i in 0..40 {
        let a = i as f32 / 40.0 * std::f32::consts::TAU;
        let r = 14.0 + noise(i, 5, 239) as f32 / 255.0 * 5.0;
        t.ellipse(
            32.0 + a.cos() * r,
            32.0 + a.sin() * r,
            2.2,
            2.2,
            rgb(42, 170, 78),
        );
    }
    t
}

fn rocket() -> Tex {
    let mut t = Tex::new(64, 64);
    // Tail flame.
    t.ellipse(32.0, 48.0, 8.0, 11.0, rgb(210, 90, 24));
    t.ellipse(32.0, 46.0, 5.0, 8.0, rgb(255, 168, 48));
    t.ellipse(32.0, 44.0, 3.0, 5.0, rgb(255, 240, 180));
    // Body.
    t.rect(27, 22, 10, 22, rgb(180, 184, 190));
    t.rect(27, 22, 3, 22, rgb(224, 228, 234));
    t.rect(34, 22, 3, 22, rgb(120, 124, 130));
    // Nose cone.
    t.ellipse(32.0, 20.0, 5.0, 6.0, rgb(206, 60, 44));
    t.ellipse(32.0, 19.0, 3.0, 3.5, rgb(240, 120, 90));
    // Fins.
    t.line(27, 40, 20, 48, rgb(96, 100, 106));
    t.line(37, 40, 44, 48, rgb(96, 100, 106));
    t
}

fn chaingun_pickup() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 55.0, 22.0, 4.0, rgb(16, 16, 18));
    // Housing.
    t.rect(20, 40, 24, 13, rgb(66, 68, 74));
    t.rect(20, 40, 24, 2, rgb(108, 112, 120));
    // Barrel cluster.
    for i in 0..3 {
        t.rect(22 + i * 7, 30, 5, 12, rgb(88, 92, 98));
        t.rect(22 + i * 7, 30, 2, 12, rgb(132, 136, 144));
    }
    t.rect(18, 34, 28, 4, rgb(52, 54, 60));
    // Link of ammo.
    t.rect(42, 44, 10, 4, rgb(180, 150, 50));
    t.rect(42, 48, 10, 3, rgb(140, 116, 40));
    t
}

fn launcher_pickup() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 55.0, 24.0, 4.0, rgb(16, 16, 18));
    // Fat tube.
    t.rect(8, 38, 48, 12, rgb(74, 88, 66));
    t.rect(8, 38, 48, 3, rgb(112, 128, 96));
    t.rect(8, 47, 48, 3, rgb(44, 54, 40));
    // Muzzle ring and sight.
    t.ellipse(56.0, 44.0, 3.0, 7.0, rgb(52, 62, 46));
    t.rect(26, 32, 12, 6, rgb(56, 58, 62));
    t.rect(28, 30, 8, 3, rgb(96, 100, 106));
    // Grip.
    t.rect(22, 48, 8, 6, rgb(46, 44, 42));
    t
}

fn rockets_pickup() -> Tex {
    let mut t = Tex::new(64, 64);
    t.ellipse(32.0, 56.0, 18.0, 3.5, rgb(18, 16, 14));
    // Two rockets standing in a crate.
    for i in 0..2 {
        let x = 22 + i * 12;
        t.rect(x, 26, 8, 24, rgb(172, 176, 182));
        t.rect(x, 26, 3, 24, rgb(214, 218, 224));
        t.ellipse((x + 4) as f32, 24.0, 4.0, 5.0, rgb(200, 58, 44));
    }
    t.rect(16, 46, 32, 10, rgb(80, 62, 36));
    t.rect(16, 46, 32, 2, rgb(118, 92, 52));
    t.rect(16, 54, 32, 2, rgb(50, 38, 22));
    t
}

// --- First person weapons: chaingun and launcher ---------------------------

fn chaingun(firing: bool) -> Tex {
    let mut t = Tex::new(160, 120);
    let skin = rgb(188, 140, 108);
    let skin_dark = rgb(142, 98, 72);
    // Forearm and gripping hand.
    t.ellipse(96.0, 142.0, 27.0, 48.0, skin_dark);
    t.ellipse(88.0, 108.0, 17.0, 14.0, skin);
    for i in 0..3 {
        t.ellipse(72.0, 94.0 + i as f32 * 7.0, 7.0, 4.5, skin);
    }
    // Receiver.
    t.rect(58, 62, 52, 40, rgb(58, 60, 66));
    t.rect(58, 62, 52, 4, rgb(102, 106, 114));
    t.rect(58, 98, 52, 4, rgb(32, 34, 38));
    t.rect(62, 70, 44, 3, rgb(44, 46, 52));
    // Ammo drum and feed chute.
    t.ellipse(44.0, 92.0, 16.0, 16.0, rgb(70, 72, 78));
    t.ellipse(44.0, 92.0, 10.0, 10.0, rgb(50, 52, 58));
    t.rect(52, 86, 14, 6, rgb(176, 148, 50));
    // Three rotating barrels.
    for i in 0..3 {
        let x = 66 + i * 15;
        t.rect(x, 18, 12, 48, rgb(66, 68, 74));
        t.rect(x, 18, 4, 48, rgb(112, 116, 124));
        t.rect(x + 8, 18, 4, 48, rgb(38, 40, 46));
        t.ellipse((x + 6) as f32, 19.0, 6.0, 3.0, rgb(22, 22, 26));
    }
    // Barrel clamp.
    t.rect(62, 40, 52, 6, rgb(48, 50, 56));
    t.rect(62, 40, 52, 2, rgb(90, 94, 102));
    if firing {
        muzzle_flash(&mut t, 92.0, 10.0, 1.3);
    }
    t
}

fn launcher(firing: bool) -> Tex {
    let mut t = Tex::new(160, 120);
    let skin = rgb(188, 140, 108);
    let skin_dark = rgb(142, 98, 72);
    // Forearm on the grip, support hand under the tube.
    t.ellipse(104.0, 144.0, 26.0, 46.0, skin_dark);
    t.ellipse(96.0, 112.0, 16.0, 13.0, skin);
    t.ellipse(50.0, 116.0, 20.0, 12.0, skin_dark);
    t.ellipse(58.0, 106.0, 13.0, 10.0, skin);
    // Tube body.
    t.rect(30, 56, 100, 26, rgb(74, 88, 66));
    t.rect(30, 56, 100, 5, rgb(116, 132, 100));
    t.rect(30, 77, 100, 5, rgb(42, 52, 38));
    // Rear vent ring and front muzzle.
    t.rect(24, 52, 10, 34, rgb(56, 66, 50));
    t.ellipse(132.0, 69.0, 6.0, 17.0, rgb(52, 62, 46));
    t.ellipse(133.0, 69.0, 4.0, 12.0, rgb(24, 26, 24));
    // Rocket tip peeking out.
    t.ellipse(138.0, 69.0, 4.0, 5.0, rgb(196, 60, 44));
    // Sight and grip.
    t.rect(76, 44, 18, 14, rgb(56, 58, 62));
    t.rect(80, 40, 10, 5, rgb(96, 100, 106));
    t.rect(90, 80, 10, 14, rgb(52, 50, 48));
    // Heat shield ribs.
    for i in 0..4 {
        t.rect(46 + i * 14, 60, 4, 14, rgb(58, 70, 54));
    }
    if firing {
        muzzle_flash(&mut t, 140.0, 62.0, 1.6);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_ids_map_to_the_right_art() {
        let t = Textures::generate();
        assert_eq!(t.wall(TEX_BRICK).px, brick().px);
        assert_eq!(t.wall(TEX_TECH).px, tech().px);
        assert_eq!(t.wall(TEX_HELL).px, hell().px);
        assert_eq!(t.wall(TEX_DOOR).px, door().px);
        assert_eq!(t.wall(TEX_EXIT).px, exit_switch().px);
        assert_eq!(t.wall(TEX_MARBLE).px, marble().px);
        assert_eq!(t.wall(TEX_CRATE).px, crate_wall().px);
    }

    #[test]
    fn the_demon_does_not_look_like_the_imp() {
        let imp = imp_walk(0);
        let demon = demon_walk(0);
        let mean = |t: &Tex| {
            let mut n = 0.0;
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for &p in &t.px {
                if p >> 24 == 0 {
                    continue;
                }
                r += (p & 0xFF) as f32;
                g += ((p >> 8) & 0xFF) as f32;
                b += ((p >> 16) & 0xFF) as f32;
                n += 1.0;
            }
            (r / n, g / n, b / n)
        };
        let (ir, ig, ib) = mean(&imp);
        let (dr, dg, db) = mean(&demon);
        let distance = ((ir - dr).powi(2) + (ig - dg).powi(2) + (ib - db).powi(2)).sqrt();
        assert!(
            distance > 40.0,
            "demon and imp share a palette (distance {distance:.1})"
        );
    }

    #[test]
    fn explosion_frames_fade_out() {
        let flash = explosion_frame(0);
        let smoke = explosion_frame(2);
        let brightness = |t: &Tex| {
            t.px.iter()
                .filter(|&&p| p >> 24 != 0)
                .map(|p| (p & 0xFF) + ((p >> 8) & 0xFF) + ((p >> 16) & 0xFF))
                .sum::<u32>() as f32
        };
        assert!(brightness(&flash) > brightness(&smoke));
    }

    #[test]
    fn every_sprite_has_a_transparent_background() {
        let t = Textures::generate();
        assert!(t.sprites.len() > SPR_FIREBALL);
        assert!(t.weapons.len() > WEAPON_SHOTGUN_FIRE);
        for spr in &t.sprites {
            assert!(
                spr.px.iter().any(|&p| p >> 24 == 0),
                "sprite has no transparency"
            );
        }
    }
}

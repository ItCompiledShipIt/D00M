//! Headless frame capture, used for testing the renderer and the menus without
//! opening a window. Writes 24-bit BMP files so it needs no image crate.

use crate::config::Score;
use crate::game::{Difficulty, Game, Input, Weapon};
use crate::math::Vec2;
use crate::render::{H, Renderer, W};
use crate::save::SlotInfo;
use crate::ui;

pub struct ShotOptions {
    pub at: Option<Vec2>,
    pub angle: Option<f32>,
    pub steps: usize,
    pub fire: bool,
    pub show_map: bool,
    pub use_key: bool,
    pub level: usize,
    /// Equip a weapon before the frame is drawn, for checking its art.
    pub weapon: Option<usize>,
}

pub fn capture(path: &str, opts: ShotOptions) -> std::io::Result<()> {
    let mut game = Game::new(opts.level);
    if let Some(pos) = opts.at {
        game.player.pos = pos;
    }
    if let Some(angle) = opts.angle {
        game.player.angle = angle;
    }
    if let Some(index) = opts.weapon {
        let weapon = Weapon::from_index(index);
        match weapon {
            Weapon::Pistol => {}
            Weapon::Shotgun => game.player.has_shotgun = true,
            Weapon::Chaingun => game.player.has_chaingun = true,
            Weapon::Launcher => game.player.has_launcher = true,
        }
        game.player.weapon = weapon;
        game.player.bullets = 120;
        game.player.shells = 32;
        game.player.rockets = 24;
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

/// Draw one of the front-end screens into a frame, for visual checks.
pub fn capture_screen(path: &str, name: &str) -> std::io::Result<()> {
    let mut frame = vec![0u32; W * H];
    let time = 3.0;
    let settings = crate::config::load();
    match name {
        "menu" => {
            let best = crate::config::load_scores().into_iter().next();
            let mut items = Vec::new();
            if crate::save::any_saved() {
                items.push(ui::CONTINUE);
            }
            items.push(ui::NEW_GAME);
            if crate::save::any_saved() {
                items.push(ui::LOAD_GAME);
            }
            items.push("SETTINGS");
            items.push("QUIT");
            ui::title_menu(&mut frame, time, &items, 0, best.as_ref(), 0.0)
        }
        "notice" => {
            let items = [ui::CONTINUE, ui::NEW_GAME, "SETTINGS", "QUIT"];
            ui::title_menu(&mut frame, time, &items, 0, None, 0.6);
            ui::notice(
                &mut frame,
                "GAMEPAD CONNECTED: XBOX WIRELESS CONTROLLER",
                2.5,
            )
        }
        "load" => {
            let slots = [
                Some(SlotInfo {
                    level: 1,
                    level_name: "E1M2: THE FOUNDRY",
                    kills: 17,
                    time: 344.0,
                    difficulty: Difficulty::Normal,
                }),
                None,
                Some(SlotInfo {
                    level: 4,
                    level_name: "E1M5: THE INFERNO",
                    kills: 41,
                    time: 902.0,
                    difficulty: Difficulty::Hard,
                }),
            ];
            ui::load_screen(&mut frame, time, &slots, 0)
        }
        "save" => {
            let slots = [
                Some(SlotInfo {
                    level: 1,
                    level_name: "E1M2: THE FOUNDRY",
                    kills: 17,
                    time: 344.0,
                    difficulty: Difficulty::Normal,
                }),
                None,
                None,
            ];
            ui::save_screen(&mut frame, time, &slots, 1)
        }
        "settings" => ui::settings_screen(&mut frame, time, &settings, 0),
        "volume" => ui::volume_screen(&mut frame, time, &settings, 1),
        "skills" => ui::skill_screen(&mut frame, time, settings.difficulty.index()),
        "intro" => ui::intro_screen(&mut frame, 6.0),
        "pause" => {
            let mut renderer = Renderer::new();
            renderer.render(&Game::new(0));
            frame.copy_from_slice(&renderer.frame);
            ui::pause_screen(&mut frame, 0);
        }
        "banner" => {
            let mut renderer = Renderer::new();
            renderer.render(&Game::new(0));
            frame.copy_from_slice(&renderer.frame);
            ui::level_banner(&mut frame, "E1M1: ENTRYWAY", 0.6);
        }
        "intermission" => ui::intermission_screen(
            &mut frame,
            time,
            &ui::Stats {
                level_name: "E1M1: ENTRYWAY",
                kills: 9,
                total: 12,
                secrets: 1,
                secrets_total: 1,
                time: 187.0,
            },
            true,
        ),
        "episode" => {
            let run = Score {
                kills: 31,
                time: 902.0,
                difficulty: Difficulty::Normal,
            };
            let scores = [
                Score {
                    kills: 41,
                    time: 612.0,
                    difficulty: Difficulty::Hard,
                },
                Score {
                    kills: 38,
                    time: 754.0,
                    difficulty: Difficulty::Normal,
                },
                run,
                Score {
                    kills: 24,
                    time: 733.0,
                    difficulty: Difficulty::Easy,
                },
            ];
            ui::episode_complete(&mut frame, time, &run, &scores, Some(2))
        }
        other => {
            return Err(std::io::Error::other(format!(
                "unknown screen {other:?} (menu, load, save, notice, skills, settings, volume, intro, pause, banner, intermission, episode)"
            )));
        }
    }
    save_bmp(path, W, H, &frame)
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

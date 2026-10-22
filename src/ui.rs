//! Menu and overlay screens: title, intro crawl, settings, volume, pause and
//! intermission. Everything is drawn with the bitmap font and primitives.

use crate::config::Score;
use crate::font;
use crate::game::Difficulty;
use crate::gfx::{Color, blend, rgb, shade};
use crate::math::noise;
use crate::render::{H, W};

pub const NEW_GAME: &str = "NEW GAME";
pub const CONTINUE: &str = "CONTINUE";
pub const LOAD_GAME: &str = "LOAD GAME";
pub const SETTINGS_ITEMS: [&str; 6] = [
    "MOUSE SENSITIVITY",
    "STICK SENSITIVITY",
    "INVERT STICK",
    "VOLUME",
    "FULLSCREEN",
    "BACK",
];
pub const VOLUME_ITEMS: [&str; 4] = ["MASTER", "SOUND EFFECTS", "AMBIENT", "BACK"];
pub const PAUSE_ITEMS: [&str; 4] = ["RESUME", "SAVE GAME", "SETTINGS", "MAIN MENU"];

const BRIGHT: Color = rgb(255, 210, 120);
const TEXT: Color = rgb(196, 190, 178);
const DIM: Color = rgb(120, 114, 108);
const ACCENT: Color = rgb(210, 74, 30);
const PANEL: Color = rgb(20, 16, 16);

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub sensitivity: f32,
    /// Right-stick turn speed for gamepads.
    pub stick_sensitivity: f32,
    /// Push the left stick down to walk forward.
    pub invert_stick: bool,
    pub master: f32,
    pub sfx: f32,
    pub ambient: f32,
    pub fullscreen: bool,
    pub difficulty: Difficulty,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 1.0,
            stick_sensitivity: 1.0,
            invert_stick: false,
            master: 0.8,
            sfx: 0.9,
            ambient: 0.7,
            fullscreen: false,
            difficulty: Difficulty::default(),
        }
    }
}

impl Settings {
    pub const SENS_MIN: f32 = 0.3;
    pub const SENS_MAX: f32 = 2.5;

    pub fn adjust_sensitivity(&mut self, dir: f32) {
        self.sensitivity = (self.sensitivity + dir * 0.1).clamp(Self::SENS_MIN, Self::SENS_MAX);
    }

    pub fn adjust_stick_sensitivity(&mut self, dir: f32) {
        self.stick_sensitivity =
            (self.stick_sensitivity + dir * 0.1).clamp(Self::SENS_MIN, Self::SENS_MAX);
    }

    pub fn adjust_volume(&mut self, which: usize, dir: f32) {
        let value = match which {
            0 => &mut self.master,
            1 => &mut self.sfx,
            _ => &mut self.ambient,
        };
        *value = (*value + dir * 0.05).clamp(0.0, 1.0);
    }

    pub fn volume(&self, which: usize) -> f32 {
        match which {
            0 => self.master,
            1 => self.sfx,
            _ => self.ambient,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub level_name: &'static str,
    pub kills: usize,
    pub total: usize,
    pub time: f32,
}

const INTRO_LINES: [&str; 7] = [
    "MARS BASE, 2149",
    "THE UAC DRILLING PROJECT",
    "HAS GONE SILENT.",
    "YOU ARE THE LAST MARINE",
    "IN THE COMPLEX.",
    "SOMETHING CAME THROUGH THE GATE.",
    "SHUT IT.",
];

fn fill(frame: &mut [u32], x: i32, y: i32, w: i32, h: i32, c: Color) {
    for py in y..y + h {
        for px in x..x + w {
            if px >= 0 && py >= 0 && (px as usize) < W && (py as usize) < H {
                frame[py as usize * W + px as usize] = c;
            }
        }
    }
}

fn overlay(frame: &mut [u32], alpha: u8) {
    let tint = (alpha as u32) << 24;
    for p in frame.iter_mut() {
        *p = crate::gfx::over(*p, tint);
    }
}

pub fn dim(frame: &mut [u32], amount: f32) {
    overlay(frame, (amount.clamp(0.0, 1.0) * 255.0) as u8);
}

fn panel(frame: &mut [u32], x: i32, y: i32, w: i32, h: i32) {
    fill(frame, x, y, w, h, PANEL);
    fill(frame, x, y, w, 2, rgb(74, 34, 26));
    fill(frame, x, y + h - 2, w, 2, rgb(10, 8, 8));
    fill(frame, x, y, 2, h, rgb(58, 28, 22));
    fill(frame, x + w - 2, y, 2, h, rgb(8, 6, 6));
}

fn hints(frame: &mut [u32], text: &str) {
    font::draw_centered(frame, W, H, W as i32 / 2, H as i32 - 22, text, DIM, 1);
}

/// Dark, slowly flickering backdrop with rising embers.
pub fn backdrop(frame: &mut [u32], time: f32) {
    let top = rgb(28, 6, 6);
    let bottom = rgb(5, 4, 6);
    for y in 0..H {
        let t = y as f32 / (H - 1) as f32;
        let base = blend(top, bottom, t);
        for x in 0..W {
            let dx = (x as f32 - W as f32 * 0.5) / (W as f32 * 0.5);
            let dy = (y as f32 - H as f32 * 0.45) / (H as f32 * 0.55);
            let d = (dx * dx + dy * dy).sqrt();
            let f = (1.0 - d * 0.5).clamp(0.35, 1.0);
            frame[y * W + x] = shade(base, f);
        }
    }
    for i in 0..80 {
        let seed = i;
        let base_x = noise(seed, 1, 17) as f32 / 255.0 * W as f32;
        let speed = 9.0 + noise(seed, 2, 23) as f32 / 255.0 * 26.0;
        let phase = noise(seed, 3, 29) as f32 / 255.0 * H as f32;
        let y = H as f32 + 10.0 - (time * speed + phase) % (H as f32 + 20.0);
        let drift = (time * 0.8 + i as f32 * 0.7).sin() * 7.0;
        let flicker = 0.45 + 0.55 * (time * 5.0 + i as f32 * 1.9).sin().abs();
        let x = (base_x + drift) as i32;
        let y = y as i32;
        if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
            let c = shade(rgb(255, 120, 38), flicker);
            frame[y as usize * W + x as usize] = c;
            if flicker > 0.85 && (x + 1) < W as i32 {
                frame[y as usize * W + x as usize + 1] = shade(c, 0.55);
            }
        }
    }
}

pub fn title_screen(frame: &mut [u32], time: f32) {
    backdrop(frame, time);
    let logo = "D00M";
    let scale = 9;
    let width = font::gradient_width(logo, scale) as i32;
    let x = (W as i32 - width) / 2;
    let y = 40;
    for (dx, dy) in [(-3, 0), (3, 0), (0, 3), (2, 3), (-2, 3)] {
        font::draw_gradient(
            frame,
            W,
            H,
            x + dx,
            y + dy,
            logo,
            rgb(120, 22, 10),
            rgb(50, 8, 6),
            scale,
        );
    }
    font::draw_gradient(
        frame,
        W,
        H,
        x,
        y,
        logo,
        rgb(255, 226, 150),
        rgb(184, 46, 18),
        scale,
    );
    fill(frame, x, y + 72, width, 2, rgb(120, 30, 16));
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        128,
        "THE GATES OF HELL ARE OPEN",
        rgb(186, 122, 94),
        1,
    );
}

pub fn menu(frame: &mut [u32], items: &[&str], selected: usize, top: i32, step: i32) {
    for (i, item) in items.iter().enumerate() {
        let y = top + i as i32 * step;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 106, y - 5, 268, 22, rgb(58, 14, 10));
            fill(frame, 106, y - 5, 3, 22, ACCENT);
            font::draw(frame, W, H, 118, y, ">", BRIGHT, 2);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw_centered(frame, W, H, W as i32 / 2, y, item, color, 2);
    }
}

pub fn title_menu(
    frame: &mut [u32],
    time: f32,
    items: &[&str],
    selected: usize,
    best: Option<&Score>,
    quit_progress: f32,
) {
    title_screen(frame, time);
    if let Some(best) = best {
        let text = format!(
            "BEST RUN   {} KILLS   {}   {}",
            best.kills,
            clock(best.time),
            best.difficulty.short()
        );
        font::draw_centered(frame, W, H, W as i32 / 2, 141, &text, rgb(170, 118, 78), 1);
    }
    // Five entries (CONTINUE .. QUIT) need tighter rows to clear the hints.
    let (top, step) = if items.len() >= 5 {
        (160, 22)
    } else {
        (168, 28)
    };
    menu(frame, items, selected, top, step);
    if quit_progress > 0.0 {
        // Fill a bar under the selected row while QUIT (or Esc) is held.
        let width = 160;
        let x = (W as i32 - width) / 2;
        let y = top + selected as i32 * step + 17;
        fill(frame, x, y, width, 3, rgb(44, 20, 16));
        let filled = (width as f32 * quit_progress.clamp(0.0, 1.0)) as i32;
        fill(frame, x, y, filled, 3, ACCENT);
    }
    hints(frame, "ARROWS SELECT   ENTER CONFIRM   HOLD TO QUIT");
}

/// Small banner for transient messages such as a controller being plugged in.
pub fn notice(frame: &mut [u32], text: &str, remaining: f32) {
    if remaining <= 0.0 {
        return;
    }
    let fade = (remaining.min(0.5) / 0.5).clamp(0.0, 1.0);
    let alpha = (150.0 * fade) as u8;
    let width = font::text_width(text, 1) as i32 + 20;
    let x = (W as i32 - width) / 2;
    let y = 11;
    for py in y - 5..y + 13 {
        for px in x..x + width {
            if px >= 0 && py >= 0 && (px as usize) < W && (py as usize) < H {
                let i = py as usize * W + px as usize;
                frame[i] = crate::gfx::over(frame[i], (alpha as u32) << 24);
            }
        }
    }
    let color = blend(rgb(60, 44, 36), rgb(240, 214, 150), fade);
    font::draw_centered(frame, W, H, W as i32 / 2, y, text, color, 1);
}

fn screen_title(frame: &mut [u32], text: &str) {
    font::draw_centered(frame, W, H, W as i32 / 2, 26, text, BRIGHT, 3);
    fill(frame, 120, 60, 240, 2, rgb(94, 32, 22));
}

pub fn settings_screen(frame: &mut [u32], time: f32, settings: &Settings, selected: usize) {
    backdrop(frame, time);
    screen_title(frame, "SETTINGS");
    for (i, item) in SETTINGS_ITEMS.iter().enumerate() {
        let y = 84 + i as i32 * 32;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 54, y - 6, 372, 24, rgb(50, 14, 10));
            fill(frame, 54, y - 6, 3, 24, ACCENT);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw(frame, W, H, 68, y, item, color, 2);
        match i {
            0 => right_text(frame, y, &format!("{:.1}X", settings.sensitivity), color),
            1 => right_text(
                frame,
                y,
                &format!("{:.1}X", settings.stick_sensitivity),
                color,
            ),
            2 => right_text(
                frame,
                y,
                if settings.invert_stick { "ON" } else { "OFF" },
                color,
            ),
            3 => right_text(frame, y, "ENTER", color),
            4 => right_text(
                frame,
                y,
                if settings.fullscreen { "ON" } else { "OFF" },
                color,
            ),
            _ => {}
        }
        if is_selected && matches!(i, 0 | 1 | 2 | 4) {
            font::draw(frame, W, H, 336, y, "<", DIM, 2);
            font::draw(frame, W, H, 452, y, ">", DIM, 2);
        }
    }
    hints(frame, "ARROWS ADJUST   ENTER SELECT   ESC BACK");
}

pub fn volume_screen(frame: &mut [u32], time: f32, settings: &Settings, selected: usize) {
    backdrop(frame, time);
    screen_title(frame, "VOLUME");
    for (i, item) in VOLUME_ITEMS.iter().enumerate() {
        let y = 88 + i as i32 * 40;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 54, y - 6, 372, 28, rgb(50, 14, 10));
            fill(frame, 54, y - 6, 3, 28, ACCENT);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw(frame, W, H, 68, y, item, color, 2);
        if i < 3 {
            let value = settings.volume(i);
            draw_bar(frame, 272, y + 2, 112, value);
            right_text(
                frame,
                y,
                &format!("{}%", (value * 100.0).round() as i32),
                color,
            );
        }
    }
    hints(frame, "ARROWS ADJUST   ESC BACK");
}

fn draw_bar(frame: &mut [u32], x: i32, y: i32, width: i32, value: f32) {
    let segments = 10;
    let gap = 2;
    let seg_w = (width - gap * (segments - 1)) / segments;
    let filled = (value * segments as f32).round() as i32;
    for i in 0..segments {
        let sx = x + i * (seg_w + gap);
        let c = if i < filled {
            if i >= 8 {
                rgb(220, 90, 40)
            } else {
                rgb(190, 160, 60)
            }
        } else {
            rgb(44, 38, 36)
        };
        fill(frame, sx, y, seg_w, 12, c);
    }
    fill(frame, x, y + 12, width, 1, rgb(10, 8, 8));
}

fn right_text(frame: &mut [u32], y: i32, text: &str, color: Color) {
    let w = font::text_width(text, 2) as i32;
    font::draw(frame, W, H, 448 - w, y, text, color, 2);
}

pub fn intro_screen(frame: &mut [u32], elapsed: f32) {
    backdrop(frame, elapsed);
    let start_y = 96;
    for (i, line) in INTRO_LINES.iter().enumerate() {
        let appear = ((elapsed - (1.0 + i as f32 * 1.6)) / 0.7).clamp(0.0, 1.0);
        if appear <= 0.0 {
            continue;
        }
        let y = start_y + i as i32 * 26;
        if i == 0 {
            let color = blend(rgb(8, 6, 8), rgb(255, 206, 120), appear);
            font::draw_centered(frame, W, H, W as i32 / 2, y, line, color, 3);
        } else {
            let color = blend(rgb(8, 6, 8), rgb(214, 140, 98), appear);
            font::draw_centered(frame, W, H, W as i32 / 2, y, line, color, 2);
        }
    }
    if elapsed > 1.2 {
        let pulse = 0.55 + 0.45 * (elapsed * 3.0).sin().abs();
        let color = blend(rgb(20, 16, 16), rgb(190, 180, 160), pulse);
        font::draw_centered(
            frame,
            W,
            H,
            W as i32 / 2,
            H as i32 - 26,
            "PRESS ENTER TO SKIP",
            color,
            1,
        );
    }
}

/// Doom-style skill selection, shown when a new game starts.
pub fn skill_screen(frame: &mut [u32], time: f32, selected: usize) {
    backdrop(frame, time);
    screen_title(frame, "CHOOSE SKILL");
    for (i, skill) in Difficulty::ALL.iter().enumerate() {
        let y = 96 + i as i32 * 48;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 54, y - 7, 372, 36, rgb(50, 14, 10));
            fill(frame, 54, y - 7, 3, 36, ACCENT);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw(frame, W, H, 72, y, skill.short(), color, 2);
        let flavor = if is_selected {
            blend(DIM, BRIGHT, 0.55)
        } else {
            DIM
        };
        font::draw(frame, W, H, 188, y + 3, skill.name(), flavor, 1);
    }
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        240,
        "DEMONS HIT HARDER AND MOVE FASTER AS IT RISES",
        DIM,
        1,
    );
    hints(frame, "ARROWS SELECT   ENTER CONFIRM   ESC BACK");
}

fn clock(seconds: f32) -> String {
    let total = seconds.round().max(0.0) as u32;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Three save slots, each showing where the run is and how it is going.
pub fn load_screen(
    frame: &mut [u32],
    time: f32,
    slots: &[Option<crate::save::SlotInfo>],
    selected: usize,
) {
    slot_screen(
        frame,
        time,
        "LOAD GAME",
        slots,
        selected,
        "ARROWS SELECT   ENTER LOAD   ESC BACK",
    );
}

/// Same list, used to write the paused run into a slot of your choice.
pub fn save_screen(
    frame: &mut [u32],
    time: f32,
    slots: &[Option<crate::save::SlotInfo>],
    selected: usize,
) {
    slot_screen(
        frame,
        time,
        "SAVE GAME",
        slots,
        selected,
        "ARROWS SELECT   ENTER SAVE   ESC BACK",
    );
}

fn slot_screen(
    frame: &mut [u32],
    time: f32,
    title: &str,
    slots: &[Option<crate::save::SlotInfo>],
    selected: usize,
    hint_line: &str,
) {
    backdrop(frame, time);
    screen_title(frame, title);
    for (i, slot) in slots.iter().enumerate() {
        let y = 94 + i as i32 * 48;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 54, y - 8, 372, 42, rgb(50, 14, 10));
            fill(frame, 54, y - 8, 3, 42, ACCENT);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw(frame, W, H, 68, y, &format!("SLOT {}", i + 1), color, 2);
        match slot {
            Some(info) => {
                font::draw(frame, W, H, 180, y, info.level_name, color, 1);
                font::draw(
                    frame,
                    W,
                    H,
                    180,
                    y + 12,
                    &format!(
                        "{} KILLS   {}   {}",
                        info.kills,
                        clock(info.time),
                        info.difficulty.short()
                    ),
                    DIM,
                    1,
                );
            }
            None => font::draw(frame, W, H, 180, y + 6, "EMPTY", DIM, 1),
        }
    }
    hints(frame, hint_line);
}

pub fn pause_screen(frame: &mut [u32], selected: usize) {
    dim(frame, 0.62);
    panel(frame, 100, 50, 280, 196);
    font::draw_centered(frame, W, H, W as i32 / 2, 62, "PAUSED", BRIGHT, 3);
    fill(frame, 150, 90, 180, 2, rgb(74, 30, 22));
    for (i, item) in PAUSE_ITEMS.iter().enumerate() {
        let y = 108 + i as i32 * 32;
        let is_selected = i == selected;
        if is_selected {
            fill(frame, 130, y - 5, 220, 22, rgb(58, 14, 10));
            fill(frame, 130, y - 5, 3, 22, ACCENT);
        }
        let color = if is_selected { BRIGHT } else { TEXT };
        font::draw_centered(frame, W, H, W as i32 / 2, y, item, color, 2);
    }
    font::draw_centered(frame, W, H, W as i32 / 2, 226, "ESC RESUME", DIM, 1);
}

pub fn intermission_screen(frame: &mut [u32], time: f32, stats: &Stats, has_next: bool) {
    backdrop(frame, time);
    font::draw_centered(frame, W, H, W as i32 / 2, 46, "LEVEL COMPLETE", BRIGHT, 4);
    fill(frame, 100, 92, 280, 2, rgb(94, 32, 22));
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        108,
        stats.level_name,
        rgb(220, 130, 80),
        2,
    );
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        158,
        &format!("DEMONS SLAIN   {} / {}", stats.kills, stats.total),
        TEXT,
        1,
    );
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        178,
        &format!("TIME   {} SECONDS", stats.time.round() as i32),
        TEXT,
        1,
    );
    let prompt = if has_next {
        "PRESS ENTER TO CONTINUE"
    } else {
        "PRESS ENTER FOR THE FINAL SCREEN"
    };
    font::draw_centered(frame, W, H, W as i32 / 2, 228, prompt, BRIGHT, 2);
    font::draw_centered(frame, W, H, W as i32 / 2, 258, "ESC FOR MAIN MENU", DIM, 1);
}

/// Final screen: this run's totals and the best runs so far, with the run that
/// just finished highlighted.
pub fn episode_complete(
    frame: &mut [u32],
    time: f32,
    run: &Score,
    scores: &[Score],
    highlight: Option<usize>,
) {
    backdrop(frame, time);
    font::draw_centered(frame, W, H, W as i32 / 2, 22, "EPISODE COMPLETE", BRIGHT, 3);
    fill(frame, 90, 52, 300, 2, rgb(94, 32, 22));
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        62,
        "THE GATES ARE SEALED",
        rgb(150, 210, 150),
        1,
    );
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        82,
        &format!(
            "THIS RUN   {} KILLS   {}   {}",
            run.kills,
            clock(run.time),
            run.difficulty.short()
        ),
        rgb(220, 150, 100),
        1,
    );

    font::draw(frame, W, H, 118, 110, "RANK", DIM, 1);
    font::draw(frame, W, H, 196, 110, "KILLS", DIM, 1);
    font::draw(frame, W, H, 268, 110, "TIME", DIM, 1);
    font::draw(frame, W, H, 344, 110, "SKILL", DIM, 1);

    if scores.is_empty() {
        font::draw_centered(frame, W, H, W as i32 / 2, 140, "NO RUNS RECORDED", DIM, 1);
    }
    for (i, score) in scores.iter().enumerate() {
        let y = 128 + i as i32 * 20;
        let is_new = highlight == Some(i);
        let color = if is_new { BRIGHT } else { TEXT };
        if is_new {
            fill(frame, 108, y - 5, 272, 16, rgb(58, 14, 10));
            fill(frame, 108, y - 5, 3, 16, ACCENT);
        }
        font::draw(frame, W, H, 118, y, &format!("{}.", i + 1), color, 1);
        let kills = format!("{}", score.kills);
        let kw = font::text_width(&kills, 1) as i32;
        font::draw(frame, W, H, 226 - kw, y, &kills, color, 1);
        let time_text = clock(score.time);
        let tw = font::text_width(&time_text, 1) as i32;
        font::draw(frame, W, H, 296 - tw, y, &time_text, color, 1);
        font::draw(frame, W, H, 344, y, score.difficulty.short(), color, 1);
        if is_new {
            font::draw(frame, W, H, 388, y, "NEW", ACCENT, 1);
        }
    }

    let note = match highlight {
        Some(0) => "A NEW RECORD!",
        Some(_) => "YOU MADE THE TABLE",
        None => "",
    };
    if !note.is_empty() {
        font::draw_centered(frame, W, H, W as i32 / 2, 232, note, ACCENT, 1);
    }
    font::draw_centered(
        frame,
        W,
        H,
        W as i32 / 2,
        256,
        "PRESS ENTER FOR MAIN MENU",
        BRIGHT,
        2,
    );
}

/// Level name plaque shown for a few seconds after a level starts.
pub fn level_banner(frame: &mut [u32], name: &str, remaining: f32) {
    if remaining <= 0.0 {
        return;
    }
    let fade = (remaining.min(0.8) / 0.8).clamp(0.0, 1.0);
    let alpha = (150.0 * fade) as u8;
    for y in 16..54 {
        for x in 60..(W as i32 - 60) {
            let i = y as usize * W + x as usize;
            frame[i] = crate::gfx::over(frame[i], (alpha as u32) << 24);
        }
    }
    let color = blend(rgb(40, 20, 16), rgb(240, 208, 140), fade);
    font::draw_centered(frame, W, H, W as i32 / 2, 28, name, color, 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(frame: &[u32]) -> usize {
        frame.iter().filter(|&&p| p & 0x00FF_FFFF != 0).count()
    }

    #[test]
    fn every_screen_draws_something() {
        let mut frame = vec![0u32; W * H];
        let items = [CONTINUE, NEW_GAME, "SETTINGS", "QUIT"];
        title_menu(&mut frame, 1.0, &items, 0, None, 0.0);
        assert!(lit(&frame) > W * H / 3);

        // Without a save the menu is one entry shorter.
        frame.fill(0);
        let short = [NEW_GAME, "SETTINGS", "QUIT"];
        title_menu(&mut frame, 1.0, &short, 2, None, 0.0);
        assert!(lit(&frame) > W * H / 3);

        // With a best run the title screen still fills the frame.
        frame.fill(0);
        let best = Score {
            kills: 40,
            time: 611.0,
            difficulty: Difficulty::Hard,
        };
        title_menu(&mut frame, 1.0, &items, 0, Some(&best), 0.0);
        assert!(lit(&frame) > W * H / 3);

        // Five entries (with LOAD GAME) and a quit bar in progress.
        let full = [CONTINUE, NEW_GAME, LOAD_GAME, "SETTINGS", "QUIT"];
        frame.fill(0);
        title_menu(&mut frame, 1.0, &full, 4, Some(&best), 0.5);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        settings_screen(&mut frame, 1.0, &Settings::default(), 0);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        volume_screen(&mut frame, 1.0, &Settings::default(), 1);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        intro_screen(&mut frame, 3.0);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        intermission_screen(
            &mut frame,
            1.0,
            &Stats {
                level_name: "E1M1",
                kills: 3,
                total: 5,
                time: 42.0,
            },
            true,
        );
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        skill_screen(&mut frame, 1.0, 1);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        let slots = [
            Some(crate::save::SlotInfo {
                level: 0,
                level_name: "E1M1: ENTRYWAY",
                kills: 5,
                time: 120.0,
                difficulty: Difficulty::Easy,
            }),
            None,
            None,
        ];
        load_screen(&mut frame, 1.0, &slots, 1);
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        save_screen(&mut frame, 1.0, &slots, 0);
        assert!(lit(&frame) > W * H / 3);

        let run = Score {
            kills: 20,
            time: 200.0,
            difficulty: Difficulty::Normal,
        };
        frame.fill(0);
        episode_complete(&mut frame, 1.0, &run, &[run], Some(0));
        assert!(lit(&frame) > W * H / 3);

        frame.fill(0);
        pause_screen(&mut frame, 1);
        assert!(lit(&frame) > W * H / 3);
    }

    #[test]
    fn the_score_table_shows_every_run() {
        let run = Score {
            kills: 12,
            time: 65.0,
            difficulty: Difficulty::Hard,
        };
        let scores: Vec<Score> = (0..crate::config::MAX_SCORES)
            .map(|i| Score {
                kills: 40 - i * 3,
                time: 300.0 + i as f32 * 30.0,
                difficulty: Difficulty::Normal,
            })
            .collect();
        let mut frame = vec![0u32; W * H];
        episode_complete(&mut frame, 1.0, &run, &scores, None);
        assert!(lit(&frame) > W * H / 3);
        // Rows are drawn inside the frame, never past the bottom edge.
        assert!(128 + (scores.len() as i32 - 1) * 20 < H as i32 - 20);
    }

    #[test]
    fn settings_clamp() {
        let mut s = Settings::default();
        for _ in 0..100 {
            s.adjust_sensitivity(1.0);
        }
        assert!(s.sensitivity <= Settings::SENS_MAX);
        for _ in 0..100 {
            s.adjust_volume(0, -1.0);
        }
        assert_eq!(s.master, 0.0);
        for _ in 0..100 {
            s.adjust_volume(2, 1.0);
        }
        assert_eq!(s.ambient, 1.0);
    }
}

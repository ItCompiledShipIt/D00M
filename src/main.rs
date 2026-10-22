//! D00M: a from-scratch Doom-style first person shooter.
//!
//! Rendering is a software raycaster (no GPU); all art and sound are generated
//! at start-up. The front end is a small screen state machine: title menu,
//! intro crawl, gameplay, pause, settings, volume, intermission and episode
//! complete screens. Sensitivity and volume are saved to the user's config
//! directory as they change. Keyboard, mouse and an optional gamepad all feed
//! the same input handling.

mod audio;
mod config;
mod font;
mod game;
mod gfx;
mod levels;
mod map;
mod math;
mod pad;
mod render;
mod save;
mod shot;
mod textures;
mod ui;

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use audio::{Audio, SoundId, SoundRequest};
use game::{Difficulty, Game, GameState, Input};
use levels::LEVELS;
use render::{H, Renderer, W};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

const WINDOW_SCALE: f64 = 2.0;
const INTRO_SECONDS: f32 = 16.0;
const NOTICE_SECONDS: f32 = 3.5;
/// How long QUIT has to be held before the game closes.
const QUIT_HOLD_SECONDS: f32 = 1.0;
/// How fast a fully deflected right stick turns, in mouse-pixel equivalents.
const PAD_TURN_RATE: f32 = 1000.0;

#[derive(Clone, Copy, Debug)]
enum Screen {
    Menu,
    Load,
    Save,
    Skills,
    Intro { elapsed: f32 },
    Settings,
    Volume,
    Playing,
    Paused,
    Intermission,
    EpisodeComplete,
}

fn wrap(index: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    (index as i32 + delta).rem_euclid(len as i32) as usize
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "D00M\n\n  cargo run --release            play\n  cargo run --release -- --shot frame.bmp [--at X,Y,ANGLE] [--level N] [--steps N] [--fire] [--map] [--use]\n  cargo run --release -- --shot frame.bmp --screen menu|load|save|notice|skills|settings|volume|intro|pause|intermission|episode\n\n\
             Controls:\n  Menus: arrows/W S select, enter confirm, esc back\n  WASD move / strafe, mouse or arrow keys turn, Shift run\n  Left mouse / Space fire, E use, 1-2 weapons, Tab map\n  Esc pause, R restart level"
        );
        return Ok(());
    }

    if let Some(i) = args.iter().position(|a| a == "--shot") {
        let path = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| "frame.bmp".into());
        if let Some(j) = args.iter().position(|a| a == "--screen")
            && let Some(name) = args.get(j + 1)
        {
            shot::capture_screen(&path, name)?;
            println!("wrote {path} (screen {name})");
            return Ok(());
        }
        let mut opts = shot::ShotOptions {
            at: None,
            angle: None,
            steps: 20,
            fire: false,
            show_map: false,
            use_key: false,
            level: 0,
        };
        if let Some(j) = args.iter().position(|a| a == "--at")
            && let Some(spec) = args.get(j + 1)
        {
            let parts: Vec<f32> = spec
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if parts.len() >= 2 {
                opts.at = Some(math::Vec2::new(parts[0], parts[1]));
                opts.angle = parts.get(2).copied();
            }
        }
        if let Some(n) = args
            .iter()
            .position(|a| a == "--steps")
            .and_then(|j| args.get(j + 1))
            .and_then(|n| n.parse().ok())
        {
            opts.steps = n;
        }
        if let Some(level) = args
            .iter()
            .position(|a| a == "--level")
            .and_then(|j| args.get(j + 1))
            .and_then(|n| n.parse().ok())
        {
            opts.level = level;
        }
        opts.fire = args.iter().any(|a| a == "--fire");
        opts.show_map = args.iter().any(|a| a == "--map");
        opts.use_key = args.iter().any(|a| a == "--use");
        shot::capture(&path, opts)?;
        println!("wrote {path} ({W}x{H})");
        return Ok(());
    }

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    renderer: Renderer,
    game: Game,
    audio: Option<Audio>,
    screen: Screen,
    menu_index: usize,
    settings_index: usize,
    volume_index: usize,
    pause_index: usize,
    settings_from_pause: bool,
    settings: ui::Settings,
    skill_index: usize,
    scores: Vec<config::Score>,
    run_score: config::Score,
    run_rank: Option<usize>,
    level: usize,
    level_banner: f32,
    stats: ui::Stats,
    total_kills: usize,
    total_time: f32,
    anim_time: f32,
    keys: HashSet<KeyCode>,
    pad: Option<pad::Pad>,
    pad_motion: pad::Motion,
    pad_notice: Option<(String, f32)>,
    has_saves: bool,
    current_slot: usize,
    load_index: usize,
    load_slots: Vec<Option<save::SlotInfo>>,
    quit_hold: f32,
    quit_requested: bool,
    input: Input,
    mouse_fire: bool,
    grabbed: bool,
    last_frame: Instant,
    last_render: Instant,
}

impl App {
    fn new() -> Self {
        let settings = config::load();
        let audio = Audio::start();
        if let Some(audio) = &audio {
            audio.set_volumes(settings.master, settings.sfx, settings.ambient);
            audio.set_ambient_gain(0.0);
        }
        Self {
            window: None,
            context: None,
            surface: None,
            renderer: Renderer::new(),
            game: Game::new(0),
            audio,
            screen: Screen::Menu,
            menu_index: 0,
            settings_index: 0,
            volume_index: 0,
            pause_index: 0,
            settings_from_pause: false,
            settings,
            skill_index: settings.difficulty.index(),
            scores: config::load_scores(),
            run_score: config::Score {
                kills: 0,
                time: 0.0,
                difficulty: settings.difficulty,
            },
            run_rank: None,
            level: 0,
            level_banner: 0.0,
            stats: ui::Stats::default(),
            total_kills: 0,
            total_time: 0.0,
            anim_time: 0.0,
            keys: HashSet::new(),
            pad: pad::Pad::open(),
            pad_motion: pad::Motion::default(),
            pad_notice: None,
            has_saves: save::any_saved(),
            current_slot: save::next_slot(),
            load_index: 0,
            load_slots: Vec::new(),
            quit_hold: 0.0,
            quit_requested: false,
            input: Input::default(),
            mouse_fire: false,
            grabbed: false,
            last_frame: Instant::now(),
            last_render: Instant::now(),
        }
    }

    fn grab_pointer(window: &Window) {
        if window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
            .is_ok()
        {
            window.set_cursor_visible(false);
        }
    }

    fn ui_sound(&self, id: SoundId) {
        if let Some(audio) = &self.audio {
            audio.play(SoundRequest {
                id,
                gain: 0.55,
                pan: 0.0,
            });
        }
    }

    fn set_invert_stick(&mut self, on: bool) {
        if self.settings.invert_stick == on {
            return;
        }
        self.settings.invert_stick = on;
        config::save(&self.settings);
    }

    fn set_fullscreen(&mut self, on: bool) {
        if self.settings.fullscreen == on {
            return;
        }
        self.settings.fullscreen = on;
        self.apply_fullscreen();
        config::save(&self.settings);
    }

    fn apply_fullscreen(&self) {
        if let Some(window) = &self.window {
            let mode = if self.settings.fullscreen {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            };
            window.set_fullscreen(mode);
        }
    }

    fn push_volumes(&self) {
        if let Some(audio) = &self.audio {
            audio.set_volumes(
                self.settings.master,
                self.settings.sfx,
                self.settings.ambient,
            );
        }
        config::save(&self.settings);
    }

    fn set_screen(&mut self, screen: Screen) {
        self.quit_hold = 0.0;
        self.keys.clear();
        self.input = Input::default();
        self.mouse_fire = false;
        let duck = match screen {
            Screen::Playing => 0.55,
            Screen::Paused => 0.18,
            Screen::Intermission => 0.22,
            Screen::Volume => 0.3,
            _ => 0.0,
        };
        if let Some(audio) = &self.audio {
            audio.set_ambient_gain(duck);
        }
        self.screen = screen;
    }

    fn start_game(&mut self, level: usize) {
        self.level = level.min(LEVELS.len() - 1);
        self.game = Game::with_difficulty(self.level, self.settings.difficulty);
        self.level_banner = 2.6;
        self.set_screen(Screen::Playing);
        // A save at the top of every level means a crash never loses the run.
        self.store_run();
    }

    fn begin_intro(&mut self) {
        self.total_kills = 0;
        self.total_time = 0.0;
        self.current_slot = save::next_slot();
        self.set_screen(Screen::Intro { elapsed: 0.0 });
    }

    fn show_menu(&mut self) {
        self.store_run();
        // Dying does not overwrite the save, but a finished episode clears one.
        self.has_saves = save::any_saved();
        self.menu_index = 0;
        self.set_screen(Screen::Menu);
    }

    /// The title menu gains a CONTINUE entry whenever a run is saved.
    fn menu_items(&self) -> Vec<&'static str> {
        let mut items = Vec::new();
        if self.has_saves {
            items.push(ui::CONTINUE);
        }
        items.push(ui::NEW_GAME);
        if self.has_saves {
            items.push(ui::LOAD_GAME);
        }
        items.push("SETTINGS");
        items.push("QUIT");
        items
    }

    fn pause(&mut self) {
        self.store_run();
        self.pause_index = 0;
        self.set_screen(Screen::Paused);
    }

    /// Continue the most recently written slot.
    fn continue_run(&mut self) {
        if let Some((slot, _)) = save::newest() {
            self.load_slot(slot);
        }
    }

    /// Restore one of the saved runs and start playing it.
    fn load_slot(&mut self, slot: usize) {
        let Some(run) = save::read_slot(slot) else {
            return;
        };
        self.current_slot = slot;
        self.level = run.snapshot.level.min(LEVELS.len() - 1);
        self.settings.difficulty = run.snapshot.difficulty;
        self.total_kills = run.total_kills;
        self.total_time = run.total_time;
        self.game = Game::from_snapshot(run.snapshot);
        self.level_banner = 2.6;
        self.set_screen(Screen::Playing);
    }

    /// Pause menu entry: pick a slot to write the current run into.
    fn open_save_screen(&mut self) {
        self.load_slots = save::summaries();
        self.load_index = self.current_slot.min(save::SLOTS - 1);
        self.set_screen(Screen::Save);
    }

    fn save_to_slot(&mut self, slot: usize) {
        let run = save::rundown(&self.game, self.total_kills, self.total_time);
        match save::write_slot(slot, &run) {
            Ok(()) => {
                self.current_slot = slot;
                self.has_saves = true;
                self.load_slots = save::summaries();
                self.notice(format!("SAVED TO SLOT {}", slot + 1));
            }
            Err(e) => self.notice(format!("COULD NOT SAVE: {e}")),
        }
    }

    fn open_load_screen(&mut self) {
        self.load_slots = save::summaries();
        self.load_index = self
            .load_slots
            .iter()
            .position(|slot| slot.is_some())
            .unwrap_or(0);
        self.set_screen(Screen::Load);
    }

    /// Write the level in progress to disk so it can be continued later.
    fn store_run(&mut self) {
        if !matches!(self.screen, Screen::Playing | Screen::Paused) {
            return;
        }
        if self.game.state != GameState::Playing {
            return;
        }
        let run = save::rundown(&self.game, self.total_kills, self.total_time);
        if save::write_slot(self.current_slot, &run).is_ok() {
            self.has_saves = true;
        }
    }

    fn notice(&mut self, text: impl Into<String>) {
        self.pad_notice = Some((text.into(), NOTICE_SECONDS));
    }

    fn leave_settings(&mut self) {
        if self.settings_from_pause {
            self.set_screen(Screen::Paused);
        } else {
            self.set_screen(Screen::Menu);
        }
    }

    fn finish_level(&mut self) {
        self.stats = ui::Stats {
            level_name: self.game.level_name(),
            kills: self.game.kills,
            total: self.game.total_enemies,
            time: self.game.time,
        };
        self.total_kills += self.game.kills;
        self.total_time += self.game.time;
        if self.level + 1 < LEVELS.len() {
            let next = Game::with_difficulty(self.level + 1, self.settings.difficulty);
            let run = save::rundown(&next, self.total_kills, self.total_time);
            if save::write_slot(self.current_slot, &run).is_ok() {
                self.has_saves = true;
            }
        } else {
            save::clear_slot(self.current_slot);
            self.has_saves = save::any_saved();
        }
        self.set_screen(Screen::Intermission);
    }

    fn next_level(&mut self) {
        if self.level + 1 < LEVELS.len() {
            self.start_game(self.level + 1);
        } else {
            self.run_score = config::Score {
                kills: self.total_kills,
                time: self.total_time,
                difficulty: self.settings.difficulty,
            };
            self.run_rank = config::insert_score(&mut self.scores, self.run_score);
            config::save_scores(&self.scores);
            self.set_screen(Screen::EpisodeComplete);
        }
    }

    /// Map a controller action onto the key it stands in for, then reuse the
    /// keyboard handling so both inputs behave identically.
    fn on_pad(&mut self, action: pad::PadAction) {
        use pad::PadAction;
        // B on the title screen would quit the game; require QUIT for that.
        if action == PadAction::Back && matches!(self.screen, Screen::Menu) {
            return;
        }
        let playing = matches!(self.screen, Screen::Playing);
        let code = match action {
            PadAction::Up => KeyCode::ArrowUp,
            PadAction::Down => KeyCode::ArrowDown,
            PadAction::Left => KeyCode::ArrowLeft,
            PadAction::Right => KeyCode::ArrowRight,
            PadAction::Confirm => KeyCode::Enter,
            PadAction::Back => KeyCode::Escape,
            // Start pauses in game and confirms in menus.
            PadAction::Start => {
                if playing {
                    KeyCode::Escape
                } else {
                    KeyCode::Enter
                }
            }
            PadAction::Use => {
                if playing {
                    KeyCode::KeyE
                } else {
                    KeyCode::Enter
                }
            }
            PadAction::PrevWeapon => KeyCode::Digit1,
            PadAction::NextWeapon => KeyCode::Digit2,
            PadAction::Map => KeyCode::Tab,
        };
        self.on_key(code);
    }

    fn on_key(&mut self, code: KeyCode) {
        match self.screen {
            Screen::Menu => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.menu_index = wrap(self.menu_index, self.menu_items().len(), -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.menu_index = wrap(self.menu_index, self.menu_items().len(), 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.ui_sound(SoundId::MenuSelect);
                    let items = self.menu_items();
                    let choice = items[self.menu_index.min(items.len() - 1)];
                    match choice {
                        ui::CONTINUE => self.continue_run(),
                        ui::NEW_GAME => {
                            self.skill_index = self.settings.difficulty.index();
                            self.set_screen(Screen::Skills);
                        }
                        ui::LOAD_GAME => self.open_load_screen(),
                        "SETTINGS" => {
                            self.settings_from_pause = false;
                            self.settings_index = 0;
                            self.set_screen(Screen::Settings);
                        }
                        // QUIT and Esc only take effect while held.
                        _ => {}
                    }
                }
                _ => {}
            },
            Screen::Save => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.load_index = wrap(self.load_index, save::SLOTS, -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.load_index = wrap(self.load_index, save::SLOTS, 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => {
                    let slot = self.load_index;
                    self.ui_sound(SoundId::MenuSelect);
                    self.save_to_slot(slot);
                    self.set_screen(Screen::Paused);
                }
                KeyCode::Escape => {
                    self.ui_sound(SoundId::MenuMove);
                    self.set_screen(Screen::Paused);
                }
                _ => {}
            },
            Screen::Load => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.load_index = wrap(self.load_index, save::SLOTS, -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.load_index = wrap(self.load_index, save::SLOTS, 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => {
                    if self
                        .load_slots
                        .get(self.load_index)
                        .is_some_and(|slot| slot.is_some())
                    {
                        self.ui_sound(SoundId::MenuSelect);
                        self.load_slot(self.load_index);
                    } else {
                        self.ui_sound(SoundId::MenuMove);
                    }
                }
                KeyCode::Escape => {
                    self.ui_sound(SoundId::MenuMove);
                    self.show_menu();
                }
                _ => {}
            },
            Screen::Skills => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.skill_index = wrap(self.skill_index, Difficulty::ALL.len(), -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.skill_index = wrap(self.skill_index, Difficulty::ALL.len(), 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.ui_sound(SoundId::MenuSelect);
                    self.settings.difficulty = Difficulty::from_index(self.skill_index);
                    config::save(&self.settings);
                    self.begin_intro();
                }
                KeyCode::Escape => {
                    self.ui_sound(SoundId::MenuMove);
                    self.show_menu();
                }
                _ => {}
            },
            Screen::Intro { .. } => {
                if matches!(code, KeyCode::Enter | KeyCode::Space | KeyCode::Escape) {
                    self.ui_sound(SoundId::MenuSelect);
                    self.start_game(0);
                }
            }
            Screen::Settings => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.settings_index = wrap(self.settings_index, ui::SETTINGS_ITEMS.len(), -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.settings_index = wrap(self.settings_index, ui::SETTINGS_ITEMS.len(), 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowLeft | KeyCode::KeyA => {
                    match self.settings_index {
                        0 => {
                            self.settings.adjust_sensitivity(-1.0);
                            config::save(&self.settings);
                        }
                        1 => {
                            self.settings.adjust_stick_sensitivity(-1.0);
                            config::save(&self.settings);
                        }
                        2 => self.set_invert_stick(false),
                        4 => self.set_fullscreen(false),
                        _ => {}
                    }
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowRight | KeyCode::KeyD => {
                    match self.settings_index {
                        0 => {
                            self.settings.adjust_sensitivity(1.0);
                            config::save(&self.settings);
                        }
                        1 => {
                            self.settings.adjust_stick_sensitivity(1.0);
                            config::save(&self.settings);
                        }
                        2 => self.set_invert_stick(true),
                        4 => self.set_fullscreen(true),
                        _ => {}
                    }
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => match self.settings_index {
                    2 => {
                        let next = !self.settings.invert_stick;
                        self.set_invert_stick(next);
                        self.ui_sound(SoundId::MenuSelect);
                    }
                    3 => {
                        self.volume_index = 0;
                        self.ui_sound(SoundId::MenuSelect);
                        self.set_screen(Screen::Volume);
                    }
                    4 => {
                        let next = !self.settings.fullscreen;
                        self.set_fullscreen(next);
                        self.ui_sound(SoundId::MenuSelect);
                    }
                    5 => {
                        self.ui_sound(SoundId::MenuSelect);
                        self.leave_settings();
                    }
                    _ => {}
                },
                KeyCode::Escape => {
                    self.ui_sound(SoundId::MenuMove);
                    self.leave_settings();
                }
                _ => {}
            },
            Screen::Volume => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.volume_index = wrap(self.volume_index, ui::VOLUME_ITEMS.len(), -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.volume_index = wrap(self.volume_index, ui::VOLUME_ITEMS.len(), 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowLeft | KeyCode::KeyA => {
                    if self.volume_index < 3 {
                        self.settings.adjust_volume(self.volume_index, -1.0);
                        self.push_volumes();
                        self.ui_sound(SoundId::MenuMove);
                    }
                }
                KeyCode::ArrowRight | KeyCode::KeyD => {
                    if self.volume_index < 3 {
                        self.settings.adjust_volume(self.volume_index, 1.0);
                        self.push_volumes();
                        self.ui_sound(SoundId::MenuMove);
                    }
                }
                KeyCode::Enter | KeyCode::Space => {
                    if self.volume_index == 3 {
                        self.ui_sound(SoundId::MenuSelect);
                        self.set_screen(Screen::Settings);
                    }
                }
                KeyCode::Escape => {
                    self.ui_sound(SoundId::MenuMove);
                    self.set_screen(Screen::Settings);
                }
                _ => {}
            },
            Screen::Playing => match code {
                KeyCode::Escape => {
                    if self.game.state == GameState::Dead {
                        self.show_menu();
                    } else {
                        self.pause();
                    }
                }
                KeyCode::Enter if self.game.state == GameState::Dead => {
                    self.input.restart = true;
                }
                KeyCode::KeyE => self.input.use_pressed = true,
                KeyCode::Tab => self.input.toggle_map = true,
                KeyCode::KeyR => self.input.restart = true,
                KeyCode::Digit1 => self.input.weapon1 = true,
                KeyCode::Digit2 => self.input.weapon2 = true,
                _ => {}
            },
            Screen::Paused => match code {
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    self.pause_index = wrap(self.pause_index, ui::PAUSE_ITEMS.len(), -1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    self.pause_index = wrap(self.pause_index, ui::PAUSE_ITEMS.len(), 1);
                    self.ui_sound(SoundId::MenuMove);
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.ui_sound(SoundId::MenuSelect);
                    match self.pause_index {
                        0 => self.set_screen(Screen::Playing),
                        1 => self.open_save_screen(),
                        2 => {
                            self.settings_from_pause = true;
                            self.settings_index = 0;
                            self.set_screen(Screen::Settings);
                        }
                        _ => self.show_menu(),
                    }
                }
                KeyCode::Escape => self.set_screen(Screen::Playing),
                _ => {}
            },
            Screen::Intermission => {
                if matches!(code, KeyCode::Enter | KeyCode::Space) {
                    self.ui_sound(SoundId::MenuSelect);
                    self.next_level();
                } else if code == KeyCode::Escape {
                    self.show_menu();
                }
            }
            Screen::EpisodeComplete => {
                if matches!(code, KeyCode::Enter | KeyCode::Space | KeyCode::Escape) {
                    self.ui_sound(SoundId::MenuSelect);
                    self.show_menu();
                }
            }
        }
    }

    fn apply_held_keys(&mut self) {
        let k = &self.keys;
        self.input.forward = k.contains(&KeyCode::KeyW) || k.contains(&KeyCode::ArrowUp);
        self.input.back = k.contains(&KeyCode::KeyS) || k.contains(&KeyCode::ArrowDown);
        self.input.strafe_l = k.contains(&KeyCode::KeyA);
        self.input.strafe_r = k.contains(&KeyCode::KeyD);
        self.input.turn_l = k.contains(&KeyCode::ArrowLeft);
        self.input.turn_r = k.contains(&KeyCode::ArrowRight);
        self.input.run = k.contains(&KeyCode::ShiftLeft) || k.contains(&KeyCode::ShiftRight);
        self.input.fire =
            self.mouse_fire || k.contains(&KeyCode::Space) || k.contains(&KeyCode::ControlLeft);
    }

    fn clear_one_shots(&mut self) {
        self.input.use_pressed = false;
        self.input.weapon1 = false;
        self.input.weapon2 = false;
        self.input.restart = false;
        self.input.toggle_map = false;
        self.input.mouse_dx = 0.0;
    }

    fn drain_game_sounds(&mut self) {
        let sounds = self.game.take_sounds();
        if let Some(audio) = &self.audio {
            for sound in sounds {
                audio.play(sound);
            }
        }
    }

    fn draw(&mut self) {
        match self.screen {
            Screen::Menu => {
                let items = self.menu_items();
                let progress = self.quit_hold / QUIT_HOLD_SECONDS;
                ui::title_menu(
                    &mut self.renderer.frame,
                    self.anim_time,
                    &items,
                    self.menu_index,
                    self.scores.first(),
                    progress,
                )
            }
            Screen::Save => ui::save_screen(
                &mut self.renderer.frame,
                self.anim_time,
                &self.load_slots,
                self.load_index,
            ),
            Screen::Load => ui::load_screen(
                &mut self.renderer.frame,
                self.anim_time,
                &self.load_slots,
                self.load_index,
            ),
            Screen::Skills => {
                ui::skill_screen(&mut self.renderer.frame, self.anim_time, self.skill_index)
            }
            Screen::Intro { elapsed } => ui::intro_screen(&mut self.renderer.frame, elapsed),
            Screen::Settings => ui::settings_screen(
                &mut self.renderer.frame,
                self.anim_time,
                &self.settings,
                self.settings_index,
            ),
            Screen::Volume => ui::volume_screen(
                &mut self.renderer.frame,
                self.anim_time,
                &self.settings,
                self.volume_index,
            ),
            Screen::Intermission => ui::intermission_screen(
                &mut self.renderer.frame,
                self.anim_time,
                &self.stats,
                self.level + 1 < LEVELS.len(),
            ),
            Screen::EpisodeComplete => ui::episode_complete(
                &mut self.renderer.frame,
                self.anim_time,
                &self.run_score,
                &self.scores,
                self.run_rank,
            ),
            Screen::Playing | Screen::Paused => {
                self.renderer.render(&self.game);
                if matches!(self.screen, Screen::Playing) && self.game.state == GameState::Playing {
                    ui::level_banner(
                        &mut self.renderer.frame,
                        self.game.level_name(),
                        self.level_banner,
                    );
                }
                if matches!(self.screen, Screen::Paused) {
                    ui::pause_screen(&mut self.renderer.frame, self.pause_index);
                }
            }
        }
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }

        // Cap at 60 fps even if the compositor does not vsync.
        let now = Instant::now();
        let elapsed = now - self.last_frame;
        if elapsed < Duration::from_micros(16_600) {
            std::thread::sleep(Duration::from_micros(16_600) - elapsed);
        }
        let dt = self.last_render.elapsed().as_secs_f32().min(0.05);
        self.last_frame = Instant::now();
        self.last_render = self.last_frame;
        self.anim_time += dt;

        let mut advance_intro = false;
        if let Screen::Intro { elapsed } = &mut self.screen {
            *elapsed += dt;
            advance_intro = *elapsed >= INTRO_SECONDS;
        }
        if advance_intro {
            self.start_game(0);
        }

        if matches!(self.screen, Screen::Menu) {
            // Esc always arms the quit bar; Enter/Space only on the QUIT row.
            let on_quit = self.menu_items().get(self.menu_index).copied() == Some("QUIT");
            let confirm =
                self.keys.contains(&KeyCode::Enter) || self.keys.contains(&KeyCode::Space);
            let holding = self.keys.contains(&KeyCode::Escape) || (on_quit && confirm);
            self.quit_hold = if holding { self.quit_hold + dt } else { 0.0 };
            if self.quit_hold >= QUIT_HOLD_SECONDS {
                self.quit_requested = true;
            }
        }

        if matches!(self.screen, Screen::Playing) {
            self.apply_held_keys();
            let motion = self.pad_motion;
            self.input.move_axis = (motion.forward, motion.strafe);
            self.input.fire |= motion.fire;
            self.input.mouse_dx +=
                motion.turn * self.settings.stick_sensitivity * PAD_TURN_RATE * dt;
            self.game.update(dt, &self.input);
            self.drain_game_sounds();
            self.clear_one_shots();
            self.level_banner = (self.level_banner - dt).max(0.0);
            if self.game.state == GameState::Won {
                self.finish_level();
            }
        }

        let notice_expired = if let Some((_, left)) = &mut self.pad_notice {
            *left -= dt;
            *left <= 0.0
        } else {
            false
        };
        if notice_expired {
            self.pad_notice = None;
        }

        self.draw();
        let notice = self.pad_notice.clone();
        if let Some((text, left)) = notice {
            ui::notice(&mut self.renderer.frame, &text, left);
        }

        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        self.renderer
            .blit_to(&mut buffer, size.width as usize, size.height as usize);
        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("D00M")
            .with_inner_size(LogicalSize::new(
                W as f64 * WINDOW_SCALE,
                H as f64 * WINDOW_SCALE,
            ))
            .with_min_inner_size(LogicalSize::new(480.0, 300.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("could not open a window: {e}");
                event_loop.exit();
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("softbuffer context failed: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("softbuffer surface failed: {e}");
                event_loop.exit();
                return;
            }
        };
        Self::grab_pointer(&window);
        self.grabbed = true;
        self.window = Some(window.clone());
        self.apply_fullscreen();
        self.context = Some(context);
        self.surface = Some(surface);
        self.last_render = Instant::now();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.store_run();
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                match event.state {
                    ElementState::Pressed => {
                        if self.keys.insert(code) && !event.repeat {
                            self.on_key(code);
                        }
                    }
                    ElementState::Released => {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_fire = state == ElementState::Pressed;
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.mouse_fire = false;
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.grabbed
            && matches!(self.screen, Screen::Playing)
        {
            self.input.mouse_dx += delta.0 as f32 * self.settings.sensitivity;
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let invert = self.settings.invert_stick;
        if self.quit_requested {
            event_loop.exit();
            return;
        }
        let polled = self.pad.as_mut().map(|pad| pad.poll(invert));
        if let Some((actions, motion, event)) = polled {
            match event {
                Some(pad::PadEvent::Connected(name)) => {
                    println!("gamepad: {name}");
                    self.notice(format!("GAMEPAD CONNECTED: {name}"));
                }
                Some(pad::PadEvent::Disconnected) => {
                    println!("gamepad: disconnected");
                    self.notice("GAMEPAD DISCONNECTED");
                }
                None => {}
            }
            self.pad_motion = motion;
            for action in actions {
                self.on_pad(action);
            }
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

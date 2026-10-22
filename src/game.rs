//! Game simulation: player, weapons, hitscan combat, enemy AI, doors, pickups.
//! Rendering lives in `render.rs`; this module has no graphics code.

use crate::audio::{SoundId, SoundRequest};
use crate::levels::LEVELS;
use crate::map::{Cell, Map, UseOutcome};
use crate::math::{Rng, Vec2};
use crate::textures::*;

const PLAYER_RADIUS: f32 = 0.28;
const WALK_SPEED: f32 = 3.1;
const RUN_SPEED: f32 = 4.9;
const MOUSE_SENSITIVITY: f32 = 0.0022;

/// How hard the episode is. Chosen on the skill screen and remembered in the
/// saved settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn index(self) -> usize {
        match self {
            Difficulty::Easy => 0,
            Difficulty::Normal => 1,
            Difficulty::Hard => 2,
        }
    }

    pub fn from_index(index: usize) -> Difficulty {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    /// Doom-style skill names for the selection screen.
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "I'M TOO YOUNG TO DIE",
            Difficulty::Normal => "HURT ME PLENTY",
            Difficulty::Hard => "NIGHTMARE!",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Difficulty::Easy => "EASY",
            Difficulty::Normal => "NORMAL",
            Difficulty::Hard => "HARD",
        }
    }

    /// Multiplier on everything the demons do to the player.
    pub fn damage_taken(self) -> f32 {
        match self {
            Difficulty::Easy => 0.55,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.5,
        }
    }

    pub fn enemy_health(self) -> i32 {
        match self {
            Difficulty::Easy => 30,
            Difficulty::Normal => 40,
            Difficulty::Hard => 55,
        }
    }

    pub fn enemy_speed(self) -> f32 {
        match self {
            Difficulty::Easy => 0.8,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.25,
        }
    }

    /// Multiplier on how long imps wait between attacks.
    pub fn enemy_cooldown(self) -> f32 {
        match self {
            Difficulty::Easy => 1.35,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.72,
        }
    }

    /// Whether the nth imp in a map spawns at all. Easy throws a third fewer
    /// demons at you; both other skills use the whole map.
    pub fn spawns_enemy(self, index: usize) -> bool {
        match self {
            Difficulty::Easy => index % 3 != 2,
            _ => true,
        }
    }

    /// Whether the nth pickup in a map spawns. Hard leaves fewer supplies.
    pub fn spawns_pickup(self, index: usize) -> bool {
        match self {
            Difficulty::Hard => index % 3 != 2,
            _ => true,
        }
    }

    /// How much health and ammo a pickup is worth, relative to normal.
    pub fn pickup_bonus(self) -> f32 {
        match self {
            Difficulty::Easy => 1.25,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.8,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameState {
    Playing,
    Dead,
    Won,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weapon {
    Pistol,
    Shotgun,
}

impl Weapon {
    pub fn index(self) -> usize {
        match self {
            Weapon::Pistol => 0,
            Weapon::Shotgun => 1,
        }
    }

    pub fn from_index(index: usize) -> Weapon {
        if index == 1 {
            Weapon::Shotgun
        } else {
            Weapon::Pistol
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Weapon::Pistol => "PISTOL",
            Weapon::Shotgun => "SHOTGUN",
        }
    }

    fn cooldown(self) -> f32 {
        match self {
            Weapon::Pistol => 0.30,
            Weapon::Shotgun => 0.82,
        }
    }

    fn pellets(self) -> usize {
        match self {
            Weapon::Pistol => 1,
            Weapon::Shotgun => 7,
        }
    }

    fn spread(self) -> f32 {
        match self {
            Weapon::Pistol => 0.012,
            Weapon::Shotgun => 0.085,
        }
    }

    fn damage(self) -> i32 {
        match self {
            Weapon::Pistol => 14,
            Weapon::Shotgun => 8,
        }
    }

    pub fn tex(self, firing: bool) -> usize {
        match (self, firing) {
            (Weapon::Pistol, false) => WEAPON_PISTOL,
            (Weapon::Pistol, true) => WEAPON_PISTOL_FIRE,
            (Weapon::Shotgun, false) => WEAPON_SHOTGUN,
            (Weapon::Shotgun, true) => WEAPON_SHOTGUN_FIRE,
        }
    }
}

pub struct Player {
    pub pos: Vec2,
    pub angle: f32,
    pub health: i32,
    pub bullets: i32,
    pub shells: i32,
    pub weapon: Weapon,
    pub has_shotgun: bool,
    pub cooldown: f32,
    pub fire_anim: f32,
    pub muzzle: f32,
    pub bob: f32,
    bob_phase: f32,
    pub hurt: f32,
}

impl Player {
    pub fn ammo(&self) -> i32 {
        match self.weapon {
            Weapon::Pistol => self.bullets,
            Weapon::Shotgun => self.shells,
        }
    }

    /// Phase of the walk cycle, used to sway the weapon.
    pub fn sway_phase(&self) -> f32 {
        self.bob_phase
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyState {
    Idle,
    Chase,
    Attack,
    Pain,
    Dead,
}

pub struct Enemy {
    pub pos: Vec2,
    pub health: i32,
    pub state: EnemyState,
    pub anim: f32,
    pub cooldown: f32,
    state_time: f32,
    pub dead_time: f32,
    pub alive: bool,
}

impl EnemyState {
    pub fn index(self) -> usize {
        match self {
            EnemyState::Idle => 0,
            EnemyState::Chase => 1,
            EnemyState::Attack => 2,
            EnemyState::Pain => 3,
            EnemyState::Dead => 4,
        }
    }

    pub fn from_index(index: usize) -> EnemyState {
        match index {
            1 => EnemyState::Chase,
            2 => EnemyState::Attack,
            3 => EnemyState::Pain,
            4 => EnemyState::Dead,
            _ => EnemyState::Idle,
        }
    }
}

impl Enemy {
    pub fn sprite(&self) -> usize {
        match self.state {
            EnemyState::Dead => {
                if self.dead_time < 0.15 {
                    SPR_IMP_DIE0
                } else if self.dead_time < 0.30 {
                    SPR_IMP_DIE1
                } else {
                    SPR_IMP_CORPSE
                }
            }
            EnemyState::Pain => SPR_IMP_PAIN,
            EnemyState::Attack => SPR_IMP_ATTACK,
            _ => {
                const WALK: [usize; 4] =
                    [SPR_IMP_WALK0, SPR_IMP_WALK1, SPR_IMP_WALK2, SPR_IMP_WALK3];
                WALK[(self.anim * 6.0) as usize % 4]
            }
        }
    }
}

pub struct Barrel {
    pub pos: Vec2,
    pub health: i32,
    pub alive: bool,
    pub fade: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickupKind {
    Medkit,
    Ammo,
    Shells,
    Shotgun,
}

impl PickupKind {
    pub fn index(self) -> usize {
        match self {
            PickupKind::Medkit => 0,
            PickupKind::Ammo => 1,
            PickupKind::Shells => 2,
            PickupKind::Shotgun => 3,
        }
    }

    pub fn from_index(index: usize) -> PickupKind {
        match index {
            1 => PickupKind::Ammo,
            2 => PickupKind::Shells,
            3 => PickupKind::Shotgun,
            _ => PickupKind::Medkit,
        }
    }
}

pub struct Pickup {
    pub pos: Vec2,
    pub kind: PickupKind,
    pub taken: bool,
}

/// Everything needed to rebuild a level exactly as it was, for save games.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub level: usize,
    pub difficulty: Difficulty,
    pub total_enemies: usize,
    pub time: f32,
    pub kills: usize,
    pub player: PlayerSnapshot,
    pub enemies: Vec<EnemySnapshot>,
    pub barrels: Vec<BarrelSnapshot>,
    pub pickups: Vec<PickupSnapshot>,
    pub doors: Vec<DoorSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub pos: Vec2,
    pub angle: f32,
    pub health: i32,
    pub bullets: i32,
    pub shells: i32,
    pub weapon: Weapon,
    pub has_shotgun: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemySnapshot {
    pub pos: Vec2,
    pub health: i32,
    pub state: EnemyState,
    pub anim: f32,
    pub cooldown: f32,
    pub state_time: f32,
    pub dead_time: f32,
    pub alive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BarrelSnapshot {
    pub pos: Vec2,
    pub health: i32,
    pub alive: bool,
    pub fade: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickupSnapshot {
    pub pos: Vec2,
    pub kind: PickupKind,
    pub taken: bool,
}

/// A sliding door's progress and timer, keyed by its map cell.
#[derive(Clone, Debug, PartialEq)]
pub struct DoorSnapshot {
    pub x: i32,
    pub y: i32,
    pub open: f32,
    pub phase: u8,
    pub timer: f32,
}

pub struct Projectile {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
}

pub struct Sprite {
    pub pos: Vec2,
    pub tex: usize,
    pub w: f32,
    pub h: f32,
    pub z: f32,
}

#[derive(Default, Clone)]
pub struct Input {
    /// Analog movement from a gamepad: (forward, strafe), each -1 to 1.
    pub move_axis: (f32, f32),
    pub forward: bool,
    pub back: bool,
    pub strafe_l: bool,
    pub strafe_r: bool,
    pub turn_l: bool,
    pub turn_r: bool,
    pub run: bool,
    pub fire: bool,
    pub use_pressed: bool,
    pub weapon1: bool,
    pub weapon2: bool,
    pub restart: bool,
    pub toggle_map: bool,
    pub mouse_dx: f32,
}

pub struct Game {
    pub level: usize,
    pub difficulty: Difficulty,
    pub map: Map,
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub barrels: Vec<Barrel>,
    pub pickups: Vec<Pickup>,
    pub projectiles: Vec<Projectile>,
    pub messages: Vec<(String, f32)>,
    pub state: GameState,
    pub time: f32,
    pub kills: usize,
    pub total_enemies: usize,
    pub map_open: bool,
    /// Sound events raised this frame; drained by the front end.
    pub sounds: Vec<SoundRequest>,
    rng: Rng,
}

impl Game {
    pub fn new(level: usize) -> Self {
        Self::with_difficulty(level, Difficulty::default())
    }

    pub fn with_difficulty(level: usize, difficulty: Difficulty) -> Self {
        let level = level.min(LEVELS.len() - 1);
        let map = Map::from_level(LEVELS[level].rows);
        let mut spawn = Vec2::new(2.5, 2.5);
        let mut enemies = Vec::new();
        let mut barrels = Vec::new();
        let mut pickups = Vec::new();
        let mut imp_count = 0usize;
        let mut pickup_count = 0usize;

        for (y, row) in LEVELS[level].rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let pos = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                match ch {
                    'P' => spawn = pos,
                    'i' => {
                        if difficulty.spawns_enemy(imp_count) {
                            enemies.push(Enemy {
                                pos,
                                health: difficulty.enemy_health(),
                                state: EnemyState::Idle,
                                anim: 0.0,
                                cooldown: 0.6,
                                state_time: 0.0,
                                dead_time: 0.0,
                                alive: true,
                            });
                        }
                        imp_count += 1;
                    }
                    'b' => barrels.push(Barrel {
                        pos,
                        health: 22,
                        alive: true,
                        fade: 0.0,
                    }),
                    'm' | 'a' | 's' => {
                        if difficulty.spawns_pickup(pickup_count) {
                            let kind = match ch {
                                'm' => PickupKind::Medkit,
                                'a' => PickupKind::Ammo,
                                _ => PickupKind::Shells,
                            };
                            pickups.push(Pickup {
                                pos,
                                kind,
                                taken: false,
                            });
                        }
                        pickup_count += 1;
                    }
                    // The shotgun always spawns: without it the harder skills
                    // would be unwinnable rather than hard.
                    'g' => pickups.push(Pickup {
                        pos,
                        kind: PickupKind::Shotgun,
                        taken: false,
                    }),
                    _ => {}
                }
            }
        }

        let total_enemies = enemies.len();
        Self {
            level,
            difficulty,
            map,
            player: Player {
                pos: spawn,
                angle: 0.0,
                health: 100,
                bullets: 50,
                shells: 0,
                weapon: Weapon::Pistol,
                has_shotgun: false,
                cooldown: 0.0,
                fire_anim: 0.0,
                muzzle: 0.0,
                bob: 0.0,
                bob_phase: 0.0,
                hurt: 0.0,
            },
            enemies,
            barrels,
            pickups,
            projectiles: Vec::new(),
            messages: Vec::new(),
            state: GameState::Playing,
            time: 0.0,
            kills: 0,
            total_enemies,
            map_open: false,
            sounds: Vec::new(),
            rng: Rng::new(0xD00D_1234_5678_9ABC),
        }
    }

    /// Capture the whole level so it can be written to disk and resumed later.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            level: self.level,
            difficulty: self.difficulty,
            total_enemies: self.total_enemies,
            time: self.time,
            kills: self.kills,
            player: PlayerSnapshot {
                pos: self.player.pos,
                angle: self.player.angle,
                health: self.player.health,
                bullets: self.player.bullets,
                shells: self.player.shells,
                weapon: self.player.weapon,
                has_shotgun: self.player.has_shotgun,
            },
            enemies: self
                .enemies
                .iter()
                .map(|e| EnemySnapshot {
                    pos: e.pos,
                    health: e.health,
                    state: e.state,
                    anim: e.anim,
                    cooldown: e.cooldown,
                    state_time: e.state_time,
                    dead_time: e.dead_time,
                    alive: e.alive,
                })
                .collect(),
            barrels: self
                .barrels
                .iter()
                .map(|b| BarrelSnapshot {
                    pos: b.pos,
                    health: b.health,
                    alive: b.alive,
                    fade: b.fade,
                })
                .collect(),
            pickups: self
                .pickups
                .iter()
                .map(|p| PickupSnapshot {
                    pos: p.pos,
                    kind: p.kind,
                    taken: p.taken,
                })
                .collect(),
            doors: self
                .map
                .door_states()
                .into_iter()
                .map(|(x, y, open, phase, timer)| DoorSnapshot {
                    x,
                    y,
                    open,
                    phase,
                    timer,
                })
                .collect(),
        }
    }

    /// Rebuild a level from a snapshot, keeping every value inside the range
    /// the game itself can produce.
    pub fn from_snapshot(s: Snapshot) -> Game {
        let mut game = Game::with_difficulty(s.level, s.difficulty);
        game.total_enemies = s.total_enemies.max(s.enemies.len());
        game.time = s.time.max(0.0);
        game.kills = s.kills.min(game.total_enemies);
        game.player.pos = s.player.pos;
        game.player.angle = s.player.angle.rem_euclid(std::f32::consts::TAU);
        game.player.health = s.player.health.clamp(1, 100);
        game.player.bullets = s.player.bullets.clamp(0, 200);
        game.player.shells = s.player.shells.clamp(0, 60);
        game.player.weapon = s.player.weapon;
        game.player.has_shotgun = s.player.has_shotgun || s.player.weapon == Weapon::Shotgun;
        game.player.cooldown = 0.0;
        game.enemies = s
            .enemies
            .into_iter()
            .map(|e| Enemy {
                pos: e.pos,
                health: e.health,
                state: e.state,
                anim: e.anim,
                cooldown: e.cooldown,
                state_time: e.state_time,
                dead_time: e.dead_time,
                alive: e.alive,
            })
            .collect();
        game.barrels = s
            .barrels
            .into_iter()
            .map(|b| Barrel {
                pos: b.pos,
                health: b.health,
                alive: b.alive,
                fade: b.fade,
            })
            .collect();
        game.pickups = s
            .pickups
            .into_iter()
            .map(|p| Pickup {
                pos: p.pos,
                kind: p.kind,
                taken: p.taken,
            })
            .collect();
        let doors: Vec<(i32, i32, f32, u8, f32)> = s
            .doors
            .iter()
            .map(|d| (d.x, d.y, d.open, d.phase, d.timer))
            .collect();
        game.map.restore_door_states(&doors);
        game
    }

    pub fn level_name(&self) -> &'static str {
        LEVELS[self.level].name
    }

    pub fn message(&mut self, text: impl Into<String>) {
        self.messages.insert(0, (text.into(), 3.5));
        self.messages.truncate(3);
    }

    pub fn take_sounds(&mut self) -> Vec<SoundRequest> {
        std::mem::take(&mut self.sounds)
    }

    /// Play an effect positioned in the world, panned relative to the camera.
    fn play_at(&mut self, id: SoundId, pos: Vec2, gain: f32) {
        let rel = pos - self.player.pos;
        let dist = rel.len();
        if dist > 24.0 {
            return;
        }
        let right = Vec2::from_angle(self.player.angle).perp();
        let pan = if dist > 0.4 {
            rel.norm().dot(right).clamp(-1.0, 1.0) * 0.85
        } else {
            0.0
        };
        let gain = gain / (1.0 + dist * 0.22);
        self.sounds.push(SoundRequest { id, gain, pan });
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.clamp(0.0001, 0.05);
        self.time += dt;
        for m in &mut self.messages {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);

        if input.restart {
            let level = self.level;
            *self = Game::with_difficulty(level, self.difficulty);
            return;
        }
        if input.toggle_map {
            self.map_open = !self.map_open;
        }
        self.player.hurt = (self.player.hurt - dt * 1.4).max(0.0);
        self.player.muzzle = (self.player.muzzle - dt).max(0.0);
        self.player.fire_anim = (self.player.fire_anim - dt * 6.0).max(0.0);
        self.player.cooldown = (self.player.cooldown - dt).max(0.0);
        for e in &mut self.enemies {
            if !e.alive {
                e.dead_time += dt;
            }
        }

        if self.state != GameState::Playing {
            return;
        }

        self.handle_look(dt, input);
        self.handle_movement(dt, input);
        self.handle_weapons(dt, input);
        self.map.update(dt, self.player.pos);
        for pos in std::mem::take(&mut self.map.closed_doors) {
            self.play_at(SoundId::DoorClose, pos, 0.85);
        }
        self.update_enemies(dt);
        self.update_projectiles(dt);
        self.update_pickups();
        self.update_barrels(dt);
    }

    fn handle_look(&mut self, dt: f32, input: &Input) {
        let mut turn = input.mouse_dx * MOUSE_SENSITIVITY;
        if input.turn_l {
            turn -= 2.4 * dt;
        }
        if input.turn_r {
            turn += 2.4 * dt;
        }
        self.player.angle = (self.player.angle + turn).rem_euclid(std::f32::consts::TAU);
    }

    fn handle_movement(&mut self, dt: f32, input: &Input) {
        let fwd = Vec2::from_angle(self.player.angle);
        let right = fwd.perp();
        let mut wish = Vec2::default();
        if input.forward {
            wish += fwd;
        }
        if input.back {
            wish -= fwd;
        }
        if input.strafe_l {
            wish -= right;
        }
        if input.strafe_r {
            wish += right;
        }
        let (axis_forward, axis_strafe) = input.move_axis;
        if axis_forward.abs() > 1e-3 || axis_strafe.abs() > 1e-3 {
            wish += fwd * axis_forward + right * axis_strafe;
        }
        // Diagonals (and a full stick plus a key) must not move faster than forward.
        if wish.len() > 1.0 {
            wish = wish.norm();
        }
        let speed = if input.run { RUN_SPEED } else { WALK_SPEED };
        if wish.len() > 1e-4 {
            let delta = wish.norm() * speed * dt;
            self.map
                .try_move(&mut self.player.pos, delta, PLAYER_RADIUS);
            self.player.bob_phase += speed * dt * 2.4;
            self.player.bob = self.player.bob_phase.sin() * 2.2;
        } else {
            self.player.bob *= (1.0 - dt * 8.0).max(0.0);
        }
        self.resolve_entity_overlap();
    }

    /// Keep the player from standing inside imps and barrels.
    fn resolve_entity_overlap(&mut self) {
        let mut push = Vec2::default();
        for b in &self.barrels {
            if !b.alive {
                continue;
            }
            let d = self.player.pos - b.pos;
            if d.len() < 0.62 {
                push += d.norm() * (0.62 - d.len());
            }
        }
        for e in &self.enemies {
            if !e.alive {
                continue;
            }
            let d = self.player.pos - e.pos;
            if d.len() < 0.58 {
                push += d.norm() * (0.58 - d.len());
            }
        }
        if push.len() > 1e-4 {
            let target = self.player.pos + push;
            if !self.map.circle_blocked(target, PLAYER_RADIUS) {
                self.player.pos = target;
            }
        }
    }

    fn handle_weapons(&mut self, _dt: f32, input: &Input) {
        if input.weapon1 {
            self.player.weapon = Weapon::Pistol;
            self.player.cooldown = self.player.cooldown.max(0.15);
        }
        if input.weapon2 {
            if self.player.has_shotgun {
                self.player.weapon = Weapon::Shotgun;
                self.player.cooldown = self.player.cooldown.max(0.15);
            } else {
                self.message("YOU NEED THE SHOTGUN");
            }
        }

        if input.use_pressed {
            match self.map.use_at(self.player.pos, self.player.angle) {
                UseOutcome::Opened => {
                    let pos = self.player.pos + Vec2::from_angle(self.player.angle) * 0.9;
                    self.play_at(SoundId::DoorOpen, pos, 0.9);
                }
                UseOutcome::Exit => {
                    self.state = GameState::Won;
                    self.message("EXIT ACTIVATED");
                }
                UseOutcome::None => {}
            }
        }

        if input.fire {
            self.try_fire();
        }
    }

    fn try_fire(&mut self) {
        if self.player.cooldown > 0.0 || self.state != GameState::Playing {
            return;
        }
        let weapon = self.player.weapon;
        if self.player.ammo() <= 0 {
            self.player.cooldown = 0.35;
            self.message("OUT OF AMMO");
            let pos = self.player.pos;
            self.play_at(SoundId::Empty, pos, 0.8);
            return;
        }
        match weapon {
            Weapon::Pistol => self.player.bullets -= 1,
            Weapon::Shotgun => self.player.shells -= 1,
        }
        let sound = match weapon {
            Weapon::Pistol => SoundId::Pistol,
            Weapon::Shotgun => SoundId::Shotgun,
        };
        self.sounds.push(SoundRequest {
            id: sound,
            gain: 0.9,
            pan: 0.0,
        });
        self.player.cooldown = weapon.cooldown();
        self.player.fire_anim = 1.0;
        self.player.muzzle = 0.07;
        self.player.bob_phase += 0.12;

        let base = self.player.angle;
        for _ in 0..weapon.pellets() {
            let spread = self.rng.range(-weapon.spread(), weapon.spread());
            let dir = Vec2::from_angle(base + spread);
            if self.hitscan(dir, weapon.damage()) {
                self.player.fire_anim = 1.0;
            }
        }
    }

    /// Cast a bullet. Returns true if it hit an enemy.
    fn hitscan(&mut self, dir: Vec2, damage: i32) -> bool {
        let range = 26.0;
        let wall_d = self.map.ray_wall_dist(self.player.pos, dir, range);
        let mut best: Option<(f32, usize)> = None;
        let mut best_barrel: Option<(f32, usize)> = None;

        for (i, e) in self.enemies.iter().enumerate() {
            if !e.alive {
                continue;
            }
            let rel = e.pos - self.player.pos;
            let t = rel.dot(dir);
            if t <= 0.15 || t > wall_d + 0.3 {
                continue;
            }
            let perp = (rel - dir * t).len();
            if perp < 0.38 && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, i));
            }
        }
        for (i, b) in self.barrels.iter().enumerate() {
            if !b.alive {
                continue;
            }
            let rel = b.pos - self.player.pos;
            let t = rel.dot(dir);
            if t <= 0.15 || t > wall_d + 0.3 {
                continue;
            }
            let perp = (rel - dir * t).len();
            if perp < 0.32 && best_barrel.is_none_or(|(bt, _)| t < bt) {
                best_barrel = Some((t, i));
            }
        }

        let barrel_closer = match (best, best_barrel) {
            (Some((et, _)), Some((bt, _))) => bt < et,
            (None, Some(_)) => true,
            _ => false,
        };
        if barrel_closer {
            let (_, i) = best_barrel.unwrap();
            self.barrels[i].health -= damage;
            return false;
        }
        if let Some((_, i)) = best {
            let pos = self.enemies[i].pos;
            let died = {
                let e = &mut self.enemies[i];
                e.health -= damage;
                if e.health <= 0 {
                    e.alive = false;
                    e.state = EnemyState::Dead;
                    e.dead_time = 0.0;
                    true
                } else {
                    e.state = EnemyState::Pain;
                    e.state_time = 0.22;
                    false
                }
            };
            if died {
                self.kills += 1;
                self.play_at(SoundId::ImpDeath, pos, 0.9);
            } else {
                self.play_at(SoundId::ImpPain, pos, 0.75);
            }
            return true;
        }
        false
    }

    fn update_enemies(&mut self, dt: f32) {
        let speed_scale = self.difficulty.enemy_speed();
        let cooldown_scale = self.difficulty.enemy_cooldown();
        let map = &self.map;
        let player_pos = self.player.pos;
        let playing = self.state == GameState::Playing;
        let mut damage_to_player = 0;
        let mut shots = Vec::new();
        let mut sounds: Vec<(SoundId, Vec2)> = Vec::new();

        for e in self.enemies.iter_mut() {
            if !e.alive {
                continue;
            }
            e.cooldown = (e.cooldown - dt).max(0.0);
            if !playing {
                continue;
            }
            let to_player = player_pos - e.pos;
            let dist = to_player.len();
            let los = map.line_of_sight(e.pos, player_pos);

            match e.state {
                EnemyState::Idle => {
                    if los && dist < 14.0 {
                        e.state = EnemyState::Chase;
                        e.anim = 0.0;
                        sounds.push((SoundId::ImpSight, e.pos));
                    }
                }
                EnemyState::Pain => {
                    e.state_time -= dt;
                    if e.state_time <= 0.0 {
                        e.state = EnemyState::Chase;
                    }
                }
                EnemyState::Attack => {
                    e.state_time -= dt;
                    e.anim += dt;
                    if e.state_time <= 0.0 {
                        e.state = EnemyState::Chase;
                    }
                }
                EnemyState::Chase => {
                    e.anim += dt * 1.6;
                    if !los {
                        // Wander toward the last known player position.
                        let dir = to_player.norm();
                        map.try_move(&mut e.pos, dir * 1.0 * speed_scale * dt, 0.3);
                    } else if dist > 1.35 {
                        let dir = to_player.norm();
                        map.try_move(&mut e.pos, dir * 1.75 * speed_scale * dt, 0.3);
                    }
                    if e.cooldown <= 0.0 && los {
                        if dist < 1.6 {
                            damage_to_player += 9;
                            e.cooldown = 1.1 * cooldown_scale;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.32;
                            sounds.push((SoundId::ImpAttack, e.pos));
                        } else if dist < 10.0 {
                            let aim = to_player.norm();
                            shots.push((e.pos + aim * 0.35, aim * 5.2));
                            e.cooldown = 2.0 * cooldown_scale;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.4;
                            sounds.push((SoundId::ImpAttack, e.pos));
                        }
                    }
                }
                EnemyState::Dead => {}
            }
        }

        for (pos, vel) in shots {
            self.projectiles.push(Projectile {
                pos,
                vel,
                life: 4.0,
            });
        }
        for (id, pos) in sounds {
            self.play_at(id, pos, 0.8);
        }
        if damage_to_player > 0 {
            self.damage_player(damage_to_player);
        }
    }

    fn update_projectiles(&mut self, dt: f32) {
        let player_pos = self.player.pos;
        let playing = self.state == GameState::Playing;
        let map = &self.map;
        let mut damage = 0;
        self.projectiles.retain_mut(|p| {
            p.pos += p.vel * dt;
            p.life -= dt;
            if p.life <= 0.0 {
                return false;
            }
            if map.blocks_sight(p.pos.x.floor() as i32, p.pos.y.floor() as i32) {
                return false;
            }
            if playing && p.pos.dist(player_pos) < 0.45 {
                damage += 11;
                return false;
            }
            true
        });
        if damage > 0 {
            self.damage_player(damage);
        }
    }

    fn update_pickups(&mut self) {
        let pos = self.player.pos;
        let bonus = self.difficulty.pickup_bonus();
        // Health and ammo per pickup, scaled by skill but never below one unit.
        let medkit = ((25.0 * bonus).round() as i32).max(1);
        let clip = ((20.0 * bonus).round() as i32).max(1);
        let shells = ((8.0 * bonus).round() as i32).max(1);
        let mut taken: Vec<usize> = Vec::new();
        let mut messages: Vec<&'static str> = Vec::new();
        for i in 0..self.pickups.len() {
            if self.pickups[i].taken || self.pickups[i].pos.dist(pos) > 0.6 {
                continue;
            }
            match self.pickups[i].kind {
                PickupKind::Medkit => {
                    if self.player.health >= 100 {
                        continue;
                    }
                    self.player.health = (self.player.health + medkit).min(100);
                    messages.push("PICKED UP A MEDKIT");
                }
                PickupKind::Ammo => {
                    if self.player.bullets >= 200 {
                        continue;
                    }
                    self.player.bullets = (self.player.bullets + clip).min(200);
                    messages.push("PICKED UP A CLIP");
                }
                PickupKind::Shells => {
                    if self.player.shells >= 60 {
                        continue;
                    }
                    self.player.shells = (self.player.shells + shells).min(60);
                    messages.push("PICKED UP SHELLS");
                }
                PickupKind::Shotgun => {
                    self.player.has_shotgun = true;
                    self.player.weapon = Weapon::Shotgun;
                    self.player.shells = (self.player.shells + shells).min(60);
                    messages.push("YOU GOT THE SHOTGUN!");
                }
            }
            taken.push(i);
        }
        for &i in &taken {
            self.pickups[i].taken = true;
            let pos = self.pickups[i].pos;
            self.play_at(SoundId::Pickup, pos, 0.75);
        }
        for m in messages {
            self.message(m);
        }
    }

    fn update_barrels(&mut self, dt: f32) {
        for b in &mut self.barrels {
            if b.alive && b.health <= 0 {
                b.alive = false;
                b.fade = 0.0;
            }
            if !b.alive {
                b.fade += dt;
            }
        }
        let exploded: Vec<Vec2> = self
            .barrels
            .iter()
            .filter(|b| !b.alive && b.fade < dt * 1.5)
            .map(|b| b.pos)
            .collect();
        for pos in exploded {
            self.explode(pos);
        }
    }

    fn explode(&mut self, pos: Vec2) {
        let player_dist = pos.dist(self.player.pos);
        self.play_at(SoundId::Explosion, pos, 1.0);
        let mut killed = Vec::new();
        for e in &mut self.enemies {
            if e.alive && e.pos.dist(pos) < 2.6 {
                e.health -= 55;
                if e.health <= 0 {
                    e.alive = false;
                    e.state = EnemyState::Dead;
                    e.dead_time = 0.0;
                    killed.push(e.pos);
                } else {
                    e.state = EnemyState::Pain;
                    e.state_time = 0.3;
                }
            }
        }
        self.kills += killed.len();
        for pos in killed {
            self.play_at(SoundId::ImpDeath, pos, 0.85);
        }
        for b in &mut self.barrels {
            if b.alive && b.pos.dist(pos) < 2.2 && b.pos.dist(pos) > 0.01 {
                b.health = 0;
            }
        }
        if player_dist < 2.4 {
            let dmg = (26.0 * (1.0 - player_dist / 2.4)) as i32;
            self.damage_player(dmg.max(2));
        }
    }

    fn damage_player(&mut self, amount: i32) {
        if self.state != GameState::Playing {
            return;
        }
        let amount = ((amount as f32) * self.difficulty.damage_taken()).round() as i32;
        let amount = amount.max(1);
        self.player.health -= amount;
        self.player.hurt = (self.player.hurt + amount as f32 / 26.0).min(1.0);
        let pos = self.player.pos;
        self.play_at(SoundId::PlayerHurt, pos, 1.0);
        if self.player.health <= 0 {
            self.player.health = 0;
            self.state = GameState::Dead;
            self.map_open = false;
        }
    }

    /// Billboard sprites for everything in the world, sorted far to near by the renderer.
    pub fn sprites(&self) -> Vec<Sprite> {
        let mut out = Vec::new();
        for e in &self.enemies {
            out.push(Sprite {
                pos: e.pos,
                tex: e.sprite(),
                w: 0.78,
                h: 0.92,
                z: 0.0,
            });
        }
        for b in &self.barrels {
            if b.alive {
                out.push(Sprite {
                    pos: b.pos,
                    tex: SPR_BARREL,
                    w: 0.55,
                    h: 0.72,
                    z: 0.0,
                });
            }
        }
        for p in &self.pickups {
            if p.taken {
                continue;
            }
            let (tex, size) = match p.kind {
                PickupKind::Medkit => (SPR_MEDKIT, 0.42),
                PickupKind::Ammo => (SPR_AMMO, 0.38),
                PickupKind::Shells => (SPR_SHELLS, 0.34),
                PickupKind::Shotgun => (SPR_SHOTGUN_PICKUP, 0.5),
            };
            let bob = (self.time * 2.0 + p.pos.x).sin() * 0.01;
            out.push(Sprite {
                pos: p.pos,
                tex,
                w: size,
                h: size,
                z: 0.02 + bob,
            });
        }
        for p in &self.projectiles {
            out.push(Sprite {
                pos: p.pos,
                tex: SPR_FIREBALL,
                w: 0.28,
                h: 0.28,
                z: 0.45,
            });
        }
        out
    }

    /// Used by the HUD minimap.
    pub fn cell_color(&self, cell: Cell) -> (u8, u8, u8) {
        match cell {
            Cell::Empty => (14, 14, 16),
            Cell::Wall(TEX_EXIT) => (70, 220, 110),
            Cell::Wall(TEX_DOOR) => (200, 170, 50),
            Cell::Wall(_) => (104, 110, 122),
            Cell::Door(_) => (200, 170, 50),
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn freeze_enemies(g: &mut Game) {
        for e in &mut g.enemies {
            e.cooldown = 999.0;
        }
    }

    #[test]
    fn spawning_reads_the_level() {
        let g = Game::new(0);
        assert_eq!(g.total_enemies, 12);
        assert_eq!(g.player.health, 100);
        assert_eq!(g.player.pos, Vec2::new(3.5, 2.5));
        assert!(g.barrels.iter().any(|b| (b.pos.x - 4.5).abs() < 0.01));
    }

    #[test]
    fn player_cannot_walk_through_walls() {
        let mut g = Game::new(0);
        freeze_enemies(&mut g);
        let mut input = Input {
            back: true,
            ..Default::default()
        };
        for _ in 0..120 {
            g.update(1.0 / 60.0, &input);
        }
        assert!(g.player.pos.x > 1.0 && g.player.pos.x < 2.5);
        input.back = false;
        input.forward = true;
        for _ in 0..120 {
            g.update(1.0 / 60.0, &input);
        }
        assert!(
            g.player.pos.x < 7.0,
            "walked through the east wall at {}",
            g.player.pos.x
        );
    }

    #[test]
    fn shooting_kills_an_imp() {
        let mut g = Game::new(0);
        // Stand right in front of the first imp and fire until it dies.
        g.player.pos = Vec2::new(17.0, 2.5);
        g.player.angle = 0.0;
        let before = g.kills;
        let input = Input {
            fire: true,
            ..Default::default()
        };
        for _ in 0..90 {
            g.update(1.0 / 60.0, &input);
        }
        assert!(g.kills > before, "pistol did not kill the imp in front");
        assert!(g.player.bullets < 50, "ammo was not consumed");
    }

    #[test]
    fn shotgun_pickup_switches_weapon() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(21.5, 17.5);
        g.update(1.0 / 60.0, &Input::default());
        assert!(g.player.has_shotgun);
        assert_eq!(g.player.weapon, Weapon::Shotgun);
        assert!(g.player.shells >= 8);
    }

    #[test]
    fn doors_open_when_used() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(6.6, 5.5);
        g.player.angle = 0.0;
        g.update(
            1.0 / 60.0,
            &Input {
                use_pressed: true,
                ..Default::default()
            },
        );
        assert!(g.map.door_open(7, 5) > 0.0);
        for _ in 0..80 {
            g.update(1.0 / 60.0, &Input::default());
        }
        assert!(!g.map.is_solid(7, 5));
    }

    #[test]
    fn exit_switch_wins_the_level() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(24.5, 22.4);
        g.player.angle = std::f32::consts::FRAC_PI_2;
        g.update(
            1.0 / 60.0,
            &Input {
                use_pressed: true,
                ..Default::default()
            },
        );
        assert_eq!(g.state, GameState::Won);
    }

    #[test]
    fn imps_hurt_the_player() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(19.5, 2.5);
        g.player.angle = std::f32::consts::PI;
        for _ in 0..600 {
            g.update(1.0 / 60.0, &Input::default());
            if g.player.health < 100 {
                break;
            }
        }
        assert!(g.player.health < 100, "imp never attacked");
    }

    #[test]
    fn shooting_raises_a_gunshot_sound() {
        let mut g = Game::new(0);
        let _ = g.take_sounds();
        g.update(
            1.0 / 60.0,
            &Input {
                fire: true,
                ..Default::default()
            },
        );
        let sounds = g.take_sounds();
        assert!(
            sounds.iter().any(|s| s.id == SoundId::Pistol),
            "no pistol sound emitted"
        );
        assert!(g.sounds.is_empty(), "take_sounds should drain the queue");
    }

    #[test]
    fn doors_raise_open_and_close_sounds() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(6.6, 5.5);
        g.player.angle = 0.0;
        let _ = g.take_sounds();
        g.update(
            1.0 / 60.0,
            &Input {
                use_pressed: true,
                ..Default::default()
            },
        );
        assert!(
            g.take_sounds().iter().any(|s| s.id == SoundId::DoorOpen),
            "no door sound on use"
        );
        // Run until the door has finished its open timer and closed again.
        let mut closed = false;
        for _ in 0..600 {
            g.update(1.0 / 60.0, &Input::default());
            if g.take_sounds().iter().any(|s| s.id == SoundId::DoorClose) {
                closed = true;
                break;
            }
        }
        assert!(closed, "no door-close sound after the auto-close timer");
    }

    #[test]
    fn imps_growl_when_they_spot_the_player() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(17.2, 2.5);
        let _ = g.take_sounds();
        g.update(1.0 / 60.0, &Input::default());
        let sounds = g.take_sounds();
        assert!(
            sounds.iter().any(|s| s.id == SoundId::ImpSight),
            "imp did not growl on sight"
        );
        for s in sounds {
            assert!(s.gain > 0.0 && s.pan >= -1.0 && s.pan <= 1.0);
        }
    }

    #[test]
    fn every_level_loads_with_entities() {
        for (i, level) in LEVELS.iter().enumerate() {
            let g = Game::new(i);
            assert_eq!(g.level_name(), level.name);
            assert!(g.total_enemies > 0, "{} has no enemies", level.name);
            assert_eq!(g.player.health, 100);
            assert_eq!(g.level, i);
        }
    }

    #[test]
    fn restart_keeps_the_current_level() {
        let mut g = Game::new(1);
        g.update(
            1.0 / 60.0,
            &Input {
                restart: true,
                ..Default::default()
            },
        );
        assert_eq!(g.level, 1);
        assert_eq!(g.state, GameState::Playing);
    }

    #[test]
    fn barrels_explode_with_a_sound() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(3.4, 4.5);
        g.player.angle = 0.0;
        let _ = g.take_sounds();
        let input = Input {
            fire: true,
            ..Default::default()
        };
        let mut heard = Vec::new();
        for _ in 0..150 {
            g.update(1.0 / 60.0, &input);
            heard.extend(g.take_sounds());
        }
        assert!(g.barrels.iter().any(|b| !b.alive), "barrel survived");
        assert!(
            heard.iter().any(|s| s.id == SoundId::Explosion),
            "no explosion sound"
        );
    }

    #[test]
    fn pickups_and_empty_clicks_are_audible() {
        let mut g = Game::new(0);
        g.player.pos = Vec2::new(21.5, 17.5);
        let _ = g.take_sounds();
        g.update(1.0 / 60.0, &Input::default());
        assert!(
            g.take_sounds().iter().any(|s| s.id == SoundId::Pickup),
            "no pickup sound"
        );

        g.player.weapon = Weapon::Pistol;
        g.player.bullets = 0;
        g.player.cooldown = 0.0;
        g.update(
            1.0 / 60.0,
            &Input {
                fire: true,
                ..Default::default()
            },
        );
        assert!(
            g.take_sounds().iter().any(|s| s.id == SoundId::Empty),
            "no empty-click sound"
        );
    }

    #[test]
    fn difficulty_scales_demons_and_damage() {
        let easy = Game::with_difficulty(0, Difficulty::Easy);
        let hard = Game::with_difficulty(0, Difficulty::Hard);
        assert!(easy.enemies[0].health < hard.enemies[0].health);
        assert_eq!(hard.enemies[0].health, Difficulty::Hard.enemy_health());
        assert_eq!(hard.difficulty, Difficulty::Hard);

        let mut easy = Game::with_difficulty(0, Difficulty::Easy);
        let mut hard = Game::with_difficulty(0, Difficulty::Hard);
        easy.damage_player(20);
        hard.damage_player(20);
        assert!(
            easy.player.health > hard.player.health,
            "easy {} vs hard {}",
            easy.player.health,
            hard.player.health
        );
    }

    /// Emp: pistol shots needed to drop the first imp on the entryway row.
    fn shots_to_kill(difficulty: Difficulty) -> usize {
        let mut g = Game::with_difficulty(0, difficulty);
        g.player.pos = Vec2::new(17.0, 2.5);
        g.player.angle = 0.0;
        let target = g
            .enemies
            .iter()
            .filter(|e| (e.pos.y - 2.5).abs() < 0.6)
            .map(|e| e.pos)
            .min_by(|a, b| a.x.total_cmp(&b.x))
            .expect("an imp straight ahead");
        g.enemies.retain(|e| e.pos == target);

        let mut shots = 0;
        while g.enemies.iter().any(|e| e.alive) && shots < 20 {
            g.hitscan(Vec2::new(1.0, 0.0), 14);
            shots += 1;
        }
        shots
    }

    #[test]
    fn harder_skills_take_more_shots() {
        assert!(
            shots_to_kill(Difficulty::Easy) < shots_to_kill(Difficulty::Hard),
            "hard should need more pistol hits"
        );
    }

    #[test]
    fn easy_spawns_fewer_demons_and_hard_has_less_supply() {
        let easy = Game::with_difficulty(0, Difficulty::Easy);
        let normal = Game::with_difficulty(0, Difficulty::Normal);
        let hard = Game::with_difficulty(0, Difficulty::Hard);

        assert!(
            easy.total_enemies < normal.total_enemies,
            "easy {} vs normal {}",
            easy.total_enemies,
            normal.total_enemies
        );
        assert_eq!(normal.total_enemies, hard.total_enemies);
        assert_eq!(normal.enemies.len(), normal.total_enemies);

        assert!(
            hard.pickups.len() < normal.pickups.len(),
            "hard {} vs normal {}",
            hard.pickups.len(),
            normal.pickups.len()
        );
        assert_eq!(easy.pickups.len(), normal.pickups.len());
        for g in [&easy, &normal, &hard] {
            assert!(
                g.pickups.iter().any(|p| p.kind == PickupKind::Shotgun),
                "the shotgun must always spawn"
            );
        }
    }

    #[test]
    fn pickup_values_scale_with_skill() {
        fn heal_on(difficulty: Difficulty) -> i32 {
            let mut g = Game::with_difficulty(0, difficulty);
            let pos = g
                .pickups
                .iter()
                .find(|p| p.kind == PickupKind::Medkit)
                .expect("a medkit on the entryway")
                .pos;
            g.player.health = 40;
            g.player.pos = pos;
            g.update(1.0 / 60.0, &Input::default());
            g.player.health - 40
        }
        assert!(
            heal_on(Difficulty::Easy) > heal_on(Difficulty::Hard),
            "easy medkits should heal more than hard ones"
        );
        assert!(Difficulty::Easy.pickup_bonus() > Difficulty::Hard.pickup_bonus());
        assert!(Difficulty::Hard.pickup_bonus() > 0.0);
    }

    #[test]
    fn restart_keeps_the_difficulty() {
        let mut g = Game::with_difficulty(1, Difficulty::Hard);
        g.update(
            1.0 / 60.0,
            &Input {
                restart: true,
                ..Default::default()
            },
        );
        assert_eq!(g.difficulty, Difficulty::Hard);
        assert_eq!(g.level, 1);
        assert_eq!(g.enemies[0].health, Difficulty::Hard.enemy_health());
    }

    #[test]
    fn difficulty_names_and_indices_match() {
        for (i, d) in Difficulty::ALL.iter().enumerate() {
            assert_eq!(d.index(), i);
            assert_eq!(Difficulty::from_index(i), *d);
            assert!(!d.name().is_empty() && !d.short().is_empty());
        }
        assert_eq!(Difficulty::from_index(99), Difficulty::Hard);
    }

    #[test]
    fn restart_resets_the_game() {
        let mut g = Game::new(0);
        g.player.health = 1;
        g.update(
            1.0 / 60.0,
            &Input {
                restart: true,
                ..Default::default()
            },
        );
        assert_eq!(g.player.health, 100);
        assert_eq!(g.state, GameState::Playing);
        assert_eq!(g.difficulty, Difficulty::Normal);
    }
}

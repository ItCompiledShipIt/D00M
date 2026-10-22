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

    /// Multiplier on each monster's own health.
    pub fn health_scale(self) -> f32 {
        match self {
            Difficulty::Easy => 0.75,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.4,
        }
    }

    pub fn enemy_speed(self) -> f32 {
        match self {
            Difficulty::Easy => 0.8,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.25,
        }
    }

    /// Multiplier on how long monsters wait between attacks.
    pub fn enemy_cooldown(self) -> f32 {
        match self {
            Difficulty::Easy => 1.35,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.72,
        }
    }

    /// Whether the nth monster in a map spawns at all. Easy throws a third
    /// fewer of them at you; both other skills use the whole map.
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
    Chaingun,
    Launcher,
}

impl Weapon {
    pub const ALL: [Weapon; 4] = [
        Weapon::Pistol,
        Weapon::Shotgun,
        Weapon::Chaingun,
        Weapon::Launcher,
    ];

    pub fn index(self) -> usize {
        match self {
            Weapon::Pistol => 0,
            Weapon::Shotgun => 1,
            Weapon::Chaingun => 2,
            Weapon::Launcher => 3,
        }
    }

    pub fn from_index(index: usize) -> Weapon {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    pub fn name(self) -> &'static str {
        match self {
            Weapon::Pistol => "PISTOL",
            Weapon::Shotgun => "SHOTGUN",
            Weapon::Chaingun => "CHAINGUN",
            Weapon::Launcher => "LAUNCHER",
        }
    }

    fn cooldown(self) -> f32 {
        match self {
            Weapon::Pistol => 0.30,
            Weapon::Shotgun => 0.82,
            Weapon::Chaingun => 0.10,
            Weapon::Launcher => 0.95,
        }
    }

    fn pellets(self) -> usize {
        match self {
            Weapon::Pistol => 1,
            Weapon::Shotgun => 7,
            Weapon::Chaingun => 1,
            Weapon::Launcher => 0,
        }
    }

    fn spread(self) -> f32 {
        match self {
            Weapon::Pistol => 0.012,
            Weapon::Shotgun => 0.085,
            Weapon::Chaingun => 0.045,
            Weapon::Launcher => 0.0,
        }
    }

    fn damage(self) -> i32 {
        match self {
            Weapon::Pistol => 14,
            Weapon::Shotgun => 8,
            Weapon::Chaingun => 9,
            Weapon::Launcher => 0,
        }
    }

    fn sound(self) -> SoundId {
        match self {
            Weapon::Pistol => SoundId::Pistol,
            Weapon::Shotgun => SoundId::Shotgun,
            Weapon::Chaingun => SoundId::Chaingun,
            Weapon::Launcher => SoundId::RocketLaunch,
        }
    }

    /// Which ammo pool the weapon draws from.
    pub fn ammo_label(self) -> &'static str {
        match self {
            Weapon::Pistol | Weapon::Chaingun => "BULLETS",
            Weapon::Shotgun => "SHELLS",
            Weapon::Launcher => "ROCKETS",
        }
    }

    pub fn tex(self, firing: bool) -> usize {
        match (self, firing) {
            (Weapon::Pistol, false) => WEAPON_PISTOL,
            (Weapon::Pistol, true) => WEAPON_PISTOL_FIRE,
            (Weapon::Shotgun, false) => WEAPON_SHOTGUN,
            (Weapon::Shotgun, true) => WEAPON_SHOTGUN_FIRE,
            (Weapon::Chaingun, false) => WEAPON_CHAINGUN,
            (Weapon::Chaingun, true) => WEAPON_CHAINGUN_FIRE,
            (Weapon::Launcher, false) => WEAPON_LAUNCHER,
            (Weapon::Launcher, true) => WEAPON_LAUNCHER_FIRE,
        }
    }
}

pub struct Player {
    pub pos: Vec2,
    pub angle: f32,
    pub health: i32,
    /// Armour points: absorb a third of incoming damage until spent.
    pub armour: i32,
    pub bullets: i32,
    pub shells: i32,
    pub rockets: i32,
    pub weapon: Weapon,
    pub has_shotgun: bool,
    pub has_chaingun: bool,
    pub has_launcher: bool,
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
            Weapon::Pistol | Weapon::Chaingun => self.bullets,
            Weapon::Shotgun => self.shells,
            Weapon::Launcher => self.rockets,
        }
    }

    /// Whether the player owns the weapon at all.
    pub fn owns(&self, weapon: Weapon) -> bool {
        match weapon {
            Weapon::Pistol => true,
            Weapon::Shotgun => self.has_shotgun,
            Weapon::Chaingun => self.has_chaingun,
            Weapon::Launcher => self.has_launcher,
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

/// The kinds of demon in the episode, and the thing waiting at the end of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    /// Original fireball-throwing imp.
    Imp,
    /// Fast, tough, melee-only charger.
    Demon,
    /// Slow, heavily armoured, throws green hellfire.
    Knight,
    /// The Gatekeeper: the boss holding the final exit. Fires hellfire in
    /// threes and fights faster once it is half dead.
    Boss,
}

impl EnemyKind {
    pub const ALL: [EnemyKind; 4] = [
        EnemyKind::Imp,
        EnemyKind::Demon,
        EnemyKind::Knight,
        EnemyKind::Boss,
    ];

    pub fn index(self) -> usize {
        match self {
            EnemyKind::Imp => 0,
            EnemyKind::Demon => 1,
            EnemyKind::Knight => 2,
            EnemyKind::Boss => 3,
        }
    }

    pub fn from_index(index: usize) -> EnemyKind {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    pub fn name(self) -> &'static str {
        match self {
            EnemyKind::Imp => "IMP",
            EnemyKind::Demon => "DEMON",
            EnemyKind::Knight => "HELL KNIGHT",
            EnemyKind::Boss => "THE GATEKEEPER",
        }
    }

    fn base_health(self) -> i32 {
        match self {
            EnemyKind::Imp => 40,
            EnemyKind::Demon => 80,
            EnemyKind::Knight => 120,
            EnemyKind::Boss => 600,
        }
    }

    /// Damage this kind actually takes from an incoming hit of `amount`.
    /// The hell knight's armour turns aside a quarter of every blow,
    /// rounded up so even a single pistol round still stings for 1.
    fn damage_taken(self, amount: i32) -> i32 {
        match self {
            EnemyKind::Knight => (amount * 3 + 3) / 4,
            _ => amount,
        }
    }

    /// The sound this kind makes when it dies.
    fn death_sound(self) -> SoundId {
        match self {
            EnemyKind::Imp => SoundId::ImpDeath,
            EnemyKind::Demon => SoundId::DemonDeath,
            EnemyKind::Knight => SoundId::KnightDeath,
            EnemyKind::Boss => SoundId::BossDeath,
        }
    }

    /// The sound this kind makes when it spots the player.
    fn sight_sound(self) -> SoundId {
        match self {
            EnemyKind::Imp => SoundId::ImpSight,
            EnemyKind::Demon => SoundId::DemonSight,
            EnemyKind::Knight => SoundId::KnightSight,
            EnemyKind::Boss => SoundId::BossSight,
        }
    }

    /// The sound this kind makes when a hit lands on it.
    fn pain_sound(self) -> SoundId {
        match self {
            EnemyKind::Imp => SoundId::ImpPain,
            EnemyKind::Demon => SoundId::DemonPain,
            EnemyKind::Knight => SoundId::KnightPain,
            EnemyKind::Boss => SoundId::BossPain,
        }
    }

    /// The sound this kind makes when it attacks.
    fn attack_sound(self) -> SoundId {
        match self {
            EnemyKind::Imp => SoundId::ImpAttack,
            EnemyKind::Demon => SoundId::DemonAttack,
            EnemyKind::Knight => SoundId::KnightAttack,
            EnemyKind::Boss => SoundId::BossAttack,
        }
    }

    /// Chasing speed in cells per second.
    fn speed(self) -> f32 {
        match self {
            EnemyKind::Imp => 1.75,
            EnemyKind::Demon => 2.75,
            EnemyKind::Knight => 1.25,
            EnemyKind::Boss => 1.15,
        }
    }

    fn melee_reach(self) -> f32 {
        match self {
            EnemyKind::Imp => 1.6,
            EnemyKind::Demon => 1.35,
            EnemyKind::Knight => 1.5,
            EnemyKind::Boss => 1.9,
        }
    }

    fn melee_damage(self) -> i32 {
        match self {
            EnemyKind::Imp => 9,
            EnemyKind::Demon => 14,
            EnemyKind::Knight => 11,
            EnemyKind::Boss => 24,
        }
    }

    fn attack_cooldown(self) -> f32 {
        match self {
            EnemyKind::Imp => 1.1,
            EnemyKind::Demon => 0.85,
            EnemyKind::Knight => 1.5,
            EnemyKind::Boss => 1.3,
        }
    }

    /// Projectile speed and damage, and the range it starts firing at.
    fn projectile(self) -> Option<(f32, i32, f32, usize)> {
        match self {
            EnemyKind::Imp => Some((5.2, 11, 10.0, SPR_FIREBALL)),
            EnemyKind::Demon => None,
            EnemyKind::Knight => Some((4.0, 18, 12.0, SPR_HELLFIRE)),
            EnemyKind::Boss => Some((4.6, 16, 13.0, SPR_HELLFIRE)),
        }
    }

    fn size(self) -> (f32, f32) {
        match self {
            EnemyKind::Imp => (0.78, 0.92),
            EnemyKind::Demon => (0.95, 1.02),
            EnemyKind::Knight => (0.88, 1.12),
            EnemyKind::Boss => (1.45, 1.65),
        }
    }

    fn walk(self, anim: f32) -> usize {
        let frame = (anim * 6.0) as usize;
        match self {
            EnemyKind::Imp => {
                const WALK: [usize; 4] =
                    [SPR_IMP_WALK0, SPR_IMP_WALK1, SPR_IMP_WALK2, SPR_IMP_WALK3];
                WALK[frame % 4]
            }
            EnemyKind::Demon => [SPR_DEMON_WALK0, SPR_DEMON_WALK1][frame % 2],
            EnemyKind::Knight => [SPR_KNIGHT_WALK0, SPR_KNIGHT_WALK1][frame % 2],
            EnemyKind::Boss => [SPR_BOSS_WALK0, SPR_BOSS_WALK1][frame % 2],
        }
    }

    fn attack_sprite(self) -> usize {
        match self {
            EnemyKind::Imp => SPR_IMP_ATTACK,
            EnemyKind::Demon => SPR_DEMON_ATTACK,
            EnemyKind::Knight => SPR_KNIGHT_ATTACK,
            EnemyKind::Boss => SPR_BOSS_ATTACK,
        }
    }

    fn pain_sprite(self) -> usize {
        match self {
            EnemyKind::Imp => SPR_IMP_PAIN,
            EnemyKind::Demon => SPR_DEMON_PAIN,
            EnemyKind::Knight => SPR_KNIGHT_PAIN,
            EnemyKind::Boss => SPR_BOSS_PAIN,
        }
    }

    fn corpse_sprite(self) -> usize {
        match self {
            EnemyKind::Imp => SPR_IMP_CORPSE,
            EnemyKind::Demon => SPR_DEMON_CORPSE,
            EnemyKind::Knight => SPR_KNIGHT_CORPSE,
            EnemyKind::Boss => SPR_BOSS_CORPSE,
        }
    }

    /// Death frames: imps have a two-step fall, the others drop straight down.
    fn dying_sprite(self, dead_time: f32) -> usize {
        match self {
            EnemyKind::Imp => {
                if dead_time < 0.15 {
                    SPR_IMP_DIE0
                } else if dead_time < 0.30 {
                    SPR_IMP_DIE1
                } else {
                    SPR_IMP_CORPSE
                }
            }
            other => other.corpse_sprite(),
        }
    }
}

pub struct Enemy {
    pub kind: EnemyKind,
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
            EnemyState::Dead => self.kind.dying_sprite(self.dead_time),
            EnemyState::Pain => self.kind.pain_sprite(),
            EnemyState::Attack => self.kind.attack_sprite(),
            _ => self.kind.walk(self.anim),
        }
    }

    pub fn size(&self) -> (f32, f32) {
        self.kind.size()
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
    Chaingun,
    Launcher,
    Rockets,
    Armour,
}

impl PickupKind {
    pub const ALL: [PickupKind; 8] = [
        PickupKind::Medkit,
        PickupKind::Ammo,
        PickupKind::Shells,
        PickupKind::Shotgun,
        PickupKind::Chaingun,
        PickupKind::Launcher,
        PickupKind::Rockets,
        PickupKind::Armour,
    ];

    pub fn index(self) -> usize {
        match self {
            PickupKind::Medkit => 0,
            PickupKind::Ammo => 1,
            PickupKind::Shells => 2,
            PickupKind::Shotgun => 3,
            PickupKind::Chaingun => 4,
            PickupKind::Launcher => 5,
            PickupKind::Rockets => 6,
            PickupKind::Armour => 7,
        }
    }

    pub fn from_index(index: usize) -> PickupKind {
        Self::ALL[index.min(Self::ALL.len() - 1)]
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
    pub secrets_found: usize,
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
    pub armour: i32,
    pub bullets: i32,
    pub shells: i32,
    pub rockets: i32,
    pub weapon: Weapon,
    pub has_shotgun: bool,
    pub has_chaingun: bool,
    pub has_launcher: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnemySnapshot {
    pub kind: EnemyKind,
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

#[derive(Clone, Copy)]
pub struct Projectile {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub damage: i32,
    pub sprite: usize,
}

/// A short-lived explosion flash left behind by a blast.
pub struct Burst {
    pub pos: Vec2,
    pub age: f32,
}

const BURST_LIFE: f32 = 0.5;
const BURST_FRAMES: usize = 3;

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
    pub weapon3: bool,
    pub weapon4: bool,
    pub weapon_next: bool,
    pub weapon_prev: bool,
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
    /// Rockets fired by the player, still in the air.
    pub player_rockets: Vec<Projectile>,
    /// Explosion flashes and smoke still fading.
    pub bursts: Vec<Burst>,
    pub messages: Vec<(String, f32)>,
    pub state: GameState,
    pub time: f32,
    pub kills: usize,
    pub total_enemies: usize,
    /// Secret niches found / hidden on this level.
    pub secrets_found: usize,
    pub secrets_total: usize,
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
        let mut enemy_count = 0usize;
        let mut pickup_count = 0usize;
        let mut secret_count = 0usize;

        for (y, row) in LEVELS[level].rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let pos = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                match ch {
                    'P' => spawn = pos,
                    'i' | 'd' | 'k' | 'B' => {
                        // The Gatekeeper always spawns: the finale gate
                        // would be pointless without him.
                        if ch == 'B' || difficulty.spawns_enemy(enemy_count) {
                            let kind = match ch {
                                'd' => EnemyKind::Demon,
                                'k' => EnemyKind::Knight,
                                'B' => EnemyKind::Boss,
                                _ => EnemyKind::Imp,
                            };
                            let health = (kind.base_health() as f32 * difficulty.health_scale())
                                .round() as i32;
                            enemies.push(Enemy {
                                kind,
                                pos,
                                health,
                                state: EnemyState::Idle,
                                anim: 0.0,
                                cooldown: 0.6,
                                state_time: 0.0,
                                dead_time: 0.0,
                                alive: true,
                            });
                        }
                        enemy_count += 1;
                    }
                    'b' => barrels.push(Barrel {
                        pos,
                        health: 22,
                        alive: true,
                        fade: 0.0,
                    }),
                    'm' | 'a' | 's' | 'v' => {
                        if difficulty.spawns_pickup(pickup_count) {
                            let kind = match ch {
                                'm' => PickupKind::Medkit,
                                'a' => PickupKind::Ammo,
                                'v' => PickupKind::Armour,
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
                    // Weapon pickups always spawn: without them the harder
                    // skills would be unwinnable rather than hard.
                    'g' | 'c' | 'L' => {
                        let kind = match ch {
                            'c' => PickupKind::Chaingun,
                            'L' => PickupKind::Launcher,
                            _ => PickupKind::Shotgun,
                        };
                        pickups.push(Pickup {
                            pos,
                            kind,
                            taken: false,
                        });
                    }
                    'r' => {
                        if difficulty.spawns_pickup(pickup_count) {
                            pickups.push(Pickup {
                                pos,
                                kind: PickupKind::Rockets,
                                taken: false,
                            });
                        }
                        pickup_count += 1;
                    }
                    // A secret niche: the wall itself is the container
                    // (see map.rs), and its medkit stays hidden until the
                    // niche is opened. Always spawns, like a weapon.
                    'x' => {
                        secret_count += 1;
                        pickups.push(Pickup {
                            pos,
                            kind: PickupKind::Medkit,
                            taken: false,
                        });
                    }
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
                armour: 0,
                bullets: 50,
                shells: 0,
                rockets: 0,
                weapon: Weapon::Pistol,
                has_shotgun: false,
                has_chaingun: false,
                has_launcher: false,
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
            player_rockets: Vec::new(),
            bursts: Vec::new(),
            messages: Vec::new(),
            state: GameState::Playing,
            time: 0.0,
            kills: 0,
            total_enemies,
            secrets_found: 0,
            secrets_total: secret_count,
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
            secrets_found: self.secrets_found,
            player: PlayerSnapshot {
                pos: self.player.pos,
                angle: self.player.angle,
                health: self.player.health,
                armour: self.player.armour,
                bullets: self.player.bullets,
                shells: self.player.shells,
                rockets: self.player.rockets,
                weapon: self.player.weapon,
                has_shotgun: self.player.has_shotgun,
                has_chaingun: self.player.has_chaingun,
                has_launcher: self.player.has_launcher,
            },
            enemies: self
                .enemies
                .iter()
                .map(|e| EnemySnapshot {
                    kind: e.kind,
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
        game.secrets_found = s.secrets_found.min(game.secrets_total);
        game.player.pos = s.player.pos;
        game.player.angle = s.player.angle.rem_euclid(std::f32::consts::TAU);
        game.player.health = s.player.health.clamp(1, 100);
        game.player.armour = s.player.armour.clamp(0, 100);
        game.player.bullets = s.player.bullets.clamp(0, 200);
        game.player.shells = s.player.shells.clamp(0, 60);
        game.player.rockets = s.player.rockets.clamp(0, 50);
        game.player.weapon = s.player.weapon;
        game.player.has_shotgun =
            s.player.has_shotgun || matches!(s.player.weapon, Weapon::Shotgun);
        game.player.has_chaingun = s.player.has_chaingun || s.player.weapon == Weapon::Chaingun;
        game.player.has_launcher = s.player.has_launcher || s.player.weapon == Weapon::Launcher;
        if !game.player.owns(game.player.weapon) {
            game.player.weapon = Weapon::Pistol;
        }
        game.player.cooldown = 0.0;
        game.enemies = s
            .enemies
            .into_iter()
            .map(|e| Enemy {
                kind: e.kind,
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

    /// Age and retire explosion flashes. `update` calls this before the
    /// Playing-state gate, and the front end calls it while paused, so a
    /// blast never sticks on screen behind the pause menu or a corpse.
    pub fn age_bursts(&mut self, dt: f32) {
        for b in &mut self.bursts {
            b.age += dt;
        }
        self.bursts.retain(|b| b.age < BURST_LIFE);
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
        self.age_bursts(dt);
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
        self.update_player_rockets(dt);
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

    /// Keep the player from standing inside monsters and barrels.
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
        for (wanted, pressed) in [
            (Weapon::Pistol, input.weapon1),
            (Weapon::Shotgun, input.weapon2),
            (Weapon::Chaingun, input.weapon3),
            (Weapon::Launcher, input.weapon4),
        ] {
            if !pressed {
                continue;
            }
            if self.player.owns(wanted) {
                self.player.weapon = wanted;
                self.player.cooldown = self.player.cooldown.max(0.15);
            } else {
                self.message(format!("YOU NEED THE {}", wanted.name()));
            }
        }
        if input.weapon_next {
            self.cycle_weapon(1);
        }
        if input.weapon_prev {
            self.cycle_weapon(-1);
        }

        if input.use_pressed {
            match self.map.use_at(self.player.pos, self.player.angle) {
                UseOutcome::Opened => {
                    let pos = self.player.pos + Vec2::from_angle(self.player.angle) * 0.9;
                    self.play_at(SoundId::DoorOpen, pos, 0.9);
                }
                UseOutcome::Secret => {
                    self.secrets_found += 1;
                    self.message("A SECRET IS REVEALED");
                    let pos = self.player.pos + Vec2::from_angle(self.player.angle) * 0.9;
                    self.play_at(SoundId::DoorOpen, pos, 0.9);
                }
                UseOutcome::Exit => {
                    // The final exit stays sealed while the Gatekeeper lives.
                    let gate_held = self.level + 1 == LEVELS.len()
                        && self
                            .enemies
                            .iter()
                            .any(|e| e.alive && e.kind == EnemyKind::Boss);
                    if gate_held {
                        self.message("THE GATEKEEPER STILL LIVES");
                    } else {
                        self.state = GameState::Won;
                        self.message("EXIT ACTIVATED");
                    }
                }
                UseOutcome::None => {}
            }
        }

        if input.fire {
            self.try_fire();
        }
    }

    /// Step to the next owned weapon, used by the shoulder buttons.
    fn cycle_weapon(&mut self, dir: i32) {
        let owned: Vec<Weapon> = Weapon::ALL
            .iter()
            .copied()
            .filter(|w| self.player.owns(*w))
            .collect();
        if owned.len() < 2 {
            return;
        }
        let current = owned
            .iter()
            .position(|w| *w == self.player.weapon)
            .unwrap_or(0);
        let next = (current as i32 + dir).rem_euclid(owned.len() as i32) as usize;
        self.player.weapon = owned[next];
        self.player.cooldown = self.player.cooldown.max(0.15);
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
            Weapon::Pistol | Weapon::Chaingun => self.player.bullets -= 1,
            Weapon::Shotgun => self.player.shells -= 1,
            Weapon::Launcher => self.player.rockets -= 1,
        }
        self.sounds.push(SoundRequest {
            id: weapon.sound(),
            gain: 0.9,
            pan: 0.0,
        });
        self.player.cooldown = weapon.cooldown();
        self.player.fire_anim = 1.0;
        self.player.muzzle = 0.07;
        self.player.bob_phase += 0.12;

        let base = self.player.angle;
        if weapon == Weapon::Launcher {
            let dir = Vec2::from_angle(base);
            self.player_rockets.push(Projectile {
                pos: self.player.pos + dir * 0.45,
                vel: dir * 9.5,
                life: 6.0,
                damage: 0,
                sprite: SPR_ROCKET,
            });
            return;
        }
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
            let kind = self.enemies[i].kind;
            let died = {
                let e = &mut self.enemies[i];
                e.health -= kind.damage_taken(damage);
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
                if kind != EnemyKind::Imp {
                    self.message(format!("{} SLAIN", kind.name()));
                }
                self.play_at(kind.death_sound(), pos, 0.9);
                if kind == EnemyKind::Boss {
                    self.boss_falls(pos);
                }
            } else {
                self.play_at(kind.pain_sound(), pos, 0.75);
            }
            return true;
        }
        false
    }

    fn update_enemies(&mut self, dt: f32) {
        let speed_scale = self.difficulty.enemy_speed();
        let cooldown_scale = self.difficulty.enemy_cooldown();
        let boss_full = (EnemyKind::Boss.base_health() as f32 * self.difficulty.health_scale())
            .round() as i32;
        let map = &self.map;
        let player_pos = self.player.pos;
        let playing = self.state == GameState::Playing;
        let mut damage_to_player = 0;
        let mut shots: Vec<(Vec2, Vec2, i32, usize)> = Vec::new();
        let mut sounds: Vec<(SoundId, Vec2)> = Vec::new();
        let mut boss_woke = false;

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
                        if e.kind == EnemyKind::Boss {
                            boss_woke = true;
                        }
                        sounds.push((e.kind.sight_sound(), e.pos));
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
                    let kind = e.kind;
                    let reach = kind.melee_reach();
                    let dir = to_player.norm();
                    // Half-dead, the Gatekeeper fights in a fury.
                    let enraged = kind == EnemyKind::Boss && e.health * 2 <= boss_full;
                    let speed_mul = if enraged { 1.45 } else { 1.0 };
                    let cd_mul = if enraged { 0.55 } else { 1.0 };
                    if !los {
                        // Wander toward the last known player position.
                        map.try_move(
                            &mut e.pos,
                            dir * kind.speed() * speed_mul * 0.6 * speed_scale * dt,
                            0.3,
                        );
                    } else if dist > reach * 0.8 {
                        map.try_move(
                            &mut e.pos,
                            dir * kind.speed() * speed_mul * speed_scale * dt,
                            0.3,
                        );
                    }
                    if e.cooldown <= 0.0 && los {
                        if dist < reach {
                            damage_to_player += kind.melee_damage();
                            e.cooldown = kind.attack_cooldown() * cd_mul * cooldown_scale;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.32;
                            sounds.push((kind.attack_sound(), e.pos));
                        } else if let Some((speed, damage, range, sprite)) = kind.projectile()
                            && dist < range
                        {
                            if kind == EnemyKind::Boss {
                                // Hellfire in threes: a fan, not a single bolt.
                                for off in [-0.17f32, 0.0, 0.17] {
                                    let (s, c) = off.sin_cos();
                                    let fanned =
                                        Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
                                    shots.push((e.pos + fanned * 0.35, fanned * speed, damage, sprite));
                                }
                            } else {
                                shots.push((e.pos + dir * 0.35, dir * speed, damage, sprite));
                            }
                            e.cooldown = kind.attack_cooldown() * 2.0 * cd_mul * cooldown_scale;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.4;
                            sounds.push((kind.attack_sound(), e.pos));
                        }
                    }
                }
                EnemyState::Dead => {}
            }
        }

        for (pos, vel, damage, sprite) in shots {
            self.projectiles.push(Projectile {
                pos,
                vel,
                life: 4.0,
                damage,
                sprite,
            });
        }
        for (id, pos) in sounds {
            self.play_at(id, pos, 0.8);
        }
        if boss_woke {
            self.message("THE GATEKEEPER AWAKENS");
        }
        if damage_to_player > 0 {
            self.damage_player(damage_to_player);
        }
    }

    /// The Gatekeeper does not die quietly.
    fn boss_falls(&mut self, pos: Vec2) {
        self.play_at(SoundId::Explosion, pos, 1.0);
        for (dx, dy) in [(0.0, 0.0), (0.55, 0.2), (-0.5, -0.25), (0.15, -0.55)] {
            if self.bursts.len() < 24 {
                self.bursts.push(Burst {
                    pos: pos + Vec2::new(dx, dy),
                    age: 0.0,
                });
            }
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
                damage += p.damage;
                return false;
            }
            true
        });
        if damage > 0 {
            self.damage_player(damage);
        }
    }

    /// Fly the player's rockets forward, bursting on anything they meet.
    fn update_player_rockets(&mut self, dt: f32) {
        let mut rockets = std::mem::take(&mut self.player_rockets);
        let mut bursts: Vec<Vec2> = Vec::new();
        let map = &self.map;
        let enemies = &self.enemies;
        let barrels = &self.barrels;
        rockets.retain_mut(|r| {
            let step = r.vel * dt;
            r.pos += step;
            r.life -= dt;
            if r.life <= 0.0 {
                bursts.push(r.pos);
                return false;
            }
            let cx = r.pos.x.floor() as i32;
            let cy = r.pos.y.floor() as i32;
            if map.blocks_sight(cx, cy) {
                bursts.push(r.pos - step * 0.5);
                return false;
            }
            if enemies.iter().any(|e| e.alive && e.pos.dist(r.pos) < 0.5) {
                bursts.push(r.pos);
                return false;
            }
            if barrels.iter().any(|b| b.alive && b.pos.dist(r.pos) < 0.45) {
                bursts.push(r.pos);
                return false;
            }
            true
        });
        self.player_rockets = rockets;
        for pos in bursts {
            self.explode(pos, 3.4, 95);
        }
    }

    /// A pickup inside a still-sealed secret niche is neither visible
    /// nor collectible. Concealment is derived from the map, so a save
    /// game restores it exactly from the niche's saved openness.
    fn pickup_concealed(&self, p: &Pickup) -> bool {
        let (x, y) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32);
        matches!(self.map.cell(x, y), Cell::Secret(_)) && self.map.door_open(x, y) < 0.7
    }

    fn update_pickups(&mut self) {
        let pos = self.player.pos;
        let bonus = self.difficulty.pickup_bonus();
        // Health and ammo per pickup, scaled by skill but never below one unit.
        let medkit = ((25.0 * bonus).round() as i32).max(1);
        let clip = ((20.0 * bonus).round() as i32).max(1);
        let shells = ((8.0 * bonus).round() as i32).max(1);
        let rockets = ((4.0 * bonus).round() as i32).max(1);
        let mut taken: Vec<usize> = Vec::new();
        let mut messages: Vec<&'static str> = Vec::new();
        for i in 0..self.pickups.len() {
            if self.pickups[i].taken
                || self.pickups[i].pos.dist(pos) > 0.6
                || self.pickup_concealed(&self.pickups[i])
            {
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
                PickupKind::Armour => {
                    if self.player.armour >= 100 {
                        continue;
                    }
                    self.player.armour = 100;
                    messages.push("PICKED UP AN ARMOUR VEST");
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
                PickupKind::Chaingun => {
                    self.player.has_chaingun = true;
                    self.player.weapon = Weapon::Chaingun;
                    self.player.bullets = (self.player.bullets + clip * 2).min(200);
                    messages.push("YOU GOT THE CHAINGUN!");
                }
                PickupKind::Launcher => {
                    self.player.has_launcher = true;
                    self.player.weapon = Weapon::Launcher;
                    self.player.rockets = (self.player.rockets + 6).min(50);
                    messages.push("YOU GOT THE ROCKET LAUNCHER!");
                }
                PickupKind::Rockets => {
                    if self.player.rockets >= 50 {
                        continue;
                    }
                    self.player.rockets = (self.player.rockets + rockets).min(50);
                    messages.push("PICKED UP ROCKETS");
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
            self.explode(pos, 2.6, 55);
        }
    }

    fn explode(&mut self, pos: Vec2, radius: f32, damage: i32) {
        let player_dist = pos.dist(self.player.pos);
        self.play_at(SoundId::Explosion, pos, 1.0);
        if self.bursts.len() < 24 {
            self.bursts.push(Burst { pos, age: 0.0 });
        }
        let mut killed = Vec::new();
        for e in &mut self.enemies {
            if e.alive && e.pos.dist(pos) < radius {
                e.health -= e.kind.damage_taken(damage);
                if e.health <= 0 {
                    e.alive = false;
                    e.state = EnemyState::Dead;
                    e.dead_time = 0.0;
                    killed.push((e.pos, e.kind));
                } else {
                    e.state = EnemyState::Pain;
                    e.state_time = 0.3;
                }
            }
        }
        self.kills += killed.len();
        for (pos, kind) in killed {
            if kind != EnemyKind::Imp {
                self.message(format!("{} SLAIN", kind.name()));
            }
            self.play_at(kind.death_sound(), pos, 0.85);
            if kind == EnemyKind::Boss {
                self.boss_falls(pos);
            }
        }
        for b in &mut self.barrels {
            if b.alive && b.pos.dist(pos) < radius * 0.85 && b.pos.dist(pos) > 0.01 {
                b.health = 0;
            }
        }
        // The blast hurts the player far less than it hurts demons, so a
        // rocket fired at a wall you are standing next to is a bad idea but
        // not an instant death sentence.
        if player_dist < radius {
            let dmg = (damage as f32 * 0.30 * (1.0 - player_dist / radius)) as i32;
            self.damage_player(dmg.max(2));
        }
    }

    fn damage_player(&mut self, amount: i32) {
        if self.state != GameState::Playing {
            return;
        }
        let amount = ((amount as f32) * self.difficulty.damage_taken()).round() as i32;
        let amount = amount.max(1);
        // Armour eats a third of every hit until it is spent.
        let absorbed = if self.player.armour > 0 {
            (amount / 3).min(self.player.armour)
        } else {
            0
        };
        self.player.armour -= absorbed;
        let amount = amount - absorbed;
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
            let (w, h) = e.size();
            out.push(Sprite {
                pos: e.pos,
                tex: e.sprite(),
                w,
                h,
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
            if p.taken || self.pickup_concealed(p) {
                continue;
            }
            let (tex, size) = match p.kind {
                PickupKind::Medkit => (SPR_MEDKIT, 0.42),
                PickupKind::Armour => (SPR_ARMOUR_PICKUP, 0.46),
                PickupKind::Ammo => (SPR_AMMO, 0.38),
                PickupKind::Shells => (SPR_SHELLS, 0.34),
                PickupKind::Shotgun => (SPR_SHOTGUN_PICKUP, 0.5),
                PickupKind::Chaingun => (SPR_CHAINGUN_PICKUP, 0.55),
                PickupKind::Launcher => (SPR_LAUNCHER_PICKUP, 0.58),
                PickupKind::Rockets => (SPR_ROCKETS_PICKUP, 0.4),
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
                tex: p.sprite,
                w: 0.28,
                h: 0.28,
                z: 0.45,
            });
        }
        for r in &self.player_rockets {
            out.push(Sprite {
                pos: r.pos,
                tex: r.sprite,
                w: 0.34,
                h: 0.34,
                z: 0.45,
            });
        }
        for b in &self.bursts {
            let t = (b.age / BURST_LIFE).clamp(0.0, 0.999);
            let tex = match (t * BURST_FRAMES as f32) as usize {
                0 => SPR_BOOM0,
                1 => SPR_BOOM1,
                _ => SPR_BOOM2,
            };
            let size = 0.6 + b.age * 2.6;
            out.push(Sprite {
                pos: b.pos,
                tex,
                w: size,
                h: size,
                z: 0.30,
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
            // A secret looks like a wall here too. That is the point.
            Cell::Secret(_) => (104, 110, 122),
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
        let expected = LEVELS[0]
            .rows
            .iter()
            .flat_map(|r| r.chars())
            .filter(|c| matches!(c, 'i' | 'd' | 'k'))
            .count();
        assert_eq!(g.total_enemies, expected);
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
    fn the_gatekeeper_spawns_on_every_difficulty() {
        for d in Difficulty::ALL {
            let g = Game::with_difficulty(4, d);
            assert!(
                g.enemies.iter().any(|e| e.kind == EnemyKind::Boss),
                "no gatekeeper on {d:?}"
            );
        }
    }

    #[test]
    fn the_gatekeeper_guards_the_final_exit() {
        let mut g = Game::new(4);
        let use_it = Input {
            use_pressed: true,
            ..Default::default()
        };
        g.player.pos = Vec2::new(30.5, 22.4);
        g.player.angle = std::f32::consts::FRAC_PI_2;
        g.update(1.0 / 60.0, &use_it);
        assert_eq!(g.state, GameState::Playing, "the gate opened for a live boss");
        assert!(
            g.messages.iter().any(|(m, _)| m == "THE GATEKEEPER STILL LIVES"),
            "the refusal was never explained"
        );
        for e in &mut g.enemies {
            if e.kind == EnemyKind::Boss {
                e.alive = false;
            }
        }
        g.update(1.0 / 60.0, &use_it);
        assert_eq!(g.state, GameState::Won, "the gate stayed shut for a dead boss");
    }

    #[test]
    fn the_gatekeeper_fires_hellfire_in_threes_and_enrages() {
        let mut g = Game::new(4);
        let boss = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Boss)
            .expect("the inferno has a gatekeeper");
        g.enemies[boss].pos = g.player.pos + Vec2::new(4.0, 0.0);
        g.enemies[boss].state = EnemyState::Chase;
        freeze_enemies(&mut g);
        g.enemies[boss].cooldown = 0.0;
        g.update(1.0 / 60.0, &Input::default());
        assert_eq!(g.projectiles.len(), 3, "the volley was not three bolts");
        let calm = g.enemies[boss].cooldown;
        // Down to a quarter health, the same volley comes much sooner.
        g.enemies[boss].health = EnemyKind::Boss.base_health() / 4;
        g.enemies[boss].cooldown = 0.0;
        g.enemies[boss].state = EnemyState::Chase;
        g.update(1.0 / 60.0, &Input::default());
        assert_eq!(g.projectiles.len(), 6, "the enraged volley never came");
        assert!(
            g.enemies[boss].cooldown < calm * 0.7,
            "no enrage: cooldown {} vs calm {}",
            g.enemies[boss].cooldown,
            calm
        );
    }

    #[test]
    fn the_gatekeepers_death_is_announced() {
        let mut g = Game::new(4);
        let boss = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Boss)
            .expect("the inferno has a gatekeeper");
        for (j, e) in g.enemies.iter_mut().enumerate() {
            if j != boss {
                e.alive = false;
            }
        }
        // Stand close: play_at drops sounds past 24 cells, by design.
        let pos = g.player.pos + Vec2::new(2.0, 0.0);
        g.enemies[boss].pos = pos;
        g.enemies[boss].health = 10;
        g.explode(pos, 3.4, 95);
        assert!(!g.enemies[boss].alive, "the blast did not kill the gatekeeper");
        assert!(
            g.messages.iter().any(|(m, _)| m == "THE GATEKEEPER SLAIN"),
            "the fall was never announced"
        );
        assert!(
            g.sounds.iter().any(|s| s.id == SoundId::BossDeath),
            "the gatekeeper died with the wrong sound"
        );
        assert!(!g.bursts.is_empty(), "the gatekeeper died without a flash");
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
        assert_eq!(
            hard.enemies[0].health,
            (hard.enemies[0].kind.base_health() as f32 * Difficulty::Hard.health_scale()).round()
                as i32
        );
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
    fn every_enemy_kind_spawns_with_its_own_stats() {
        let kinds: Vec<EnemyKind> = LEVELS
            .iter()
            .enumerate()
            .flat_map(|(i, _)| Game::new(i).enemies.into_iter().map(|e| e.kind))
            .collect();
        for kind in EnemyKind::ALL {
            assert!(kinds.contains(&kind), "{} never spawns", kind.name());
        }
        // A demon and a knight are not just recoloured imps.
        assert!(EnemyKind::Demon.speed() > EnemyKind::Imp.speed());
        assert!(EnemyKind::Demon.projectile().is_none());
        assert!(EnemyKind::Knight.base_health() > EnemyKind::Imp.base_health());
        assert!(EnemyKind::Knight.projectile().is_some());
        for kind in EnemyKind::ALL {
            assert_eq!(EnemyKind::from_index(kind.index()), kind);
        }
    }

    #[test]
    fn demons_charge_into_melee_range() {
        let mut g = Game::new(0);
        let demon = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Demon)
            .expect("the entryway has a demon");
        // Line the demon up in the player's starting room and let it charge.
        g.enemies[demon].pos = g.player.pos + Vec2::new(2.0, 0.0);
        g.enemies[demon].state = EnemyState::Chase;
        freeze_enemies(&mut g);
        g.enemies[demon].cooldown = 0.0;
        let start = g.enemies[demon].pos.dist(g.player.pos);
        for _ in 0..180 {
            g.update(1.0 / 60.0, &Input::default());
        }
        let end = g.enemies[demon].pos.dist(g.player.pos);
        assert!(end < start, "demon did not close in: {start} -> {end}");
    }

    #[test]
    fn knights_throw_hellfire_that_hurts() {
        let mut g = Game::new(2);
        let knight = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Knight)
            .expect("E1M3 has a hell knight");
        g.enemies[knight].pos = g.player.pos + Vec2::new(2.5, 0.0);
        g.enemies[knight].state = EnemyState::Chase;
        g.enemies[knight].cooldown = 0.0;
        freeze_enemies(&mut g);
        g.enemies[knight].cooldown = 0.0;
        let _ = g.take_sounds();
        let mut saw_hellfire = false;
        for _ in 0..240 {
            g.update(1.0 / 60.0, &Input::default());
            if g.projectiles
                .iter()
                .any(|p| p.sprite == SPR_HELLFIRE && p.damage > 11)
            {
                saw_hellfire = true;
                break;
            }
        }
        assert!(saw_hellfire, "knight never threw a hellfire ball");
    }

    #[test]
    fn chaingun_and_launcher_pickups_switch_weapons() {
        let mut g = Game::new(0);
        let chaingun = g
            .pickups
            .iter()
            .position(|p| p.kind == PickupKind::Chaingun)
            .expect("the entryway has a chaingun");
        g.player.pos = g.pickups[chaingun].pos;
        g.update(1.0 / 60.0, &Input::default());
        assert!(g.player.has_chaingun);
        assert_eq!(g.player.weapon, Weapon::Chaingun);
        assert!(g.player.bullets > 50);

        // The launcher appears later in the episode.
        let mut later = Game::new(2);
        let idx = later
            .pickups
            .iter()
            .position(|p| p.kind == PickupKind::Launcher)
            .expect("E1M3 has a rocket launcher");
        later.player.pos = later.pickups[idx].pos;
        later.update(1.0 / 60.0, &Input::default());
        assert!(later.player.has_launcher);
        assert_eq!(later.player.weapon, Weapon::Launcher);
        assert!(later.player.rockets > 0);

        // Rockets restock an owned launcher without switching to it.
        later.player.weapon = Weapon::Pistol;
        later.player.rockets = 0;
        let rockets = later
            .pickups
            .iter()
            .position(|p| p.kind == PickupKind::Rockets)
            .expect("E1M3 has rockets");
        later.player.pos = later.pickups[rockets].pos;
        later.update(1.0 / 60.0, &Input::default());
        assert!(later.player.rockets > 0);
        assert_eq!(later.player.weapon, Weapon::Pistol);
    }

    #[test]
    fn rockets_fly_explode_and_kill() {
        let mut g = Game::new(0);
        freeze_enemies(&mut g);
        g.player.has_launcher = true;
        g.player.weapon = Weapon::Launcher;
        g.player.rockets = 5;
        let target = g
            .enemies
            .iter()
            .position(|e| e.alive)
            .expect("an enemy to shoot");
        g.enemies[target].pos = g.player.pos + Vec2::new(3.0, 0.0);
        g.player.angle = 0.0;
        let _ = g.take_sounds();
        g.update(
            1.0 / 60.0,
            &Input {
                fire: true,
                ..Default::default()
            },
        );
        assert_eq!(g.player.rockets, 4);
        assert_eq!(g.player_rockets.len(), 1);
        let mut heard_launch = g
            .take_sounds()
            .iter()
            .any(|s| s.id == SoundId::RocketLaunch);
        let mut heard_boom = false;
        for _ in 0..90 {
            g.update(1.0 / 60.0, &Input::default());
            for s in g.take_sounds() {
                if s.id == SoundId::RocketLaunch {
                    heard_launch = true;
                }
                if s.id == SoundId::Explosion {
                    heard_boom = true;
                }
            }
            if g.player_rockets.is_empty() && heard_boom {
                break;
            }
        }
        assert!(heard_launch, "no launch sound");
        assert!(heard_boom, "the rocket never exploded");
        assert!(!g.enemies[target].alive, "the rocket did not kill the imp");
    }

    #[test]
    fn explosions_flash_fade_and_grow() {
        let mut g = Game::new(0);
        // Blow up the barrel near the start.
        g.barrels[0].health = 0;
        g.update(1.0 / 60.0, &Input::default());
        assert!(!g.bursts.is_empty(), "no explosion flash");
        let size = |g: &Game| {
            g.sprites()
                .into_iter()
                .find(|s| s.tex == SPR_BOOM0 || s.tex == SPR_BOOM1 || s.tex == SPR_BOOM2)
                .map(|s| s.w)
                .expect("explosion sprite")
        };
        let early = size(&g);
        for _ in 0..10 {
            g.update(1.0 / 60.0, &Input::default());
        }
        assert!(size(&g) > early, "the flash did not expand");
        for _ in 0..60 {
            g.update(1.0 / 60.0, &Input::default());
        }
        assert!(g.bursts.is_empty(), "the flash never faded");
    }

    #[test]
    fn a_point_blank_rocket_hurts_but_does_not_execute_you() {
        let mut g = Game::new(0);
        freeze_enemies(&mut g);
        g.player.has_launcher = true;
        g.player.weapon = Weapon::Launcher;
        g.player.rockets = 4;
        // Stand right against the wall east of the starting room.
        g.player.pos = Vec2::new(6.4, 2.5);
        g.player.angle = 0.0;
        let health = g.player.health;
        for _ in 0..30 {
            g.update(
                1.0 / 60.0,
                &Input {
                    fire: true,
                    ..Default::default()
                },
            );
        }
        assert!(g.player.health < health, "the blast did no damage at all");
        assert!(
            g.player.health > 50,
            "a wall-hugging rocket halved the player: {}",
            g.player.health
        );
    }

    #[test]
    fn knight_armour_turns_aside_a_quarter_of_every_hit() {
        assert_eq!(EnemyKind::Knight.damage_taken(100), 75);
        assert_eq!(EnemyKind::Knight.damage_taken(95), 72);
        assert_eq!(EnemyKind::Knight.damage_taken(9), 7);
        assert_eq!(EnemyKind::Knight.damage_taken(1), 1);
        assert_eq!(EnemyKind::Imp.damage_taken(100), 100);
        assert_eq!(EnemyKind::Demon.damage_taken(100), 100);
    }

    #[test]
    fn a_healthy_knight_survives_a_rocket_blast() {
        let mut g = Game::new(0);
        let i = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Demon)
            .expect("the entryway has a demon");
        // Isolate the test subject so no other death muddies the sounds.
        for (j, e) in g.enemies.iter_mut().enumerate() {
            if j != i {
                e.alive = false;
            }
        }
        let pos = g.player.pos + Vec2::new(2.0, 0.0);
        g.enemies[i].kind = EnemyKind::Knight;
        g.enemies[i].health = EnemyKind::Knight.base_health();
        g.enemies[i].pos = pos;
        g.explode(pos, 3.4, 95);
        assert!(g.enemies[i].alive, "armour did not save the knight");
        assert_eq!(g.enemies[i].health, 120 - 72);
        assert!(
            !g.sounds.iter().any(|s| s.id == SoundId::KnightDeath),
            "the knight lived but still played its death sound"
        );
    }

    #[test]
    fn splash_kills_use_each_kinds_own_death_sound() {
        let mut g = Game::new(0);
        let i = g
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::Demon)
            .expect("the entryway has a demon");
        for (j, e) in g.enemies.iter_mut().enumerate() {
            if j != i {
                e.alive = false;
            }
        }
        let pos = g.player.pos + Vec2::new(2.0, 0.0);
        g.enemies[i].pos = pos;
        g.enemies[i].health = 10;
        g.explode(pos, 3.4, 95);
        assert!(!g.enemies[i].alive, "the blast did not kill the demon");
        assert!(
            g.sounds.iter().any(|s| s.id == SoundId::DemonDeath),
            "the demon died with the wrong sound"
        );
        assert!(
            !g.sounds.iter().any(|s| s.id == SoundId::ImpDeath),
            "the demon died with the imp's sound"
        );
    }

    #[test]
    fn the_demon_has_the_shortest_reach_of_the_three_kinds() {
        assert!(EnemyKind::Demon.melee_reach() < EnemyKind::Knight.melee_reach());
        assert!(EnemyKind::Knight.melee_reach() < EnemyKind::Imp.melee_reach());
    }

    #[test]
    fn bursts_age_out_without_the_world_updating() {
        let mut g = Game::new(0);
        g.barrels[0].health = 0;
        g.update(1.0 / 60.0, &Input::default());
        assert!(!g.bursts.is_empty(), "no explosion flash");
        g.age_bursts(1.0);
        assert!(g.bursts.is_empty(), "the flash never faded");
    }

    #[test]
    fn armour_eats_a_third_of_every_hit() {
        let mut g = Game::new(0);
        g.player.armour = 100;
        g.damage_player(30);
        assert_eq!(g.player.armour, 90, "armour did not absorb its third");
        assert_eq!(g.player.health, 80, "the wearer took the unabsorbed rest");
        // Once it is spent, hits land in full again.
        g.player.armour = 0;
        g.damage_player(30);
        assert_eq!(g.player.health, 50);
    }

    #[test]
    fn an_armour_vest_sets_you_to_full_armour() {
        let mut g = Game::new(0);
        let vest = g
            .pickups
            .iter()
            .position(|p| p.kind == PickupKind::Armour)
            .expect("the entryway has a vest");
        g.player.pos = g.pickups[vest].pos;
        g.update(1.0 / 60.0, &Input::default());
        assert_eq!(g.player.armour, 100);
        assert!(g.pickups[vest].taken);
    }

    #[test]
    fn a_secret_hides_its_medkit_until_opened() {
        let mut g = Game::new(0);
        assert_eq!(g.secrets_total, 1);
        let niche = Vec2::new(15.5, 10.5);
        // Even standing in the sealed niche, you cannot take what you
        // cannot see.
        g.player.pos = niche;
        g.player.health = 50;
        g.update(1.0 / 60.0, &Input::default());
        assert_eq!(g.player.health, 50, "took a medkit through a sealed wall");
        assert!(
            !g.sprites().iter().any(|s| s.pos == niche),
            "the hidden medkit was drawn"
        );
        // Face the wall and use it: the niche opens and stays counted.
        g.player.pos = Vec2::new(14.5, 10.5);
        g.player.angle = 0.0;
        g.update(
            1.0 / 60.0,
            &Input {
                use_pressed: true,
                ..Default::default()
            },
        );
        assert_eq!(g.secrets_found, 1);
        assert!(
            g.messages.iter().any(|(m, _)| m == "A SECRET IS REVEALED"),
            "the reveal was never announced"
        );
        for _ in 0..90 {
            g.update(1.0 / 60.0, &Input::default());
        }
        // Now the medkit is there for the taking.
        g.player.pos = niche;
        g.update(1.0 / 60.0, &Input::default());
        assert_eq!(g.player.health, 75, "the revealed medkit did nothing");
    }

    #[test]
    fn every_kind_has_its_own_voice() {
        let voices: [fn(EnemyKind) -> SoundId; 4] = [
            EnemyKind::sight_sound,
            EnemyKind::pain_sound,
            EnemyKind::attack_sound,
            EnemyKind::death_sound,
        ];
        for voice in voices {
            let mut ids: Vec<usize> = EnemyKind::ALL.iter().map(|k| voice(*k) as usize).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), 4, "two kinds share a voice");
        }
    }

    #[test]
    fn weapon_keys_switch_and_cycle_between_owned_guns() {
        let mut g = Game::new(0);
        g.player.has_shotgun = true;
        g.player.shells = 20;
        g.player.has_chaingun = true;
        g.player.weapon = Weapon::Pistol;
        g.update(
            1.0 / 60.0,
            &Input {
                weapon3: true,
                ..Default::default()
            },
        );
        assert_eq!(g.player.weapon, Weapon::Chaingun);
        g.update(
            1.0 / 60.0,
            &Input {
                weapon_next: true,
                ..Default::default()
            },
        );
        assert_eq!(
            g.player.weapon,
            Weapon::Pistol,
            "cycles past the last owned gun"
        );
        g.update(
            1.0 / 60.0,
            &Input {
                weapon_prev: true,
                ..Default::default()
            },
        );
        assert_eq!(g.player.weapon, Weapon::Chaingun);
        g.update(
            1.0 / 60.0,
            &Input {
                weapon4: true,
                ..Default::default()
            },
        );
        assert_ne!(
            g.player.weapon,
            Weapon::Launcher,
            "unowned launcher equipped"
        );
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
        assert_eq!(
            g.enemies[0].health,
            (g.enemies[0].kind.base_health() as f32 * Difficulty::Hard.health_scale()).round()
                as i32
        );
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

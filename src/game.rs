//! Game simulation: player, weapons, hitscan combat, enemy AI, doors, pickups.
//! Rendering lives in `render.rs`; this module has no graphics code.

use crate::map::{Cell, LEVEL, Map, UseOutcome};
use crate::math::{Rng, Vec2};
use crate::textures::*;

const PLAYER_RADIUS: f32 = 0.28;
const WALK_SPEED: f32 = 3.1;
const RUN_SPEED: f32 = 4.9;
const MOUSE_SENSITIVITY: f32 = 0.0022;

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

pub struct Pickup {
    pub pos: Vec2,
    pub kind: PickupKind,
    pub taken: bool,
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
    rng: Rng,
}

impl Game {
    pub fn new() -> Self {
        let map = Map::from_level(LEVEL);
        let mut spawn = Vec2::new(2.5, 2.5);
        let mut enemies = Vec::new();
        let mut barrels = Vec::new();
        let mut pickups = Vec::new();

        for (y, row) in LEVEL.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let pos = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                match ch {
                    'P' => spawn = pos,
                    'i' => enemies.push(Enemy {
                        pos,
                        health: 40,
                        state: EnemyState::Idle,
                        anim: 0.0,
                        cooldown: 0.6,
                        state_time: 0.0,
                        dead_time: 0.0,
                        alive: true,
                    }),
                    'b' => barrels.push(Barrel {
                        pos,
                        health: 22,
                        alive: true,
                        fade: 0.0,
                    }),
                    'm' => pickups.push(Pickup {
                        pos,
                        kind: PickupKind::Medkit,
                        taken: false,
                    }),
                    'a' => pickups.push(Pickup {
                        pos,
                        kind: PickupKind::Ammo,
                        taken: false,
                    }),
                    's' => pickups.push(Pickup {
                        pos,
                        kind: PickupKind::Shells,
                        taken: false,
                    }),
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
            rng: Rng::new(0xD00D_1234_5678_9ABC),
        }
    }

    pub fn message(&mut self, text: impl Into<String>) {
        self.messages.insert(0, (text.into(), 3.5));
        self.messages.truncate(3);
    }

    pub fn update(&mut self, dt: f32, input: &Input) {
        let dt = dt.clamp(0.0001, 0.05);
        self.time += dt;
        for m in &mut self.messages {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);

        if input.restart {
            *self = Game::new();
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
                UseOutcome::Opened => {}
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
            return;
        }
        match weapon {
            Weapon::Pistol => self.player.bullets -= 1,
            Weapon::Shotgun => self.player.shells -= 1,
        }
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
            let e = &mut self.enemies[i];
            e.health -= damage;
            if e.health <= 0 {
                e.alive = false;
                e.state = EnemyState::Dead;
                e.dead_time = 0.0;
                self.kills += 1;
            } else {
                e.state = EnemyState::Pain;
                e.state_time = 0.22;
            }
            return true;
        }
        false
    }

    fn update_enemies(&mut self, dt: f32) {
        let map = &self.map;
        let player_pos = self.player.pos;
        let playing = self.state == GameState::Playing;
        let mut damage_to_player = 0;
        let mut shots = Vec::new();

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
                        map.try_move(&mut e.pos, dir * 1.0 * dt, 0.3);
                    } else if dist > 1.35 {
                        let dir = to_player.norm();
                        map.try_move(&mut e.pos, dir * 1.75 * dt, 0.3);
                    }
                    if e.cooldown <= 0.0 && los {
                        if dist < 1.6 {
                            damage_to_player += 9;
                            e.cooldown = 1.1;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.32;
                        } else if dist < 10.0 {
                            let aim = to_player.norm();
                            shots.push((e.pos + aim * 0.35, aim * 5.2));
                            e.cooldown = 2.0;
                            e.state = EnemyState::Attack;
                            e.state_time = 0.4;
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
                    self.player.health = (self.player.health + 25).min(100);
                    messages.push("PICKED UP A MEDKIT");
                }
                PickupKind::Ammo => {
                    if self.player.bullets >= 200 {
                        continue;
                    }
                    self.player.bullets = (self.player.bullets + 20).min(200);
                    messages.push("PICKED UP A CLIP");
                }
                PickupKind::Shells => {
                    if self.player.shells >= 60 {
                        continue;
                    }
                    self.player.shells = (self.player.shells + 8).min(60);
                    messages.push("PICKED UP SHELLS");
                }
                PickupKind::Shotgun => {
                    self.player.has_shotgun = true;
                    self.player.weapon = Weapon::Shotgun;
                    self.player.shells = (self.player.shells + 8).min(60);
                    messages.push("YOU GOT THE SHOTGUN!");
                }
            }
            taken.push(i);
        }
        for i in taken {
            self.pickups[i].taken = true;
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
        for e in &mut self.enemies {
            if e.alive && e.pos.dist(pos) < 2.6 {
                e.health -= 55;
                if e.health <= 0 {
                    e.alive = false;
                    e.state = EnemyState::Dead;
                    e.dead_time = 0.0;
                    self.kills += 1;
                } else {
                    e.state = EnemyState::Pain;
                    e.state_time = 0.3;
                }
            }
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
        self.player.health -= amount;
        self.player.hurt = (self.player.hurt + amount as f32 / 26.0).min(1.0);
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
        Self::new()
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
        let g = Game::new();
        assert_eq!(g.total_enemies, 12);
        assert_eq!(g.player.health, 100);
        assert_eq!(g.player.pos, Vec2::new(3.5, 2.5));
        assert!(g.barrels.iter().any(|b| (b.pos.x - 4.5).abs() < 0.01));
    }

    #[test]
    fn player_cannot_walk_through_walls() {
        let mut g = Game::new();
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
        let mut g = Game::new();
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
        let mut g = Game::new();
        g.player.pos = Vec2::new(21.5, 17.5);
        g.update(1.0 / 60.0, &Input::default());
        assert!(g.player.has_shotgun);
        assert_eq!(g.player.weapon, Weapon::Shotgun);
        assert!(g.player.shells >= 8);
    }

    #[test]
    fn doors_open_when_used() {
        let mut g = Game::new();
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
        let mut g = Game::new();
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
        let mut g = Game::new();
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
    fn restart_resets_the_game() {
        let mut g = Game::new();
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
    }
}

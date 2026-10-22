//! One saved run, written as text beside the settings so a game in progress
//! can be picked up again from the title screen.
//!
//! The format is a small line-per-entity text file: easy to diff, easy to
//! ignore when it is corrupt (a broken save is simply not offered).

use std::fs;
use std::path::PathBuf;

use crate::config;
use crate::game::{
    BarrelSnapshot, Difficulty, DoorSnapshot, EnemyKind, EnemySnapshot, EnemyState, Game,
    PickupKind, PickupSnapshot, PlayerSnapshot, Snapshot, Weapon,
};
use crate::math::Vec2;

const HEADER: &str = "D00M SAVE 1";

/// How many parallel runs the game keeps.
pub const SLOTS: usize = 3;

pub struct Run {
    pub snapshot: Snapshot,
    /// Kills and time from the levels already finished in this episode.
    pub total_kills: usize,
    pub total_time: f32,
}

/// What the load screen shows for an occupied slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotInfo {
    pub level: usize,
    pub level_name: &'static str,
    /// Kills and time for the whole episode so far.
    pub kills: usize,
    pub time: f32,
    pub difficulty: Difficulty,
}

pub fn path_for(slot: usize) -> Option<PathBuf> {
    if slot >= SLOTS {
        return None;
    }
    config::path().map(|p| p.with_file_name(format!("save{}.txt", slot + 1)))
}

pub fn write_slot(slot: usize, run: &Run) -> std::io::Result<()> {
    let Some(path) = path_for(slot) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, encode(run))
}

pub fn read_slot(slot: usize) -> Option<Run> {
    let path = path_for(slot)?;
    decode(&fs::read_to_string(path).ok()?)
}

pub fn clear_slot(slot: usize) {
    if let Some(path) = path_for(slot) {
        let _ = fs::remove_file(path);
    }
}

pub fn summary(slot: usize) -> Option<SlotInfo> {
    let run = read_slot(slot)?;
    let s = &run.snapshot;
    let level = s.level.min(crate::levels::LEVELS.len() - 1);
    Some(SlotInfo {
        level,
        level_name: crate::levels::LEVELS[level].name,
        kills: run.total_kills + s.kills,
        time: run.total_time + s.time,
        difficulty: s.difficulty,
    })
}

pub fn summaries() -> Vec<Option<SlotInfo>> {
    (0..SLOTS).map(summary).collect()
}

pub fn any_saved() -> bool {
    (0..SLOTS).any(|slot| path_for(slot).is_some_and(|p| p.is_file()))
}

fn modified(slot: usize) -> Option<std::time::SystemTime> {
    fs::metadata(path_for(slot)?).ok()?.modified().ok()
}

/// The most recently written save, for CONTINUE.
pub fn newest() -> Option<(usize, Run)> {
    let slot = (0..SLOTS)
        .filter(|&s| read_slot(s).is_some())
        .max_by_key(|&s| modified(s))?;
    Some((slot, read_slot(slot)?))
}

/// Slot a new run should use: the first empty one, else the oldest save.
pub fn next_slot() -> usize {
    for slot in 0..SLOTS {
        if summary(slot).is_none() {
            return slot;
        }
    }
    (0..SLOTS).min_by_key(|&s| modified(s)).unwrap_or_default()
}

fn encode(run: &Run) -> String {
    let s = &run.snapshot;
    let mut out = String::new();
    out.push_str(HEADER);
    out.push('\n');
    out.push_str(&format!("run {} {:.4}\n", run.total_kills, run.total_time));
    out.push_str(&format!(
        "level {} {} {} {:.4} {}\n",
        s.level,
        s.difficulty.index(),
        s.kills,
        s.time,
        s.total_enemies
    ));
    let p = &s.player;
    out.push_str(&format!(
        "ply {:.4} {:.4} {:.4} {} {} {} {} {} {} {} {}\n",
        p.pos.x,
        p.pos.y,
        p.angle,
        p.health,
        p.bullets,
        p.shells,
        p.weapon.index(),
        if p.has_shotgun { 1 } else { 0 },
        p.rockets,
        if p.has_chaingun { 1 } else { 0 },
        if p.has_launcher { 1 } else { 0 }
    ));
    for d in &s.doors {
        out.push_str(&format!(
            "door {} {} {:.4} {} {:.4}\n",
            d.x, d.y, d.open, d.phase, d.timer
        ));
    }
    for e in &s.enemies {
        out.push_str(&format!(
            "enemy {:.4} {:.4} {} {} {:.4} {:.4} {:.4} {:.4} {} {}\n",
            e.pos.x,
            e.pos.y,
            e.health,
            e.state.index(),
            e.anim,
            e.cooldown,
            e.state_time,
            e.dead_time,
            if e.alive { 1 } else { 0 },
            e.kind.index()
        ));
    }
    for b in &s.barrels {
        out.push_str(&format!(
            "barrel {:.4} {:.4} {} {} {:.4}\n",
            b.pos.x,
            b.pos.y,
            b.health,
            if b.alive { 1 } else { 0 },
            b.fade
        ));
    }
    for p in &s.pickups {
        out.push_str(&format!(
            "pickup {:.4} {:.4} {} {}\n",
            p.pos.x,
            p.pos.y,
            p.kind.index(),
            if p.taken { 1 } else { 0 }
        ));
    }
    out
}

fn num<T: std::str::FromStr>(parts: &[&str], index: usize) -> Option<T> {
    parts.get(index)?.parse().ok()
}

fn flag(parts: &[&str], index: usize) -> bool {
    num::<i32>(parts, index).is_some_and(|v| v != 0)
}

fn point(parts: &[&str], index: usize) -> Option<Vec2> {
    Some(Vec2::new(num(parts, index)?, num(parts, index + 1)?))
}

fn decode(text: &str) -> Option<Run> {
    let mut lines = text.lines();
    if !lines.next()?.trim().starts_with(HEADER) {
        return None;
    }

    let mut total_kills = 0;
    let mut total_time = 0.0f32;
    let mut level: Option<(usize, Difficulty, usize, f32, usize)> = None;
    let mut player: Option<PlayerSnapshot> = None;
    let mut doors = Vec::new();
    let mut enemies = Vec::new();
    let mut barrels = Vec::new();
    let mut pickups = Vec::new();

    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let Some(tag) = parts.first() else { continue };
        match *tag {
            "run" => {
                total_kills = num::<usize>(&parts, 1)?;
                total_time = num::<f32>(&parts, 2)?.max(0.0);
            }
            "level" => {
                level = Some((
                    num(&parts, 1)?,
                    Difficulty::from_index(num::<usize>(&parts, 2)?),
                    num(&parts, 3)?,
                    num::<f32>(&parts, 4)?.max(0.0),
                    num(&parts, 5)?,
                ));
            }
            "ply" => {
                player = Some(PlayerSnapshot {
                    pos: point(&parts, 1)?,
                    angle: num::<f32>(&parts, 3)?,
                    health: num(&parts, 4)?,
                    bullets: num(&parts, 5)?,
                    shells: num(&parts, 6)?,
                    weapon: Weapon::from_index(num::<usize>(&parts, 7)?),
                    has_shotgun: flag(&parts, 8),
                    rockets: num(&parts, 9).unwrap_or(0),
                    has_chaingun: flag(&parts, 10),
                    has_launcher: flag(&parts, 11),
                });
            }
            "door" => doors.push(DoorSnapshot {
                x: num(&parts, 1)?,
                y: num(&parts, 2)?,
                open: num(&parts, 3)?,
                phase: num::<u8>(&parts, 4)?,
                timer: num(&parts, 5)?,
            }),
            "enemy" => enemies.push(EnemySnapshot {
                pos: point(&parts, 1)?,
                health: num(&parts, 3)?,
                state: EnemyState::from_index(num::<usize>(&parts, 4)?),
                anim: num(&parts, 5)?,
                cooldown: num(&parts, 6)?,
                state_time: num(&parts, 7)?,
                dead_time: num(&parts, 8)?,
                alive: flag(&parts, 9),
                kind: num::<usize>(&parts, 10)
                    .map(EnemyKind::from_index)
                    .unwrap_or(EnemyKind::Imp),
            }),
            "barrel" => barrels.push(BarrelSnapshot {
                pos: point(&parts, 1)?,
                health: num(&parts, 3)?,
                alive: flag(&parts, 4),
                fade: num(&parts, 5)?,
            }),
            "pickup" => pickups.push(PickupSnapshot {
                pos: point(&parts, 1)?,
                kind: PickupKind::from_index(num::<usize>(&parts, 3)?),
                taken: flag(&parts, 4),
            }),
            // Unknown tags (from a newer build) are skipped on purpose.
            _ => {}
        }
    }

    let (level, difficulty, kills, time, total_enemies) = level?;
    Some(Run {
        snapshot: Snapshot {
            level: level.min(crate::levels::LEVELS.len() - 1),
            difficulty,
            total_enemies,
            time,
            kills,
            player: player?,
            enemies,
            barrels,
            pickups,
            doors,
        },
        total_kills,
        total_time,
    })
}

/// A run saved from a live game, ready to be restored.
pub fn rundown(game: &Game, total_kills: usize, total_time: f32) -> Run {
    Run {
        snapshot: game.snapshot(),
        total_kills,
        total_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Input;

    fn played_game() -> Game {
        let mut game = Game::with_difficulty(0, Difficulty::Hard);
        game.player.pos = Vec2::new(4.5, 6.5);
        game.player.angle = 1.25;
        game.player.health = 64;
        game.player.bullets = 37;
        game.player.shells = 11;
        game.player.has_shotgun = true;
        game.player.has_chaingun = true;
        game.player.has_launcher = true;
        game.player.rockets = 9;
        game.player.weapon = Weapon::Shotgun;
        // Open a door and take some damage.
        game.player.pos = Vec2::new(6.6, 5.5);
        game.player.angle = 0.0;
        game.update(
            1.0 / 60.0,
            &Input {
                use_pressed: true,
                ..Default::default()
            },
        );
        for _ in 0..30 {
            game.update(1.0 / 60.0, &Input::default());
        }
        assert!(game.map.door_open(7, 5) > 0.0, "the door never opened");
        game.player.pos = Vec2::new(4.5, 6.5);
        game.player.angle = 1.25;
        game.player.health -= 20;
        game.kills = 3;
        game.time = 42.5;
        game
    }

    #[test]
    fn a_run_round_trips_through_text() {
        let game = played_game();
        let before = game.snapshot();
        let run = rundown(&game, 7, 128.75);
        let text = encode(&run);
        let loaded = decode(&text).expect("decode");

        assert_eq!(loaded.total_kills, 7);
        assert!((loaded.total_time - 128.75).abs() < 0.01);

        let after = loaded.snapshot;
        assert_eq!(after.level, before.level);
        assert_eq!(after.difficulty, before.difficulty);
        assert_eq!(after.total_enemies, before.total_enemies);
        assert_eq!(after.kills, before.kills);
        assert!((after.time - before.time).abs() < 0.01);
        assert!((after.player.pos.x - before.player.pos.x).abs() < 0.01);
        assert!((after.player.pos.y - before.player.pos.y).abs() < 0.01);
        assert!((after.player.angle - before.player.angle).abs() < 0.01);
        assert_eq!(after.player.health, before.player.health);
        assert_eq!(after.player.weapon, Weapon::Shotgun);
        assert!(after.player.has_shotgun);
        assert_eq!(after.player.rockets, 9);
        assert!(after.player.has_chaingun);
        assert!(after.player.has_launcher);
        assert_eq!(after.enemies.len(), before.enemies.len());
        let kinds: Vec<EnemyKind> = after.enemies.iter().map(|e| e.kind).collect();
        let before_kinds: Vec<EnemyKind> = before.enemies.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, before_kinds, "enemy kinds were lost");
        assert!(
            kinds.contains(&EnemyKind::Demon),
            "the entryway's demon vanished"
        );
        assert_eq!(
            after.enemies.iter().filter(|e| e.alive).count(),
            before.enemies.iter().filter(|e| e.alive).count()
        );
        assert_eq!(after.pickups.len(), before.pickups.len());
        assert_eq!(after.doors.len(), before.doors.len());
        for (a, b) in after.doors.iter().zip(before.doors.iter()) {
            assert_eq!((a.x, a.y), (b.x, b.y), "door moved");
            assert_eq!(a.phase, b.phase);
            assert!(
                (a.open - b.open).abs() < 0.001,
                "door open {} vs {}",
                a.open,
                b.open
            );
            assert!((a.timer - b.timer).abs() < 0.001);
        }
        assert!(
            before.doors.iter().any(|d| d.open > 0.0),
            "the test never opened a door"
        );

        // The restored game is playable and keeps the door that was opened.
        let mut restored = Game::from_snapshot(after);
        assert_eq!(restored.level, 0);
        assert_eq!(restored.difficulty, Difficulty::Hard);
        assert!(restored.map.door_open(7, 5) > 0.0, "door state was lost");
        for _ in 0..60 {
            restored.update(1.0 / 60.0, &Input::default());
        }
        assert!(restored.player.health > 0);
    }

    #[test]
    fn saves_from_older_builds_still_load() {
        let game = played_game();
        let text = encode(&rundown(&game, 0, 0.0));
        // Drop the fields the older format did not have.
        let legacy: String = text
            .lines()
            .map(|line| {
                let mut parts: Vec<&str> = line.split_whitespace().collect();
                match parts.first().copied() {
                    Some("ply") => parts.truncate(9),
                    Some("enemy") => parts.truncate(10),
                    _ => {}
                }
                parts.join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let loaded = decode(&legacy).expect("legacy save should still load");
        assert_eq!(loaded.snapshot.player.weapon, Weapon::Shotgun);
        assert_eq!(loaded.snapshot.player.rockets, 0);
        assert!(
            loaded
                .snapshot
                .enemies
                .iter()
                .all(|e| e.kind == EnemyKind::Imp)
        );
    }

    #[test]
    fn killed_enemies_stay_dead() {
        let mut game = Game::with_difficulty(0, Difficulty::Normal);
        game.enemies[0].alive = false;
        game.enemies[0].state = EnemyState::Dead;
        game.enemies[0].dead_time = 9.0;
        let loaded = decode(&encode(&rundown(&game, 0, 0.0))).expect("decode");
        let restored = Game::from_snapshot(loaded.snapshot);
        assert!(!restored.enemies[0].alive);
        assert_eq!(restored.enemies[0].state, EnemyState::Dead);
    }

    #[test]
    fn junk_is_rejected_instead_of_half_loaded() {
        assert!(decode("").is_none());
        assert!(decode("hello world\n").is_none());
        assert!(
            decode("D00M SAVE 1\nlevel 0 1 0 0.0 12\n").is_none(),
            "no player"
        );
        assert!(
            decode("D00M SAVE 1\nply 1 2 0 100 50 0 0 0\n").is_none(),
            "no level"
        );
        assert!(
            decode("D00M SAVE 1\nlevel x 1 0 0.0 12\n").is_none(),
            "bad number"
        );
    }

    #[test]
    fn unknown_lines_are_ignored() {
        let game = Game::new(0);
        let mut text = encode(&rundown(&game, 2, 10.0));
        text.push_str("future_tag 1 2 3\n");
        let loaded = decode(&text).expect("decode");
        assert_eq!(loaded.total_kills, 2);
    }

    #[test]
    fn slots_save_load_and_clear() {
        let dir = std::env::temp_dir().join(format!("d00m-save-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        // SAFETY: this test is the only one that reads or writes these vars.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
        }
        let game = Game::new(0);
        assert!(!any_saved());
        assert!(read_slot(0).is_none());
        assert_eq!(next_slot(), 0);

        write_slot(0, &rundown(&game, 1, 2.0)).expect("write");
        assert!(any_saved());
        assert!(read_slot(0).is_some());
        let info = summary(0).expect("summary");
        assert_eq!(info.level, 0);
        assert_eq!(info.kills, 1);
        assert_eq!(summaries().len(), SLOTS);
        assert!(summaries()[1].is_none());

        // SECOND slot, then CONTINUE picks the newest one.
        write_slot(1, &rundown(&game, 9, 20.0)).expect("write");
        assert_eq!(next_slot(), 2);
        let (slot, run) = newest().expect("newest");
        assert_eq!(slot, 1);
        assert_eq!(run.total_kills, 9);

        clear_slot(1);
        assert!(read_slot(1).is_none());
        assert_eq!(next_slot(), 1);

        clear_slot(0);
        assert!(!any_saved());
        fs::remove_dir_all(&dir).ok();
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

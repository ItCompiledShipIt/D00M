//! The level: a grid of cells, sliding doors, collision and line-of-sight.

use crate::math::Vec2;
use crate::textures::{TEX_BRICK, TEX_CRATE, TEX_DOOR, TEX_EXIT, TEX_HELL, TEX_MARBLE, TEX_TECH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Wall(u8),
    Door(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOutcome {
    None,
    Opened,
    Exit,
}

pub struct Map {
    pub w: usize,
    pub h: usize,
    cells: Vec<Cell>,
    open: Vec<f32>,
    phase: Vec<Phase>,
    timer: Vec<f32>,
    doors: Vec<(i32, i32)>,
    /// Doors that finished closing this frame (for the sound layer).
    pub closed_doors: Vec<Vec2>,
}

impl Map {
    pub fn from_level(rows: &[&str]) -> Map {
        let h = rows.len();
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut cells = vec![Cell::Empty; w * h];
        let mut doors = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let cell = match ch {
                    '#' => Cell::Wall(TEX_BRICK),
                    '=' => Cell::Wall(TEX_TECH),
                    'H' => Cell::Wall(TEX_HELL),
                    'M' => Cell::Wall(TEX_MARBLE),
                    'C' => Cell::Wall(TEX_CRATE),
                    'S' => Cell::Wall(TEX_EXIT),
                    'D' => {
                        doors.push((x as i32, y as i32));
                        Cell::Door(TEX_DOOR)
                    }
                    _ => Cell::Empty,
                };
                cells[y * w + x] = cell;
            }
        }
        Self {
            w,
            h,
            cells,
            open: vec![0.0; w * h],
            phase: vec![Phase::Closed; w * h],
            timer: vec![0.0; w * h],
            doors,
            closed_doors: Vec::new(),
        }
    }

    pub fn cell(&self, x: i32, y: i32) -> Cell {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return Cell::Wall(TEX_BRICK);
        }
        self.cells[y as usize * self.w + x as usize]
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    /// Every door's progress and timer, for save games: cell, how far open,
    /// phase as a number, and the auto-close timer.
    pub fn door_states(&self) -> Vec<(i32, i32, f32, u8, f32)> {
        self.doors
            .iter()
            .filter_map(|&(x, y)| {
                let i = self.idx(x, y)?;
                let phase = match self.phase[i] {
                    Phase::Closed => 0,
                    Phase::Opening => 1,
                    Phase::Open => 2,
                    Phase::Closing => 3,
                };
                Some((x, y, self.open[i], phase, self.timer[i]))
            })
            .collect()
    }

    /// Restore doors saved with [`Map::door_states`]. Cells that are not doors
    /// and phases out of range are ignored.
    pub fn restore_door_states(&mut self, states: &[(i32, i32, f32, u8, f32)]) {
        for &(x, y, open, phase, timer) in states {
            let Some(i) = self.idx(x, y) else { continue };
            if !matches!(self.cells[i], Cell::Door(_)) {
                continue;
            }
            self.open[i] = if open.is_finite() {
                open.clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.phase[i] = match phase {
                0 => Phase::Closed,
                1 => Phase::Opening,
                2 => Phase::Open,
                3 => Phase::Closing,
                _ => Phase::Closed,
            };
            self.timer[i] = if timer.is_finite() {
                timer.max(0.0)
            } else {
                0.0
            };
        }
    }

    /// How far a door has slid open, 0.0 (closed) to 1.0 (gone).
    pub fn door_open(&self, x: i32, y: i32) -> f32 {
        self.idx(x, y).map(|i| self.open[i]).unwrap_or(0.0)
    }

    /// Blocks walking. Doors become passable once mostly raised.
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        match self.cell(x, y) {
            Cell::Empty => false,
            Cell::Wall(_) => true,
            Cell::Door(_) => self.door_open(x, y) < 0.7,
        }
    }

    /// Blocks bullets and sight. Doors block until nearly fully open.
    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        match self.cell(x, y) {
            Cell::Empty => false,
            Cell::Wall(_) => true,
            Cell::Door(_) => self.door_open(x, y) < 0.85,
        }
    }

    /// Whether a circle at `pos` overlaps a solid cell.
    pub fn circle_blocked(&self, pos: Vec2, radius: f32) -> bool {
        let x0 = (pos.x - radius).floor() as i32;
        let x1 = (pos.x + radius).floor() as i32;
        let y0 = (pos.y - radius).floor() as i32;
        let y1 = (pos.y + radius).floor() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if self.is_solid(x, y) {
                    return true;
                }
            }
        }
        false
    }

    /// Move with wall sliding: each axis is tested separately.
    pub fn try_move(&self, pos: &mut Vec2, delta: Vec2, radius: f32) {
        let nx = pos.x + delta.x;
        if !self.circle_blocked(Vec2::new(nx, pos.y), radius) {
            pos.x = nx;
        }
        let ny = pos.y + delta.y;
        if !self.circle_blocked(Vec2::new(pos.x, ny), radius) {
            pos.y = ny;
        }
    }

    pub fn line_of_sight(&self, from: Vec2, to: Vec2) -> bool {
        let d = to - from;
        let dist = d.len();
        if dist < 1e-4 {
            return true;
        }
        let step = d / (dist / 0.08);
        let mut p = from;
        let mut travelled = 0.0;
        while travelled < dist {
            p += step;
            travelled += step.len();
            if self.blocks_sight(p.x.floor() as i32, p.y.floor() as i32) {
                return false;
            }
        }
        true
    }

    /// Distance to the first wall along `dir`, capped at `max`.
    pub fn ray_wall_dist(&self, from: Vec2, dir: Vec2, max: f32) -> f32 {
        let mut d = 0.0;
        let step = 0.04;
        while d < max {
            d += step;
            let p = from + dir * d;
            if self.blocks_sight(p.x.floor() as i32, p.y.floor() as i32) {
                return d;
            }
        }
        max
    }

    /// Press "use": open a nearby door or hit the exit switch.
    pub fn use_at(&mut self, pos: Vec2, angle: f32) -> UseOutcome {
        let dir = Vec2::from_angle(angle);
        let mut d = 0.2;
        while d <= 1.6 {
            let p = pos + dir * d;
            let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
            match self.cell(x, y) {
                Cell::Wall(TEX_EXIT) => return UseOutcome::Exit,
                Cell::Door(_) => {
                    self.raise_door(x, y);
                    return UseOutcome::Opened;
                }
                Cell::Wall(_) => return UseOutcome::None,
                Cell::Empty => {}
            }
            d += 0.1;
        }
        UseOutcome::None
    }

    fn raise_door(&mut self, x: i32, y: i32) {
        if let Some(i) = self.idx(x, y) {
            if matches!(self.phase[i], Phase::Closed | Phase::Closing) {
                self.phase[i] = Phase::Opening;
            }
            self.timer[i] = 4.0;
        }
    }

    pub fn update(&mut self, dt: f32, player: Vec2) {
        self.closed_doors.clear();
        for (x, y) in self.doors.clone() {
            let Some(i) = self.idx(x, y) else { continue };
            let occupied = player.x.floor() as i32 == x && player.y.floor() as i32 == y;
            match self.phase[i] {
                Phase::Closed => {}
                Phase::Opening => {
                    self.open[i] = (self.open[i] + dt * 1.6).min(1.0);
                    if self.open[i] >= 1.0 {
                        self.phase[i] = Phase::Open;
                        self.timer[i] = 4.0;
                    }
                }
                Phase::Open => {
                    if occupied {
                        self.timer[i] = self.timer[i].max(1.0);
                    }
                    self.timer[i] -= dt;
                    if self.timer[i] <= 0.0 {
                        self.phase[i] = Phase::Closing;
                    }
                }
                Phase::Closing => {
                    if occupied {
                        self.phase[i] = Phase::Opening;
                        continue;
                    }
                    self.open[i] = (self.open[i] - dt * 1.6).max(0.0);
                    if self.open[i] <= 0.0 {
                        self.phase[i] = Phase::Closed;
                        self.closed_doors
                            .push(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LEVELS;

    #[test]
    fn every_level_is_rectangular() {
        for level in LEVELS {
            let w = level.rows[0].len();
            assert!(w >= 10 && level.rows.len() >= 10, "{} is tiny", level.name);
            for row in level.rows {
                assert_eq!(row.len(), w, "ragged row in {}: {row:?}", level.name);
            }
        }
    }

    #[test]
    fn every_level_is_enclosed() {
        for level in LEVELS {
            let map = Map::from_level(level.rows);
            for x in 0..map.w as i32 {
                assert!(map.is_solid(x, 0), "{}: top edge open at {x}", level.name);
                assert!(
                    map.is_solid(x, map.h as i32 - 1),
                    "{}: bottom edge open at {x}",
                    level.name
                );
            }
            for y in 0..map.h as i32 {
                assert!(map.is_solid(0, y), "{}: left edge open at {y}", level.name);
                assert!(
                    map.is_solid(map.w as i32 - 1, y),
                    "{}: right edge open at {y}",
                    level.name
                );
            }
        }
    }

    #[test]
    fn every_entity_is_reachable_from_spawn() {
        for level in LEVELS {
            let map = Map::from_level(level.rows);
            let (mut sx, mut sy) = (0, 0);
            for (y, row) in level.rows.iter().enumerate() {
                if let Some(x) = row.find('P') {
                    sx = x as i32;
                    sy = y as i32;
                }
            }
            let mut seen = vec![false; map.w * map.h];
            let mut queue = vec![(sx, sy)];
            seen[sy as usize * map.w + sx as usize] = true;
            while let Some((x, y)) = queue.pop() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    let passable = matches!(map.cell(nx, ny), Cell::Empty | Cell::Door(_));
                    if passable && !seen[ny as usize * map.w + nx as usize] {
                        seen[ny as usize * map.w + nx as usize] = true;
                        queue.push((nx, ny));
                    }
                }
            }
            for (y, row) in level.rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if "imaspgb".contains(ch) {
                        assert!(
                            seen[y * map.w + x],
                            "{}: entity {ch:?} at ({x},{y}) is unreachable",
                            level.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_exit_switch_can_be_used() {
        for level in LEVELS {
            let mut map = Map::from_level(level.rows);
            let mut switches = 0;
            for (y, row) in level.rows.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch != 'S' {
                        continue;
                    }
                    switches += 1;
                    let mut usable = false;
                    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if !matches!(map.cell(nx, ny), Cell::Empty) {
                            continue;
                        }
                        let pos = Vec2::new(nx as f32 + 0.5, ny as f32 + 0.5);
                        let angle = (-dy as f32).atan2(-dx as f32);
                        if map.use_at(pos, angle) == UseOutcome::Exit {
                            usable = true;
                        }
                    }
                    assert!(
                        usable,
                        "{}: exit switch at ({x},{y}) cannot be reached",
                        level.name
                    );
                }
            }
            assert_eq!(switches, 1, "{} needs exactly one exit", level.name);
        }
    }

    #[test]
    fn doors_start_closed_then_open() {
        let mut map = Map::from_level(LEVELS[0].rows);
        assert_eq!(map.cell(7, 5), Cell::Door(TEX_DOOR));
        assert!(map.is_solid(7, 5));
        let outcome = map.use_at(Vec2::new(6.6, 5.5), 0.0);
        assert_eq!(outcome, UseOutcome::Opened);
        for _ in 0..120 {
            map.update(1.0 / 60.0, Vec2::new(3.5, 2.5));
        }
        assert!(map.door_open(7, 5) > 0.99);
        assert!(!map.is_solid(7, 5));
    }

    #[test]
    fn exit_switch_is_detected() {
        let mut map = Map::from_level(LEVELS[0].rows);
        let outcome = map.use_at(Vec2::new(24.5, 22.6), std::f32::consts::FRAC_PI_2);
        assert_eq!(outcome, UseOutcome::Exit);
    }

    #[test]
    fn collision_blocks_walls() {
        let map = Map::from_level(LEVELS[0].rows);
        let mut p = Vec2::new(1.5, 1.5);
        map.try_move(&mut p, Vec2::new(-2.0, 0.0), 0.28);
        assert!(p.x >= 1.0, "player walked through the west wall: {p:?}");
        map.try_move(&mut p, Vec2::new(0.0, -2.0), 0.28);
        assert!(p.y >= 1.0, "player walked through the north wall: {p:?}");
    }

    #[test]
    fn line_of_sight_respects_walls() {
        let map = Map::from_level(LEVELS[0].rows);
        assert!(map.line_of_sight(Vec2::new(2.5, 2.5), Vec2::new(5.5, 8.5)));
        assert!(!map.line_of_sight(Vec2::new(2.5, 2.5), Vec2::new(20.5, 2.5)));
    }
}

//! Gamepad support.
//!
//! A connected controller is polled once per frame. The raw snapshot is turned
//! into two things: one-shot menu/game actions (edges, so a held D-pad does not
//! scroll a menu at 60 steps a second) and analog motion for the player. All of
//! the mapping lives in pure functions so it can be tested without hardware;
//! only [`Pad::poll`] touches `gilrs`.

use gilrs::{Axis, Button, Gilrs};

/// Sticks smaller than this are treated as centred.
pub const DEADZONE: f32 = 0.22;
/// How far a stick must lean to register as a menu direction.
pub const NAV: f32 = 0.6;
/// Right trigger pressure that counts as firing.
pub const TRIGGER: f32 = 0.35;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PadAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Start,
    Use,
    PrevWeapon,
    NextWeapon,
    Map,
}

impl PadAction {
    pub const ALL: [PadAction; 11] = [
        PadAction::Up,
        PadAction::Down,
        PadAction::Left,
        PadAction::Right,
        PadAction::Confirm,
        PadAction::Back,
        PadAction::Start,
        PadAction::Use,
        PadAction::PrevWeapon,
        PadAction::NextWeapon,
        PadAction::Map,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|a| *a == self).unwrap_or(0)
    }
}

/// A controller reading, before any interpretation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Raw {
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub south: bool,
    pub east: bool,
    pub west: bool,
    pub north: bool,
    pub start: bool,
    pub select: bool,
    pub shoulder_l: bool,
    pub shoulder_r: bool,
    pub stick_lx: f32,
    pub stick_ly: f32,
    pub stick_rx: f32,
    pub trigger_r: f32,
}

/// Analog movement for the frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    /// +1 forward, -1 back.
    pub forward: f32,
    /// +1 right, -1 left.
    pub strafe: f32,
    /// +1 turn right, -1 turn left.
    pub turn: f32,
    pub fire: bool,
}

/// Drop the centre drift and rescale the rest back to a full range.
pub fn deadzone(value: f32) -> f32 {
    let v = value.clamp(-1.0, 1.0);
    if v.abs() <= DEADZONE {
        return 0.0;
    }
    let scaled = (v.abs() - DEADZONE) / (1.0 - DEADZONE);
    scaled.copysign(v)
}

/// Which actions are currently held.
pub fn held(raw: &Raw) -> [bool; PadAction::ALL.len()] {
    let lx = deadzone(raw.stick_lx);
    let ly = deadzone(raw.stick_ly);
    let mut held = [false; PadAction::ALL.len()];
    let mut set = |action: PadAction, value: bool| held[action.index()] = value;
    // gilrs reports the left stick Y axis as negative when pushed up.
    set(PadAction::Up, raw.dpad_up || ly < -NAV);
    set(PadAction::Down, raw.dpad_down || ly > NAV);
    set(PadAction::Left, raw.dpad_left || lx < -NAV);
    set(PadAction::Right, raw.dpad_right || lx > NAV);
    set(PadAction::Confirm, raw.south);
    set(PadAction::Back, raw.east);
    set(PadAction::Start, raw.start);
    // North doubles for "use", so either face button works.
    set(PadAction::Use, raw.west || raw.north);
    set(PadAction::Map, raw.select);
    set(PadAction::PrevWeapon, raw.shoulder_l);
    set(PadAction::NextWeapon, raw.shoulder_r);
    held
}

/// The actions that went from up to down since `prev`.
pub fn pressed(prev: &[bool], now: &[bool]) -> Vec<PadAction> {
    PadAction::ALL
        .iter()
        .copied()
        .filter(|a| now[a.index()] && !prev[a.index()])
        .collect()
}

/// Same, but `invert_forward` flips the left stick's forward axis for players
/// who prefer pushing down to walk forward.
pub fn motion_with(raw: &Raw, invert_forward: bool) -> Motion {
    let forward = -deadzone(raw.stick_ly);
    Motion {
        forward: if invert_forward { -forward } else { forward },
        strafe: deadzone(raw.stick_lx),
        turn: deadzone(raw.stick_rx),
        fire: raw.trigger_r > TRIGGER,
    }
}

/// Controller plugged in or pulled out, for the on-screen note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PadEvent {
    Connected(String),
    Disconnected,
}

pub struct Pad {
    gilrs: Gilrs,
    prev: [bool; PadAction::ALL.len()],
}

impl Pad {
    /// Open the gamepad layer. Silence is fine: no controller, no problem.
    pub fn open() -> Option<Pad> {
        match Gilrs::new() {
            Ok(gilrs) => Some(Pad {
                gilrs,
                prev: [false; PadAction::ALL.len()],
            }),
            Err(e) => {
                eprintln!("gamepad: {e}");
                None
            }
        }
    }

    fn raw(&self) -> Option<Raw> {
        let (_id, pad) = self.gilrs.gamepads().next()?;
        Some(Raw {
            dpad_up: pad.is_pressed(Button::DPadUp),
            dpad_down: pad.is_pressed(Button::DPadDown),
            dpad_left: pad.is_pressed(Button::DPadLeft),
            dpad_right: pad.is_pressed(Button::DPadRight),
            south: pad.is_pressed(Button::South),
            east: pad.is_pressed(Button::East),
            west: pad.is_pressed(Button::West),
            north: pad.is_pressed(Button::North),
            start: pad.is_pressed(Button::Start),
            select: pad.is_pressed(Button::Select), // gilrs maps the bumpers to LeftTrigger/RightTrigger and the analog
            // triggers to the …Trigger2 buttons.
            shoulder_l: pad.is_pressed(Button::LeftTrigger),
            shoulder_r: pad.is_pressed(Button::RightTrigger),
            stick_lx: pad.value(Axis::LeftStickX),
            stick_ly: pad.value(Axis::LeftStickY),
            stick_rx: pad.value(Axis::RightStickX),
            trigger_r: pad
                .button_data(Button::RightTrigger2)
                .map(|d| d.value())
                .unwrap_or(0.0),
        })
    }

    /// Pump the driver and return the newly pressed actions, stick motion and
    /// any connect/disconnect that happened since the last frame.
    pub fn poll(&mut self, invert_forward: bool) -> (Vec<PadAction>, Motion, Option<PadEvent>) {
        let mut event = None;
        while let Some(ev) = self.gilrs.next_event() {
            match ev.event {
                gilrs::EventType::Connected => {
                    let name = self.gilrs.gamepad(ev.id).name().to_string();
                    event = Some(PadEvent::Connected(name));
                }
                gilrs::EventType::Disconnected => event = Some(PadEvent::Disconnected),
                _ => {}
            }
        }
        let Some(raw) = self.raw() else {
            self.prev = [false; PadAction::ALL.len()];
            return (Vec::new(), Motion::default(), event);
        };
        let now = held(&raw);
        let actions = pressed(&self.prev, &now);
        self.prev = now;
        (actions, motion_with(&raw, invert_forward), event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> Raw {
        Raw::default()
    }

    #[test]
    fn deadzone_removes_drift_and_keeps_full_range() {
        assert_eq!(deadzone(0.0), 0.0);
        assert_eq!(deadzone(0.1), 0.0);
        assert_eq!(deadzone(-0.2), 0.0);
        assert!(deadzone(1.0) > 0.99);
        assert!(deadzone(-1.0) < -0.99);
        let mid = deadzone(0.6);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn dpad_and_stick_both_navigate() {
        let mut r = raw();
        r.dpad_down = true;
        assert!(held(&r)[PadAction::Down.index()]);
        assert!(!held(&r)[PadAction::Up.index()]);

        let mut r = raw();
        r.stick_ly = 0.9;
        assert!(
            held(&r)[PadAction::Down.index()],
            "stick down should scroll down"
        );
        r.stick_ly = -0.9;
        assert!(held(&r)[PadAction::Up.index()], "stick up should scroll up");

        // Small stick movement must not scroll menus.
        let mut r = raw();
        r.stick_ly = 0.3;
        let h = held(&r);
        assert!(!h[PadAction::Up.index()] && !h[PadAction::Down.index()]);
    }

    #[test]
    fn buttons_map_to_actions() {
        let mut r = raw();
        r.south = true;
        assert!(held(&r)[PadAction::Confirm.index()]);
        r = raw();
        r.east = true;
        assert!(held(&r)[PadAction::Back.index()]);
        r = raw();
        r.start = true;
        assert!(held(&r)[PadAction::Start.index()]);
        r = raw();
        r.north = true;
        assert!(held(&r)[PadAction::Use.index()]);
        r = raw();
        r.shoulder_r = true;
        assert!(held(&r)[PadAction::NextWeapon.index()]);
        r = raw();
        r.shoulder_l = true;
        assert!(held(&r)[PadAction::PrevWeapon.index()]);
        r = raw();
        r.select = true;
        assert!(held(&r)[PadAction::Map.index()]);
    }

    #[test]
    fn actions_fire_once_per_press() {
        let mut r = raw();
        r.south = true;
        let down = held(&r);
        assert_eq!(pressed(&[false; 11], &down), vec![PadAction::Confirm]);
        // Still held: no repeat.
        assert!(pressed(&down, &down).is_empty());
        // Released and pressed again: fires again.
        let up = held(&raw());
        assert!(pressed(&down, &up).is_empty());
        assert_eq!(pressed(&up, &down), vec![PadAction::Confirm]);
    }

    #[test]
    fn sticks_drive_motion() {
        let mut r = raw();
        r.stick_ly = -1.0;
        r.stick_lx = 0.5;
        r.stick_rx = -0.8;
        r.trigger_r = 1.0;
        let m = motion_with(&r, false);
        assert!(m.forward > 0.9, "stick up is forward, got {}", m.forward);
        assert!(m.strafe > 0.0 && m.strafe < 1.0);
        assert!(m.turn < 0.0, "stick left turns left");
        assert!(m.fire);

        let r = raw();
        let m = motion_with(&r, false);
        assert_eq!((m.forward, m.strafe, m.turn), (0.0, 0.0, 0.0));
        assert!(!m.fire);
    }

    #[test]
    fn inverting_flips_only_forward() {
        let mut r = raw();
        r.stick_ly = -0.9;
        r.stick_lx = 0.5;
        r.stick_rx = 0.4;
        let normal = motion_with(&r, false);
        let inverted = motion_with(&r, true);
        assert!(normal.forward > 0.0);
        assert!(inverted.forward < 0.0);
        assert_eq!(normal.strafe, inverted.strafe);
        assert_eq!(normal.turn, inverted.turn);
    }
}

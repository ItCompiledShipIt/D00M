//! Persisted player settings.
//!
//! Stored as a tiny `key value` text file in the user's config directory, so it
//! needs no extra crates. Unknown or broken lines are ignored and every value
//! is clamped, so a hand-edited or truncated file can never produce a settings
//! value the menus cannot show.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::game::Difficulty;
use crate::ui::Settings;

/// How many episode runs the high-score table keeps.
pub const MAX_SCORES: usize = 5;

/// One completed episode, as shown on the final screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub kills: usize,
    pub time: f32,
    pub difficulty: Difficulty,
}

/// Add a finished run and return its rank, or `None` if it did not place.
pub fn insert_score(scores: &mut Vec<Score>, score: Score) -> Option<usize> {
    scores.push(score);
    scores.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.time.total_cmp(&b.time)));
    scores.truncate(MAX_SCORES);
    scores.iter().position(|s| *s == score)
}

fn scores_path() -> Option<PathBuf> {
    path().map(|p| p.with_file_name("scores.txt"))
}

pub fn load_scores() -> Vec<Score> {
    match scores_path() {
        Some(path) => match fs::read_to_string(path) {
            Ok(text) => parse_scores(&text),
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    }
}

pub fn save_scores(scores: &[Score]) {
    if let Some(path) = scores_path()
        && let Some(parent) = path.parent()
    {
        let _ = fs::create_dir_all(parent);
        let _ = fs::write(path, format_scores(scores));
    }
}

fn format_scores(scores: &[Score]) -> String {
    // One run per line: kills, seconds, skill index.
    let mut out = String::from("# D00M scores: kills time skill\n");
    for s in scores {
        out.push_str(&format!(
            "{} {:.1} {}\n",
            s.kills,
            s.time,
            s.difficulty.index()
        ));
    }
    out
}

fn parse_scores(text: &str) -> Vec<Score> {
    let mut scores = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let kills = parts.next().and_then(|v| v.parse::<usize>().ok());
        let time = parts.next().and_then(|v| v.parse::<f32>().ok());
        let skill = parts.next().and_then(|v| v.parse::<usize>().ok());
        if let (Some(kills), Some(time), Some(skill)) = (kills, time, skill)
            && time.is_finite()
            && time >= 0.0
        {
            scores.push(Score {
                kills,
                time,
                difficulty: Difficulty::from_index(skill),
            });
        }
    }
    scores.sort_by(|a, b| b.kills.cmp(&a.kills).then(a.time.total_cmp(&b.time)));
    scores.truncate(MAX_SCORES);
    scores
}

/// Where the settings file lives, following the usual platform conventions.
pub fn path() -> Option<PathBuf> {
    for var in ["XDG_CONFIG_HOME", "HOME", "USERPROFILE"] {
        if let Some(dir) = std::env::var_os(var).filter(|v| !v.is_empty()) {
            let base = PathBuf::from(dir);
            return Some(if var == "XDG_CONFIG_HOME" {
                base.join("d00m").join("config.txt")
            } else {
                base.join(".config").join("d00m").join("config.txt")
            });
        }
    }
    None
}

/// Load the saved settings, falling back to the defaults.
pub fn load() -> Settings {
    match path() {
        Some(path) => load_from(&path),
        None => Settings::default(),
    }
}

/// Save the settings, ignoring failures (a read-only home should not break play).
pub fn save(settings: &Settings) {
    if let Some(path) = path() {
        let _ = save_to(&path, settings);
    }
}

pub fn load_from(path: &Path) -> Settings {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(_) => Settings::default(),
    }
}

pub fn save_to(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format(settings))
}

fn format(settings: &Settings) -> String {
    let mut out = String::from("# D00M settings\n");
    out.push_str(&format!("sensitivity {:.2}\n", settings.sensitivity));
    out.push_str(&format!(
        "stick_sensitivity {:.2}\n",
        settings.stick_sensitivity
    ));
    out.push_str(&format!(
        "invert_stick {}\n",
        if settings.invert_stick { 1 } else { 0 }
    ));
    out.push_str(&format!("master {:.2}\n", settings.master));
    out.push_str(&format!("sfx {:.2}\n", settings.sfx));
    out.push_str(&format!("ambient {:.2}\n", settings.ambient));
    out.push_str(&format!(
        "fullscreen {}\n",
        if settings.fullscreen { 1 } else { 0 }
    ));
    out.push_str(&format!("difficulty {}\n", settings.difficulty.index()));
    out
}

fn parse(text: &str) -> Settings {
    let mut settings = Settings::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(|c: char| c.is_whitespace()) else {
            continue;
        };
        let Ok(value) = value.trim().parse::<f32>() else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "sensitivity" => {
                settings.sensitivity = value.clamp(Settings::SENS_MIN, Settings::SENS_MAX);
            }
            "stick_sensitivity" | "stick" => {
                settings.stick_sensitivity = value.clamp(Settings::SENS_MIN, Settings::SENS_MAX);
            }
            "invert_stick" | "invert" => settings.invert_stick = value >= 0.5,
            "master" => settings.master = value.clamp(0.0, 1.0),
            "sfx" => settings.sfx = value.clamp(0.0, 1.0),
            "ambient" => settings.ambient = value.clamp(0.0, 1.0),
            "fullscreen" => settings.fullscreen = value >= 0.5,
            "difficulty" => settings.difficulty = Difficulty::from_index(value.max(0.0) as usize),
            _ => {}
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("d00m-config-{}", std::process::id()));
        let path = dir.join("config.txt");
        let settings = Settings {
            sensitivity: 1.7,
            stick_sensitivity: 0.6,
            invert_stick: true,
            master: 0.35,
            sfx: 1.0,
            ambient: 0.0,
            fullscreen: true,
            difficulty: Difficulty::Hard,
        };

        save_to(&path, &settings).expect("save");
        let loaded = load_from(&path);
        assert!((loaded.sensitivity - 1.7).abs() < 0.01);
        assert!((loaded.stick_sensitivity - 0.6).abs() < 0.01);
        assert!(loaded.invert_stick, "invert flag was not saved");
        assert!((loaded.master - 0.35).abs() < 0.01);
        assert_eq!(loaded.sfx, 1.0);
        assert_eq!(loaded.ambient, 0.0);
        assert!(loaded.fullscreen, "fullscreen flag was not saved");
        assert_eq!(loaded.difficulty, Difficulty::Hard);
        // Toggling back off has to persist too.
        save_to(&path, &Settings::default()).expect("save");
        assert!(!load_from(&path).fullscreen);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_uses_defaults() {
        let path = std::env::temp_dir().join("d00m-config-does-not-exist.txt");
        fs::remove_file(&path).ok();
        let loaded = load_from(&path);
        let default = Settings::default();
        assert_eq!(loaded.sensitivity, default.sensitivity);
        assert_eq!(loaded.master, default.master);
        assert_eq!(loaded.sfx, default.sfx);
        assert_eq!(loaded.ambient, default.ambient);
    }

    #[test]
    fn junk_is_ignored_and_values_clamped() {
        let loaded = parse(
            "# comment\n\
             sensitivity 99\n\
             master -3\n\
             sfx 0.5\n\
             ambient not-a-number\n\
             banana 1.0\n\
             garbage\n",
        );
        assert_eq!(loaded.sensitivity, Settings::SENS_MAX);
        assert_eq!(loaded.master, 0.0);
        assert_eq!(loaded.sfx, 0.5);
        assert_eq!(loaded.ambient, Settings::default().ambient);
    }

    #[test]
    fn unknown_keys_keep_defaults() {
        let loaded = parse("fov 120\nvolume 5\n");
        let default = Settings::default();
        assert_eq!(loaded.sensitivity, default.sensitivity);
        assert_eq!(loaded.master, default.master);
    }

    fn score(kills: usize, time: f32) -> Score {
        Score {
            kills,
            time,
            difficulty: Difficulty::Normal,
        }
    }

    #[test]
    fn scores_rank_by_kills_then_time() {
        let mut scores = Vec::new();
        assert_eq!(insert_score(&mut scores, score(10, 100.0)), Some(0));
        assert_eq!(insert_score(&mut scores, score(30, 500.0)), Some(0));
        assert_eq!(insert_score(&mut scores, score(10, 50.0)), Some(1));
        assert_eq!(scores[0].kills, 30);
        assert_eq!(scores[1].time, 50.0);
        assert_eq!(scores[2].time, 100.0);
    }

    #[test]
    fn scores_keep_only_the_best_and_round_trip() {
        let mut scores = Vec::new();
        for i in 0..MAX_SCORES + 3 {
            insert_score(
                &mut scores,
                Score {
                    kills: i,
                    time: 100.0,
                    difficulty: Difficulty::Easy,
                },
            );
        }
        assert_eq!(scores.len(), MAX_SCORES);
        assert_eq!(scores[0].kills, MAX_SCORES + 2);

        let loaded = parse_scores(&format_scores(&scores));
        assert_eq!(loaded, scores);

        // A run that does not beat any of them does not place.
        let mut ranked = scores.clone();
        let weak = Score {
            kills: 0,
            time: 9999.0,
            difficulty: Difficulty::Easy,
        };
        assert_eq!(insert_score(&mut ranked, weak), None);
        assert_eq!(ranked, scores);
    }

    #[test]
    fn broken_score_lines_are_skipped() {
        let loaded = parse_scores("# header\n30 200.0 2\nnonsense\n10 abc 1\n4 90.5\n");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].kills, 30);
        assert_eq!(loaded[0].difficulty, Difficulty::Hard);
    }

    /// The whole path: `save` picks the config dir, creates it, and `load`
    /// finds the file again. Uses its own XDG directory so nothing real is
    /// touched.
    #[test]
    fn save_and_load_use_the_config_directory() {
        let dir = std::env::temp_dir().join(format!("d00m-xdg-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        // SAFETY: this test is the only one that reads or writes these vars.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &dir);
        }
        let settings = Settings {
            sensitivity: 1.3,
            stick_sensitivity: 1.9,
            invert_stick: false,
            master: 0.25,
            sfx: 0.5,
            ambient: 0.75,
            fullscreen: false,
            difficulty: Difficulty::Easy,
        };
        save(&settings);
        let expected = dir.join("d00m").join("config.txt");
        assert!(expected.is_file(), "{expected:?} was not created");

        let loaded = load();
        assert!((loaded.sensitivity - 1.3).abs() < 0.01);
        assert!((loaded.stick_sensitivity - 1.9).abs() < 0.01);
        assert!((loaded.master - 0.25).abs() < 0.01);
        assert_eq!(loaded.difficulty, Difficulty::Easy);

        let scores = vec![score(9, 120.0)];
        save_scores(&scores);
        assert_eq!(load_scores(), scores);
        assert!((loaded.sfx - 0.5).abs() < 0.01);
        assert!((loaded.ambient - 0.75).abs() < 0.01);

        fs::remove_dir_all(&dir).ok();
        unsafe {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

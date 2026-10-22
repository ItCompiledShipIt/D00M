//! Procedural sound effects, an ambient loop and a small software mixer.
//!
//! Everything is synthesised at start-up (no wav files). The mixer runs on the
//! `cpal` output callback and receives commands over a channel: play an effect,
//! change the three volume levels, or duck the ambient bed. If no audio device
//! can be opened the game simply runs silent.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use crate::math::Rng;

const TAU: f32 = std::f32::consts::TAU;
const MAX_VOICES: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum SoundId {
    Pistol = 0,
    Shotgun,
    Explosion,
    PlayerHurt,
    Pickup,
    Empty,
    MenuMove,
    MenuSelect,
    DoorOpen,
    DoorClose,
    ImpSight,
    ImpAttack,
    ImpPain,
    ImpDeath,
    Chaingun,
    RocketLaunch,
    DemonDeath,
    KnightDeath,
    BossDeath,
    DemonSight,
    DemonPain,
    DemonAttack,
    KnightSight,
    KnightPain,
    KnightAttack,
    BossSight,
    BossPain,
    BossAttack,
}

const SOUND_COUNT: usize = 28;

#[derive(Clone, Copy, Debug)]
pub struct SoundRequest {
    pub id: SoundId,
    /// Linear gain 0..1 before the mixer's sfx/master volume.
    pub gain: f32,
    /// -1 hard left, 0 centre, +1 hard right.
    pub pan: f32,
}

enum Command {
    Play(SoundRequest),
    Volumes {
        master: f32,
        sfx: f32,
        ambient: f32,
    },
    /// 0..1 multiplier used to duck the ambient bed for menus and pause.
    AmbientGain(f32),
    /// Crossfade the music bed in (gameplay) or out (everywhere else).
    Music(bool),
}

/// Pre-rendered effects, indexed by `SoundId as usize`, plus the ambient
/// loop and the music loop.
pub struct Bank {
    pub sounds: Vec<Arc<Vec<f32>>>,
    pub ambient: Arc<Vec<f32>>,
    pub music: Arc<Vec<f32>>,
}

impl Bank {
    pub fn generate(sample_rate: f32) -> Self {
        let sounds = vec![
            Arc::new(pistol(sample_rate)),
            Arc::new(shotgun(sample_rate)),
            Arc::new(explosion(sample_rate)),
            Arc::new(player_hurt(sample_rate)),
            Arc::new(pickup(sample_rate)),
            Arc::new(empty_click(sample_rate)),
            Arc::new(menu_move(sample_rate)),
            Arc::new(menu_select(sample_rate)),
            Arc::new(door_open(sample_rate)),
            Arc::new(door_close(sample_rate)),
            Arc::new(imp_sight(sample_rate)),
            Arc::new(imp_attack(sample_rate)),
            Arc::new(imp_pain(sample_rate)),
            Arc::new(imp_death(sample_rate)),
            Arc::new(chaingun(sample_rate)),
            Arc::new(rocket_launch(sample_rate)),
            Arc::new(demon_death(sample_rate)),
            Arc::new(knight_death(sample_rate)),
            Arc::new(boss_death(sample_rate)),
            Arc::new(demon_sight(sample_rate)),
            Arc::new(demon_pain(sample_rate)),
            Arc::new(demon_attack(sample_rate)),
            Arc::new(knight_sight(sample_rate)),
            Arc::new(knight_pain(sample_rate)),
            Arc::new(knight_attack(sample_rate)),
            Arc::new(boss_sight(sample_rate)),
            Arc::new(boss_pain(sample_rate)),
            Arc::new(boss_attack(sample_rate)),
        ];
        debug_assert_eq!(sounds.len(), SOUND_COUNT);
        Self {
            sounds,
            ambient: Arc::new(ambient(sample_rate)),
            music: Arc::new(music_loop(sample_rate)),
        }
    }
}

struct Voice {
    samples: Arc<Vec<f32>>,
    pos: usize,
    gain_l: f32,
    gain_r: f32,
}

pub struct Mixer {
    bank: Arc<Bank>,
    voices: Vec<Voice>,
    ambient_pos: usize,
    music_pos: usize,
    music_on: bool,
    music_mix: f32,
    master: f32,
    sfx: f32,
    ambient_vol: f32,
    duck: f32,
}

impl Mixer {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self {
            bank,
            voices: Vec::with_capacity(MAX_VOICES),
            ambient_pos: 0,
            music_pos: 0,
            music_on: false,
            music_mix: 0.0,
            master: 1.0,
            sfx: 1.0,
            ambient_vol: 0.7,
            duck: 0.0,
        }
    }

    pub fn set_music(&mut self, on: bool) {
        self.music_on = on;
    }

    pub fn set_volumes(&mut self, master: f32, sfx: f32, ambient: f32) {
        self.master = master.clamp(0.0, 1.0);
        self.sfx = sfx.clamp(0.0, 1.0);
        self.ambient_vol = ambient.clamp(0.0, 1.0);
    }

    pub fn set_duck(&mut self, duck: f32) {
        self.duck = duck.clamp(0.0, 1.0);
    }

    pub fn trigger(&mut self, req: SoundRequest) {
        let Some(samples) = self.bank.sounds.get(req.id as usize).cloned() else {
            return;
        };
        if self.voices.len() >= MAX_VOICES {
            self.voices.remove(0);
        }
        let pan = req.pan.clamp(-1.0, 1.0);
        // Equal-power panning keeps loudness constant across the stereo field.
        let left = ((1.0 - pan) * 0.5).sqrt();
        let right = ((1.0 + pan) * 0.5).sqrt();
        let gain = req.gain.clamp(0.0, 2.0);
        self.voices.push(Voice {
            samples,
            pos: 0,
            gain_l: left * gain,
            gain_r: right * gain,
        });
    }

    /// Fill an interleaved output buffer, converting from f32 to the device format.
    pub fn render<T>(&mut self, data: &mut [T], channels: usize)
    where
        T: SizedSample + FromSample<f32>,
    {
        let channels = channels.max(1);
        let sfx_gain = self.master * self.sfx;
        let ambient_gain = self.master * self.ambient_vol * self.duck * 0.4;
        let music_gain = self.master * self.ambient_vol * self.duck * 0.55;
        let ambient_len = self.bank.ambient.len();
        let music_len = self.bank.music.len();
        for frame in data.chunks_mut(channels) {
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for voice in &mut self.voices {
                if let Some(sample) = voice.samples.get(voice.pos) {
                    l += sample * voice.gain_l * sfx_gain;
                    r += sample * voice.gain_r * sfx_gain;
                    voice.pos += 1;
                }
            }
            // Crossfade the music over roughly a second and a half; the
            // drone recedes underneath it while it plays.
            if self.music_on && self.music_mix < 1.0 {
                self.music_mix = (self.music_mix + 1.0 / 72_000.0).min(1.0);
            } else if !self.music_on && self.music_mix > 0.0 {
                self.music_mix = (self.music_mix - 1.0 / 72_000.0).max(0.0);
            }
            if ambient_gain > 0.0 && ambient_len > 0 {
                let sample =
                    self.bank.ambient[self.ambient_pos] * ambient_gain * (1.0 - 0.7 * self.music_mix);
                l += sample;
                r += sample;
                self.ambient_pos = (self.ambient_pos + 1) % ambient_len;
            }
            if self.music_mix > 0.001 && music_len > 0 {
                let sample = self.bank.music[self.music_pos] * music_gain * self.music_mix;
                l += sample;
                r += sample;
                self.music_pos = (self.music_pos + 1) % music_len;
            }
            let l = l.tanh();
            let r = r.tanh();
            match channels {
                1 => frame[0] = T::from_sample((l + r) * 0.5),
                _ => {
                    frame[0] = T::from_sample(l);
                    frame[1] = T::from_sample(r);
                    for out in frame.iter_mut().skip(2) {
                        *out = T::from_sample((l + r) * 0.5);
                    }
                }
            }
        }
        self.voices.retain(|v| v.pos < v.samples.len());
    }
}

/// Owns the live output stream; dropping it stops audio.
pub struct Audio {
    _stream: cpal::Stream,
    tx: Sender<Command>,
}

impl Audio {
    pub fn start() -> Option<Audio> {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                eprintln!("audio: no output device, running silent");
                return None;
            }
        };
        let supported = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("audio: no output config ({e}), running silent");
                return None;
            }
        };
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.config();
        let sample_rate = config.sample_rate as f32;
        let channels = config.channels as usize;
        let bank = Arc::new(Bank::generate(sample_rate));
        let (tx, rx) = mpsc::channel();

        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, config, bank, rx),
            cpal::SampleFormat::I16 => build::<i16>(&device, config, bank, rx),
            cpal::SampleFormat::U16 => build::<u16>(&device, config, bank, rx),
            other => {
                eprintln!("audio: unsupported sample format {other:?}, running silent");
                return None;
            }
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("audio: could not build stream ({e}), running silent");
                return None;
            }
        };
        if let Err(e) = stream.play() {
            eprintln!("audio: could not start stream ({e}), running silent");
            return None;
        }
        eprintln!("audio: {sample_rate} Hz, {channels} ch, {SOUND_COUNT} effects + ambience + music");
        Some(Audio {
            _stream: stream,
            tx,
        })
    }

    pub fn play(&self, request: SoundRequest) {
        let _ = self.tx.send(Command::Play(request));
    }

    pub fn set_volumes(&self, master: f32, sfx: f32, ambient: f32) {
        let _ = self.tx.send(Command::Volumes {
            master,
            sfx,
            ambient,
        });
    }

    pub fn set_ambient_gain(&self, gain: f32) {
        let _ = self.tx.send(Command::AmbientGain(gain));
    }

    pub fn set_music(&self, on: bool) {
        let _ = self.tx.send(Command::Music(on));
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    bank: Arc<Bank>,
    rx: Receiver<Command>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut mixer = Mixer::new(bank);
    device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            while let Ok(command) = rx.try_recv() {
                match command {
                    Command::Play(request) => mixer.trigger(request),
                    Command::Volumes {
                        master,
                        sfx,
                        ambient,
                    } => mixer.set_volumes(master, sfx, ambient),
                    Command::AmbientGain(gain) => mixer.set_duck(gain),
                    Command::Music(on) => mixer.set_music(on),
                }
            }
            mixer.render(data, channels);
        },
        |err| eprintln!("audio: stream error: {err}"),
        None,
    )
}

// --- Synthesis -------------------------------------------------------------

fn normalize(mut samples: Vec<f32>, peak: f32) -> Vec<f32> {
    let max = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 1e-6 {
        let gain = peak / max;
        for s in &mut samples {
            *s *= gain;
        }
    }
    let fade = (samples.len() / 200).max(1);
    let n = samples.len();
    for i in 0..fade.min(n) {
        let g = i as f32 / fade as f32;
        samples[n - 1 - i] *= g;
    }
    samples
}

fn saw(phase: f32) -> f32 {
    2.0 * (phase / TAU - (phase / TAU).floor()) - 1.0
}

fn pistol(sr: f32) -> Vec<f32> {
    let n = (sr * 0.22) as usize;
    let mut rng = Rng::new(0x51A7);
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.35;
        let crack = noise * (-t * 150.0).exp();
        let body = lp * (-t * 24.0).exp();
        let thump = (TAU * 92.0 * t).sin() * (-t * 20.0).exp();
        let attack = (t * 900.0).min(1.0);
        out.push((crack * 0.9 + body * 0.6 + thump * 0.55) * attack);
    }
    normalize(out, 0.85)
}

/// A short, dry mechanical bark so a held trigger does not turn into mush.
fn chaingun(sr: f32) -> Vec<f32> {
    let n = (sr * 0.16) as usize;
    let mut rng = Rng::new(0x6A11);
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.45;
        let crack = noise * (-t * 260.0).exp();
        let body = lp * (-t * 40.0).exp();
        let thump = (TAU * 128.0 * t).sin() * (-t * 34.0).exp();
        // Slight metallic ring from the spinning barrels.
        let ring = (TAU * 620.0 * t).sin() * (-t * 60.0).exp() * 0.18;
        let attack = (t * 1400.0).min(1.0);
        out.push((crack * 0.7 + body * 0.5 + thump * 0.5 + ring) * attack);
    }
    normalize(out, 0.75)
}

/// Launch whoosh: a noise sweep that falls away, with a low thump.
fn rocket_launch(sr: f32) -> Vec<f32> {
    let dur = 0.6f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x70C4);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * (0.5 - 0.4 * (t / dur));
        let freq = 70.0 + 40.0 * (-t * 6.0).exp();
        phase += TAU * freq / sr;
        let thump = (TAU * 48.0 * t).sin() * (-t * 9.0).exp();
        let hiss = lp * (-t * 3.2).exp() * 1.4;
        let puff = (phase / TAU).fract() * 2.0 - 1.0;
        let attack = (t * 400.0).min(1.0);
        out.push((thump * 0.9 + hiss + puff * 0.25 * (-t * 7.0).exp()) * attack);
    }
    normalize(out, 0.9)
}

fn shotgun(sr: f32) -> Vec<f32> {
    let n = (sr * 0.42) as usize;
    let mut rng = Rng::new(0xB005);
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.18;
        let blast = noise * (-t * 55.0).exp();
        let body = lp * (-t * 12.0).exp();
        let thump = (TAU * 66.0 * t).sin() * (-t * 9.0).exp();
        let second = if t > 0.055 {
            let t2 = t - 0.055;
            (TAU * 54.0 * t2).sin() * (-t2 * 14.0).exp() * 0.5
        } else {
            0.0
        };
        let attack = (t * 700.0).min(1.0);
        out.push((blast * 0.85 + body * 0.9 + thump * 0.9 + second) * attack);
    }
    normalize(out, 0.95)
}

fn explosion(sr: f32) -> Vec<f32> {
    let dur = 1.0f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0xE901);
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.08;
        let boom = (TAU * 42.0 * t).sin() * (-t * 5.0).exp();
        let rumble = lp * (-t * 2.6).exp();
        let crackle = if noise > 0.93 {
            noise * (-t * 6.0).exp() * 0.6
        } else {
            0.0
        };
        let attack = (t * 500.0).min(1.0);
        out.push((boom * 0.9 + rumble * 1.1 + crackle) * attack);
    }
    normalize(out, 0.95)
}

fn player_hurt(sr: f32) -> Vec<f32> {
    let dur = 0.3f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x9A11);
    let mut phase = 0.0f32;
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.2;
        let freq = 150.0 - 60.0 * (t / dur);
        phase += TAU * freq / sr;
        let voice = saw(phase) * 0.5 + lp * 0.5;
        let env = (t * 80.0).min(1.0) * (-t * 11.0).exp();
        out.push(voice * env);
    }
    normalize(out, 0.7)
}

fn pickup(sr: f32) -> Vec<f32> {
    let dur = 0.16f32;
    let n = (sr * dur) as usize;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let f = if t < 0.06 { 520.0 } else { 780.0 };
        phase += TAU * f / sr;
        let env = (t * 300.0).min(1.0) * (-t * 26.0).exp();
        out.push((phase.sin() * 0.6 + (phase * 2.0).sin() * 0.2) * env);
    }
    normalize(out, 0.55)
}

fn empty_click(sr: f32) -> Vec<f32> {
    let dur = 0.07f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0xEC01);
    let mut lp = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.5;
        let click = (noise - lp) * (-t * 120.0).exp();
        let tick = (TAU * 1400.0 * t).sin() * (-t * 90.0).exp();
        out.push(click * 0.7 + tick * 0.5);
    }
    normalize(out, 0.5)
}

fn menu_move(sr: f32) -> Vec<f32> {
    let dur = 0.06f32;
    let n = (sr * dur) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let env = (t * 400.0).min(1.0) * (-t * 70.0).exp();
        out.push((TAU * 720.0 * t).sin() * env);
    }
    normalize(out, 0.35)
}

fn menu_select(sr: f32) -> Vec<f32> {
    let dur = 0.18f32;
    let n = (sr * dur) as usize;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let f = 380.0 + 620.0 * (t / dur);
        phase += TAU * f / sr;
        let env = (t * 200.0).min(1.0) * (-t * 16.0).exp();
        out.push((saw(phase) * 0.35 + phase.sin() * 0.4) * env);
    }
    normalize(out, 0.45)
}

fn door_open(sr: f32) -> Vec<f32> {
    let dur = 0.75f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0xD001);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.12;
        let freq = 68.0 + 58.0 * (t / dur);
        phase += TAU * freq / sr;
        let motor = phase.sin() * 0.45 + (phase * 2.0).sin() * 0.22 + (phase * 3.0).sin() * 0.1;
        let grind = lp * 0.45 * (0.65 + 0.35 * (t * 46.0).sin());
        let clunk = if t < 0.05 {
            noise * (-t * 55.0).exp() * 0.7
        } else {
            0.0
        };
        let env = (t * 12.0).min(1.0) * (1.0 - t / dur).powf(0.55);
        out.push((motor + grind + clunk) * env);
    }
    normalize(out, 0.7)
}

fn door_close(sr: f32) -> Vec<f32> {
    let dur = 0.6f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0xD002);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.1;
        let freq = 118.0 - 62.0 * (t / dur);
        phase += TAU * freq / sr;
        let motor = phase.sin() * 0.4 + (phase * 2.0).sin() * 0.18;
        let grind = lp * 0.4 * (0.7 + 0.3 * (t * 38.0).sin());
        let env = (t * 14.0).min(1.0) * (1.0 - t / dur).powf(0.5);
        let mut s = (motor + grind) * env;
        let land = dur - 0.14;
        if t > land {
            let t2 = t - land;
            s += (TAU * 58.0 * t2).sin() * (-t2 * 18.0).exp() * 0.8;
            s += noise * (-t2 * 60.0).exp() * 0.25;
        }
        out.push(s);
    }
    normalize(out, 0.75)
}

fn imp_sight(sr: f32) -> Vec<f32> {
    let dur = 0.8f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A01);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut phase2 = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.09;
        let vib = (t * TAU * 6.5).sin() * 9.0;
        let freq = 92.0 + vib + 18.0 * (t / dur);
        phase += TAU * freq / sr;
        phase2 += TAU * (freq * 1.51) / sr;
        let growl = saw(phase) * 0.5 + saw(phase2) * 0.22 + lp * 0.4;
        let env = (t * 24.0).min(1.0) * (1.0 - t / dur).powf(0.7);
        out.push(growl * env);
    }
    normalize(out, 0.75)
}

fn imp_attack(sr: f32) -> Vec<f32> {
    let dur = 0.45f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A02);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * (0.05 + 0.25 * t / dur);
        let freq = 205.0 - 110.0 * (t / dur);
        phase += TAU * freq / sr;
        let growl = saw(phase) * 0.5 + lp * 0.55;
        let env = (t * 40.0).min(1.0) * (1.0 - t / dur).powf(0.9);
        out.push(growl * env);
    }
    normalize(out, 0.8)
}

fn imp_pain(sr: f32) -> Vec<f32> {
    let dur = 0.26f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A03);
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        let freq = 168.0 + 22.0 * (t * TAU * 22.0).sin();
        phase += TAU * freq / sr;
        let growl = saw(phase) * 0.55 + noise * 0.18;
        let env = (t * 55.0).min(1.0) * (-t * 14.0).exp();
        out.push(growl * env);
    }
    normalize(out, 0.7)
}

fn imp_death(sr: f32) -> Vec<f32> {
    let dur = 0.95f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A04);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.07;
        let freq = (150.0 - 105.0 * (t / dur)).max(38.0) + (t * TAU * 5.0).sin() * 6.0;
        phase += TAU * freq / sr;
        let growl = saw(phase) * 0.5 + lp * 0.5;
        let env = (t * 18.0).min(1.0) * (1.0 - t / dur).powf(1.2);
        out.push(growl * env);
    }
    normalize(out, 0.8)
}

/// The demon dies lower and rougher than the imp: a bigger animal
/// emptying its lungs through gravel.
fn demon_death(sr: f32) -> Vec<f32> {
    let dur = 0.8f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A05);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.11;
        let freq = (118.0 - 84.0 * (t / dur)).max(30.0) + (t * TAU * 4.0).sin() * 5.0;
        phase += TAU * freq / sr;
        let growl = saw(phase) * 0.55 + lp * 0.45;
        let env = (t * 22.0).min(1.0) * (1.0 - t / dur).powf(1.1);
        out.push(growl * env);
    }
    normalize(out, 0.85)
}

/// The hell knight goes down like a falling statue: a deep thud with a
/// metallic ring as its armour hits the floor.
fn knight_death(sr: f32) -> Vec<f32> {
    let dur = 1.15f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A06);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.2;
        let freq = (66.0 - 38.0 * (t / dur)).max(26.0);
        phase += TAU * freq / sr;
        let thud = saw(phase) * 0.5 + lp * (-t * 16.0).exp() * 0.3;
        let ring = ((t * TAU * 523.0).sin() + (t * TAU * 787.0).sin() * 0.6) * (-t * 8.0).exp() * 0.16;
        let env = (t * 26.0).min(1.0) * (1.0 - t / dur).powf(0.9);
        out.push((thud + ring) * env);
    }
    normalize(out, 0.85)
}

/// The Gatekeeper's death is the biggest sound in the game: a sub-bass
/// collapse under breaking stone, with the knight's ring dropped an octave.
fn boss_death(sr: f32) -> Vec<f32> {
    let dur = 1.6f32;
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(0x1A07);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.16;
        let freq = (92.0 - 66.0 * (t / dur)).max(24.0);
        phase += TAU * freq / sr;
        let body = saw(phase) * 0.5 + lp * (-t * 5.0).exp() * 0.45;
        let ring = ((t * TAU * 262.0).sin() + (t * TAU * 392.0).sin() * 0.6) * (-t * 6.0).exp() * 0.14;
        let env = (t * 30.0).min(1.0) * (1.0 - t / dur).powf(0.8);
        out.push((body + ring) * env);
    }
    normalize(out, 0.9)
}

/// A monster voice: a saw growl sliding from `base` Hz down to `end`,
/// roughened by a noise bed (`grit` is the noise share). Each kind gets
/// its own register — the imp's hand-rolled voice sits highest, the
/// Gatekeeper's is nearly subsonic.
fn growl(sr: f32, seed: u64, base: f32, end: f32, dur: f32, grit: f32) -> Vec<f32> {
    let n = (sr * dur) as usize;
    let mut rng = Rng::new(seed);
    let mut lp = 0.0f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let frac = t / dur;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.09;
        let freq = (base + (end - base) * frac).max(24.0) + (t * TAU * 17.0).sin() * 5.0;
        phase += TAU * freq / sr;
        let tone = saw(phase) * (1.0 - grit) + lp * grit;
        let env = (t * 30.0).min(1.0) * (1.0 - frac).powf(1.1);
        out.push(tone * env);
    }
    normalize(out, 0.78)
}

fn demon_sight(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B01, 132.0, 92.0, 0.5, 0.5)
}

fn demon_pain(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B02, 155.0, 112.0, 0.22, 0.45)
}

fn demon_attack(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B03, 104.0, 60.0, 0.34, 0.55)
}

fn knight_sight(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B04, 92.0, 70.0, 0.62, 0.3)
}

fn knight_pain(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B05, 122.0, 96.0, 0.22, 0.3)
}

fn knight_attack(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B06, 82.0, 54.0, 0.4, 0.35)
}

fn boss_sight(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B07, 66.0, 44.0, 0.9, 0.5)
}

fn boss_pain(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B08, 86.0, 60.0, 0.3, 0.45)
}

fn boss_attack(sr: f32) -> Vec<f32> {
    growl(sr, 0x1B09, 56.0, 34.0, 0.55, 0.55)
}

/// Add a harmonic tone (fundamental + two overtones) with an exponential
/// decay and a forced fade at the tail so notes never click off.
fn add_harmonic(
    out: &mut [f32],
    sr: f32,
    start: f32,
    dur: f32,
    freq: f32,
    gain: f32,
    bright: f32,
    decay: f32,
) {
    let s0 = (start * sr) as usize;
    let len = (dur * sr) as usize;
    for i in 0..len {
        let idx = s0 + i;
        if idx >= out.len() {
            break;
        }
        let t = i as f32 / sr;
        let tail = ((1.0 - i as f32 / len as f32) * 8.0).min(1.0);
        let env = (t * 40.0).min(1.0) * (-t * decay).exp() * tail;
        let ph = t * freq * TAU;
        out[idx] +=
            (ph.sin() + bright * (ph * 2.0).sin() * 0.5 + bright * bright * (ph * 3.0).sin() * 0.25)
                * gain
                * env;
    }
}

/// Add a slowly swelling pad tone that starts and ends at zero, so chord
/// changes (and the loop point itself) are seamless.
fn add_pad(out: &mut [f32], sr: f32, start: f32, dur: f32, freq: f32, gain: f32) {
    let s0 = (start * sr) as usize;
    let len = (dur * sr) as usize;
    for i in 0..len {
        let idx = s0 + i;
        if idx >= out.len() {
            break;
        }
        let frac = i as f32 / len as f32;
        let env = (frac * std::f32::consts::PI).sin().powf(1.5);
        let t = i as f32 / sr;
        let ph = t * freq * TAU;
        out[idx] += (ph.sin() + (ph * 2.003).sin() * 0.18) * gain * env;
    }
}

fn add_noise_hit(out: &mut [f32], noise: &[f32], sr: f32, start: f32, dur: f32, gain: f32, decay: f32) {
    let s0 = (start * sr) as usize;
    let len = (dur * sr) as usize;
    for i in 0..len {
        let idx = s0 + i;
        if idx >= out.len() {
            break;
        }
        let t = i as f32 / sr;
        out[idx] += noise[idx % noise.len()] * gain * (-t * decay).exp();
    }
}

fn add_kick(out: &mut [f32], sr: f32, start: f32) {
    let s0 = (start * sr) as usize;
    let len = (0.22 * sr) as usize;
    for i in 0..len {
        let idx = s0 + i;
        if idx >= out.len() {
            break;
        }
        let t = i as f32 / sr;
        let freq = 42.0 + 78.0 * (-t * 30.0).exp();
        out[idx] += (t * freq * TAU).sin() * 0.55 * (-t * 16.0).exp();
    }
}

/// The score: a 16-bar loop in D minor at 92 BPM, rendered note by note —
/// an eighth-note bass driving under minor pads, a sparse bell lead in
/// the second half, and a kit built from sine drops and noise. Every
/// envelope is bar-quantised, so the loop repeats seamlessly.
fn music_loop(sr: f32) -> Vec<f32> {
    const BPM: f32 = 92.0;
    let spb = 60.0 / BPM;
    let bar = spb * 4.0;
    let n = (sr * bar * 16.0) as usize;
    let mut out = vec![0.0f32; n];
    let mut rng = Rng::new(0x4D55_5331);
    let noise: Vec<f32> = (0..n).map(|_| rng.range(-1.0, 1.0)).collect();
    // Two bars each: Dm, Bb, Gm, Dm — twice through.
    let seq = [0usize, 1, 2, 0];
    let roots = [73.42f32, 58.27, 49.0, 73.42];
    let pads = [
        [146.83f32, 174.61, 220.0],
        [116.54, 146.83, 174.61],
        [196.0, 233.08, 293.66],
        [146.83, 174.61, 220.0],
    ];
    for b in 0..16usize {
        let ci = seq[(b / 2) % 4];
        let bs = b as f32 * bar;
        if b % 2 == 0 {
            for tone in pads[ci] {
                add_pad(&mut out, sr, bs, bar * 2.0, tone, 0.045);
            }
        }
        for k in 0..8usize {
            let f = if k == 4 { roots[ci] * 2.0 } else { roots[ci] };
            add_harmonic(&mut out, sr, bs + k as f32 * spb / 2.0, spb * 0.46, f, 0.16, 0.7, 6.0);
        }
        for beat in 0..4usize {
            add_kick(&mut out, sr, bs + beat as f32 * spb);
            if beat % 2 == 1 {
                add_noise_hit(&mut out, &noise, sr, bs + beat as f32 * spb, 0.11, 0.1, 26.0);
            }
            add_noise_hit(&mut out, &noise, sr, bs + (beat as f32 + 0.5) * spb, 0.035, 0.035, 90.0);
        }
    }
    // The bell lead: D minor pentatonic, second half only.
    let lead: [(usize, f32, f32, f32); 12] = [
        (8, 0.0, 587.33, 1.5),
        (8, 2.0, 698.46, 1.0),
        (9, 0.0, 880.0, 2.0),
        (10, 0.0, 783.99, 1.5),
        (10, 3.0, 698.46, 0.5),
        (11, 0.0, 659.25, 2.0),
        (12, 0.0, 587.33, 1.5),
        (12, 2.0, 523.25, 1.0),
        (13, 0.0, 587.33, 3.0),
        (14, 0.0, 698.46, 1.5),
        (14, 2.0, 880.0, 1.0),
        (15, 0.0, 1046.5, 3.5),
    ];
    for (b, beat, f, beats) in lead {
        add_harmonic(&mut out, sr, b as f32 * bar + beat * spb, beats * spb, f, 0.11, 0.25, 2.2);
    }
    normalize(out, 0.5)
}

/// An eight second, seamlessly looping industrial drone.
fn ambient(sr: f32) -> Vec<f32> {
    let dur = 8.0f32;
    let xfade = 0.5f32;
    let loop_len = (sr * dur) as usize;
    let total = loop_len + (sr * xfade) as usize;
    let mut rng = Rng::new(0xA3B1);
    let mut lp = 0.0f32;
    let mut lp2 = 0.0f32;
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let t = i as f32 / sr;
        let noise = rng.range(-1.0, 1.0);
        lp += (noise - lp) * 0.02;
        lp2 += (lp - lp2) * 0.06;
        // Frequencies are whole multiples of 1/8 Hz, so they line up at the seam.
        let hum = (TAU * 55.0 * t).sin() * 0.35
            + (TAU * 82.5 * t).sin() * 0.22
            + (TAU * 110.0 * t).sin() * 0.12;
        let trem = 0.75 + 0.25 * (TAU * 0.125 * t).sin();
        let wind = lp2 * 1.4 * (0.6 + 0.4 * (TAU * 0.0625 * t).sin());
        out.push((hum + wind) * trem * 0.6);
    }
    let xfade_samples = (sr * xfade) as usize;
    for i in 0..xfade_samples.min(loop_len) {
        let f = i as f32 / xfade_samples as f32;
        out[i] = out[i] * f + out[loop_len + i] * (1.0 - f);
    }
    out.truncate(loop_len);
    normalize(out, 0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_is_finite_and_audible() {
        let bank = Bank::generate(44_100.0);
        assert_eq!(bank.sounds.len(), SOUND_COUNT);
        for (i, sound) in bank.sounds.iter().enumerate() {
            assert!(sound.len() > 500, "effect {i} is suspiciously short");
            assert!(
                sound.iter().all(|s| s.is_finite()),
                "effect {i} has NaN/inf"
            );
            let peak = sound.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.3 && peak <= 1.0, "effect {i} peak {peak}");
        }
    }

    #[test]
    fn music_loop_is_a_full_finite_song() {
        let bank = Bank::generate(22_050.0);
        // Sixteen bars at 92 BPM is about 41.7 seconds.
        assert!(bank.music.len() > 22_050 * 40 && bank.music.len() < 22_050 * 44);
        assert!(bank.music.iter().all(|s| s.is_finite()));
        let peak = bank.music.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.3 && peak <= 1.0, "music peak {peak}");
    }

    #[test]
    fn music_plays_only_when_switched_on() {
        let bank = Arc::new(Bank::generate(22_050.0));
        let mut mixer = Mixer::new(bank);
        mixer.set_volumes(1.0, 1.0, 1.0);
        mixer.set_duck(1.0);
        let mut buf = vec![0.0f32; 4096];
        mixer.render(&mut buf, 2);
        assert_eq!(mixer.music_pos, 0, "music advanced while switched off");
        mixer.set_music(true);
        for _ in 0..30 {
            mixer.render(&mut buf, 2);
        }
        assert!(mixer.music_pos > 0, "music never started");
        assert!(buf.iter().any(|s| s.abs() > 0.0), "music produced silence");
    }

    #[test]
    fn ambient_loop_is_long_and_finite() {
        let bank = Bank::generate(22_050.0);
        assert!(bank.ambient.len() > 22_050 * 7);
        assert!(bank.ambient.iter().all(|s| s.is_finite()));
        let first = bank.ambient[0].abs();
        let last = bank.ambient[bank.ambient.len() - 1].abs();
        assert!(
            (first - last) < 0.2,
            "loop seam is discontinuous: {first} vs {last}"
        );
    }

    #[test]
    fn mixer_pans_and_decays() {
        let bank = Arc::new(Bank::generate(44_100.0));
        let mut mixer = Mixer::new(bank);
        mixer.trigger(SoundRequest {
            id: SoundId::Pistol,
            gain: 0.8,
            pan: -1.0,
        });
        let mut buf = vec![0.0f32; 1024];
        mixer.render(&mut buf, 2);
        assert!(buf.iter().any(|s| s.abs() > 0.0), "mixer produced silence");
        let right: f32 = buf.chunks(2).map(|f| f[1].abs()).sum();
        assert!(
            right < 1e-6,
            "hard-left sound leaked into the right channel"
        );
        assert!(!mixer.voices.is_empty(), "voice should still be playing");

        for _ in 0..40 {
            let mut tail = vec![0.0f32; 4096];
            mixer.render(&mut tail, 2);
        }
        assert!(mixer.voices.is_empty(), "finished voices were not released");
    }

    #[test]
    fn volume_controls_silence_the_mix() {
        let bank = Arc::new(Bank::generate(22_050.0));
        let mut mixer = Mixer::new(bank);
        mixer.set_volumes(1.0, 1.0, 1.0);
        mixer.set_duck(1.0);
        mixer.trigger(SoundRequest {
            id: SoundId::Shotgun,
            gain: 1.0,
            pan: 0.0,
        });
        let mut loud = vec![0.0f32; 2048];
        mixer.render(&mut loud, 2);
        assert!(loud.iter().any(|s| s.abs() > 0.001));

        let bank = Arc::new(Bank::generate(22_050.0));
        let mut silent = Mixer::new(bank);
        silent.set_volumes(0.0, 1.0, 1.0);
        silent.set_duck(1.0);
        silent.trigger(SoundRequest {
            id: SoundId::Shotgun,
            gain: 1.0,
            pan: 0.0,
        });
        let mut buf = vec![0.0f32; 2048];
        silent.render(&mut buf, 2);
        assert!(
            buf.iter().all(|s| s.abs() < 1e-9),
            "master 0 was not silent"
        );
    }

    #[test]
    fn ambient_only_plays_when_unducked() {
        let bank = Arc::new(Bank::generate(22_050.0));
        let mut mixer = Mixer::new(bank);
        mixer.set_volumes(1.0, 1.0, 1.0);
        let mut buf = vec![0.0f32; 2048];
        mixer.render(&mut buf, 2);
        assert!(
            buf.iter().all(|s| s.abs() < 1e-9),
            "ambient audible in menus"
        );
        mixer.set_duck(1.0);
        mixer.render(&mut buf, 2);
        assert!(buf.iter().any(|s| s.abs() > 0.0), "ambient silent in game");
    }

    #[test]
    fn voice_limit_is_enforced() {
        let bank = Arc::new(Bank::generate(22_050.0));
        let mut mixer = Mixer::new(bank);
        for _ in 0..MAX_VOICES + 10 {
            mixer.trigger(SoundRequest {
                id: SoundId::Shotgun,
                gain: 1.0,
                pan: 0.0,
            });
        }
        assert!(mixer.voices.len() <= MAX_VOICES);
    }
}

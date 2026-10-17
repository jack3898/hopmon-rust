//! Standard MIDI file renderer.
//!
//! The original game handed `Music0X.mid` to the Windows MCI sequencer, i.e.
//! the system's General MIDI synthesizer. To stay portable this module turns
//! a song into a looping PCM buffer, either with a user supplied SoundFont
//! (via `rustysynth`, best quality) or with a small built-in General MIDI
//! approximation that needs no external files.

use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

pub const SAMPLE_RATE: u32 = 44100;

pub struct Renderer {
    soundfont: Option<Arc<rustysynth::SoundFont>>,
}

/// SoundFont lookup: explicit path, `HOPMON_SOUNDFONT`, then common system
/// locations.
pub fn find_soundfont(explicit: Option<&Path>) -> Option<PathBuf> {
    let mut c: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        c.push(p.to_path_buf());
    }
    if let Ok(p) = std::env::var("HOPMON_SOUNDFONT") {
        c.push(p.into());
    }
    for p in [
        "/usr/share/sounds/sf2/FluidR3_GM.sf2",
        "/usr/share/sounds/sf2/default-GM.sf2",
        "/usr/share/sounds/sf2/TimGM6mb.sf2",
        "/usr/share/soundfonts/FluidR3_GM.sf2",
        "/usr/share/soundfonts/default.sf2",
        "/usr/local/share/soundfonts/default.sf2",
        "/opt/homebrew/share/soundfonts/default.sf2",
    ] {
        c.push(p.into());
    }
    c.into_iter().find(|p| p.is_file())
}

impl Renderer {
    pub fn new(soundfont: Option<&Path>) -> Renderer {
        let sf = soundfont.and_then(|p| {
            let mut f = std::fs::File::open(p).ok()?;
            match rustysynth::SoundFont::new(&mut f) {
                Ok(sf) => Some(Arc::new(sf)),
                Err(e) => {
                    eprintln!("soundfont {}: {e:?}", p.display());
                    None
                }
            }
        });
        Renderer { soundfont: sf }
    }

    /// Renders a whole song to a 16-bit stereo WAV file image.
    pub fn render_wav(&self, midi: &[u8]) -> Result<Vec<u8>, String> {
        let (l, r) = match &self.soundfont {
            Some(sf) => render_soundfont(sf, midi)?,
            None => render_builtin(midi)?,
        };
        Ok(encode_wav(&l, &r))
    }
}

fn render_soundfont(
    sf: &Arc<rustysynth::SoundFont>,
    midi: &[u8],
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let mut cur = std::io::Cursor::new(midi);
    let mf = Arc::new(rustysynth::MidiFile::new(&mut cur).map_err(|e| format!("{e:?}"))?);
    let settings = rustysynth::SynthesizerSettings::new(SAMPLE_RATE as i32);
    let synth = rustysynth::Synthesizer::new(sf, &settings).map_err(|e| format!("{e:?}"))?;
    let mut seq = rustysynth::MidiFileSequencer::new(synth);
    seq.play(&mf, false);
    let n = (mf.get_length() * SAMPLE_RATE as f64) as usize;
    let mut l = vec![0.0; n];
    let mut r = vec![0.0; n];
    seq.render(&mut l, &mut r);
    normalize(&mut l, &mut r, 0.9, 4.0);
    Ok((l, r))
}

// ---------------------------------------------------------------------------
// Built-in synthesizer

#[derive(Clone, Copy)]
enum Ev {
    On(u8, u8, u8),
    Off(u8, u8),
    Program(u8, u8),
    Control(u8, u8, u8),
    Bend(u8, i16),
}

/// Flattens all tracks into `(seconds, event)` using the tempo map.
fn timeline(data: &[u8]) -> Result<(Vec<(f64, Ev)>, f64), String> {
    let smf = Smf::parse(data).map_err(|e| e.to_string())?;
    let tpq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as f64,
        Timing::Timecode(fps, sub) => {
            // Treat as fixed ticks per second.
            return timeline_fixed(&smf, fps.as_f32() as f64 * sub as f64);
        }
    };
    let mut raw: Vec<(u64, usize, Option<Ev>, Option<u32>)> = Vec::new();
    let mut end_tick = 0u64;
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut t = 0u64;
        for e in track {
            t += e.delta.as_int() as u64;
            end_tick = end_tick.max(t);
            match e.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(us)) => {
                    raw.push((t, ti, None, Some(us.as_int())))
                }
                TrackEventKind::Midi { channel, message } => {
                    let c = channel.as_int();
                    let ev = match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            Ev::On(c, key.as_int(), vel.as_int())
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            Ev::Off(c, key.as_int())
                        }
                        MidiMessage::ProgramChange { program } => Ev::Program(c, program.as_int()),
                        MidiMessage::Controller { controller, value } => {
                            Ev::Control(c, controller.as_int(), value.as_int())
                        }
                        MidiMessage::PitchBend { bend } => Ev::Bend(c, bend.as_int()),
                        _ => continue,
                    };
                    raw.push((t, ti, Some(ev), None));
                }
                _ => {}
            }
        }
    }
    // Stable sort by tick keeps per-track order; note-offs before note-ons at
    // the same tick avoid cutting re-struck notes.
    raw.sort_by_key(|(t, ti, ev, tempo)| {
        let pri = match (ev, tempo) {
            (_, Some(_)) => 0,
            (Some(Ev::Off(..)), _) => 1,
            _ => 2,
        };
        (*t, pri, *ti)
    });
    let mut out = Vec::with_capacity(raw.len());
    let mut us_per_q = 500_000.0f64;
    let mut last_tick = 0u64;
    let mut secs = 0.0f64;
    for (t, _, ev, tempo) in raw {
        secs += (t - last_tick) as f64 / tpq * us_per_q / 1e6;
        last_tick = t;
        if let Some(tp) = tempo {
            us_per_q = tp as f64;
        }
        if let Some(ev) = ev {
            out.push((secs, ev));
        }
    }
    secs += (end_tick - last_tick) as f64 / tpq * us_per_q / 1e6;
    Ok((out, secs))
}

fn timeline_fixed(smf: &Smf, ticks_per_sec: f64) -> Result<(Vec<(f64, Ev)>, f64), String> {
    let mut out = Vec::new();
    let mut end = 0.0f64;
    for track in &smf.tracks {
        let mut t = 0u64;
        for e in track {
            t += e.delta.as_int() as u64;
            let s = t as f64 / ticks_per_sec;
            end = end.max(s);
            if let TrackEventKind::Midi { channel, message } = e.kind {
                let c = channel.as_int();
                let ev = match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        Ev::On(c, key.as_int(), vel.as_int())
                    }
                    MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                        Ev::Off(c, key.as_int())
                    }
                    MidiMessage::ProgramChange { program } => Ev::Program(c, program.as_int()),
                    MidiMessage::Controller { controller, value } => {
                        Ev::Control(c, controller.as_int(), value.as_int())
                    }
                    MidiMessage::PitchBend { bend } => Ev::Bend(c, bend.as_int()),
                    _ => continue,
                };
                out.push((s, ev));
            }
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    Ok((out, end))
}

#[derive(Clone, Copy, PartialEq)]
enum Wave {
    Sine,
    Triangle,
    Saw,
    Square,
    Organ,
    Piano,
    Bell,
}

#[derive(Clone, Copy)]
struct Patch {
    wave: Wave,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    /// One-pole low-pass cutoff (Hz).
    cutoff: f32,
    gain: f32,
    detune: f32,
}

fn patch(program: u8) -> Patch {
    let p = |wave, attack, decay, sustain, release, cutoff, gain| Patch {
        wave,
        attack,
        decay,
        sustain,
        release,
        cutoff,
        gain,
        detune: 0.0,
    };
    match program {
        0..=7 => p(Wave::Piano, 0.003, 1.6, 0.0, 0.25, 6000.0, 0.9),
        8..=15 => p(Wave::Bell, 0.002, 0.9, 0.0, 0.4, 9000.0, 0.7),
        16..=23 => p(Wave::Organ, 0.01, 0.1, 0.85, 0.08, 5000.0, 0.55),
        24..=31 => p(Wave::Saw, 0.003, 0.9, 0.15, 0.2, 2500.0, 0.6),
        32..=39 => p(Wave::Triangle, 0.004, 0.6, 0.55, 0.1, 1800.0, 1.1),
        40..=47 => Patch {
            detune: 0.004,
            ..p(Wave::Saw, 0.06, 0.3, 0.8, 0.3, 3000.0, 0.45)
        },
        48..=55 => Patch {
            detune: 0.006,
            ..p(Wave::Saw, 0.12, 0.3, 0.85, 0.5, 2600.0, 0.4)
        },
        56..=63 => p(Wave::Saw, 0.03, 0.2, 0.75, 0.15, 3500.0, 0.5),
        64..=71 => p(Wave::Square, 0.03, 0.2, 0.8, 0.12, 2400.0, 0.4),
        72..=79 => p(Wave::Sine, 0.04, 0.2, 0.85, 0.15, 8000.0, 0.8),
        80..=87 => p(Wave::Square, 0.005, 0.25, 0.7, 0.12, 4000.0, 0.4),
        88..=103 => Patch {
            detune: 0.007,
            ..p(Wave::Saw, 0.25, 0.5, 0.8, 0.6, 2000.0, 0.4)
        },
        104..=111 => p(Wave::Saw, 0.004, 0.7, 0.2, 0.2, 3000.0, 0.55),
        112..=119 => p(Wave::Bell, 0.001, 0.4, 0.0, 0.2, 8000.0, 0.7),
        _ => p(Wave::Sine, 0.01, 0.3, 0.5, 0.2, 6000.0, 0.5),
    }
}

#[derive(Clone, Copy)]
struct Channel {
    program: u8,
    volume: f32,
    expression: f32,
    pan: f32,
    bend: f32,
    bend_range: f32,
    sustain: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            program: 0,
            volume: 100.0 / 127.0,
            expression: 1.0,
            pan: 0.5,
            bend: 0.0,
            bend_range: 2.0,
            sustain: false,
        }
    }
}

struct Voice {
    ch: u8,
    key: u8,
    vel: f32,
    patch: Patch,
    drum: Option<u8>,
    phase: f32,
    phase2: f32,
    age: f32,
    released: Option<f32>,
    release_level: f32,
    held_by_pedal: bool,
    level: f32,
    decay_pos: f32,
    decay_coef: f32,
    rel_coef: f32,
    bend_seen: f32,
    inc: f32,
    lp_coef: f32,
    lp: f32,
    noise: u32,
    done: bool,
}

/// Sine lookup with linear interpolation; `ph` in [0, 1).
fn fsin(ph: f32) -> f32 {
    use std::sync::OnceLock;
    const N: usize = 4096;
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    let t = TABLE.get_or_init(|| (0..=N).map(|i| (i as f32 / N as f32 * TAU).sin()).collect());
    let x = ph.fract() * N as f32;
    let i = x as usize;
    let f = x - i as f32;
    t[i] + (t[i + 1] - t[i]) * f
}

fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

impl Voice {
    /// Advances the ADSR envelope by one sample (multiplicative decays
    /// instead of per-sample `exp`).
    fn env_step(&mut self, dt: f32) -> f32 {
        let p = &self.patch;
        if self.released.is_some() {
            self.release_level *= self.rel_coef;
            return self.release_level;
        }
        if self.age < p.attack {
            self.level = self.age / p.attack.max(1e-4);
        } else {
            if self.decay_coef == 0.0 {
                self.decay_coef = (-3.0 * dt / p.decay.max(1e-3)).exp();
                self.rel_coef = (-4.0 * dt / p.release.max(1e-3)).exp();
                self.decay_pos = 1.0;
            }
            self.level = p.sustain + (1.0 - p.sustain) * self.decay_pos;
            self.decay_pos *= self.decay_coef;
        }
        self.level
    }

    fn noise(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn drum_sample(&mut self, key: u8, dt: f32) -> f32 {
        let t = self.age;
        let (s, len) = match key {
            35 | 36 => {
                let f = 50.0 + 110.0 * (-t * 30.0).exp();
                self.phase = (self.phase + f * dt).fract();
                (fsin(self.phase) * (-t * 9.0).exp() * 1.4, 0.45)
            }
            38 | 40 => {
                self.phase = (self.phase + 190.0 * dt).fract();
                let n = self.noise();
                (
                    fsin(self.phase) * 0.5 * (-t * 25.0).exp() + n * 0.7 * (-t * 14.0).exp(),
                    0.3,
                )
            }
            37 | 39 => (self.noise() * (-t * 30.0).exp(), 0.15),
            41 | 43 | 45 | 47 | 48 | 50 => {
                let base = 80.0 + (key as f32 - 41.0) * 18.0;
                let f = base * (1.0 + 0.6 * (-t * 20.0).exp());
                self.phase = (self.phase + f * dt).fract();
                (fsin(self.phase) * (-t * 7.0).exp(), 0.5)
            }
            42 | 44 => {
                let n = self.noise();
                let hp = n - self.lp;
                self.lp += 0.5 * (n - self.lp);
                (hp * (-t * 60.0).exp() * 0.6, 0.08)
            }
            46 => {
                let n = self.noise();
                let hp = n - self.lp;
                self.lp += 0.5 * (n - self.lp);
                (hp * (-t * 8.0).exp() * 0.5, 0.5)
            }
            49 | 52 | 55 | 57 => {
                let n = self.noise();
                let hp = n - self.lp;
                self.lp += 0.3 * (n - self.lp);
                (hp * (-t * 2.5).exp() * 0.5, 1.6)
            }
            51 | 53 | 59 => {
                self.phase = (self.phase + 3200.0 * dt).fract();
                let n = self.noise();
                ((fsin(self.phase) * 0.3 + n * 0.25) * (-t * 4.0).exp(), 1.0)
            }
            _ => (self.noise() * (-t * 20.0).exp() * 0.6, 0.25),
        };
        if t > len {
            self.done = true;
        }
        s
    }

    fn sample(&mut self, chan: &Channel, dt: f32) -> f32 {
        if let Some(k) = self.drum {
            let s = self.drum_sample(k, dt) * self.vel;
            self.age += dt;
            return s;
        }
        if self.inc == 0.0 || self.bend_seen != chan.bend {
            let note = self.key as f32 + chan.bend * chan.bend_range;
            let freq = 440.0 * 2f32.powf((note - 69.0) / 12.0);
            self.inc = (freq * dt).min(0.45);
            self.bend_seen = chan.bend;
        }
        let inc = self.inc;
        let p = self.patch;
        let osc = |ph: f32| -> f32 {
            match p.wave {
                Wave::Sine => fsin(ph),
                Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
                Wave::Saw => 2.0 * ph - 1.0 - poly_blep(ph, inc),
                Wave::Square => {
                    let a = if ph < 0.5 { 1.0 } else { -1.0 };
                    a + poly_blep(ph, inc) - poly_blep((ph + 0.5).fract(), inc)
                }
                Wave::Organ => fsin(ph) * 0.6 + fsin(ph * 2.0) * 0.3 + fsin(ph * 4.0) * 0.15,
                Wave::Piano => fsin(ph) * 0.7 + fsin(ph * 2.0) * 0.25 + fsin(ph * 3.0) * 0.1,
                Wave::Bell => fsin(ph) * 0.7 + fsin(ph * 3.5) * 0.3,
            }
        };
        let mut s = osc(self.phase);
        if p.detune > 0.0 {
            s = 0.6 * s + 0.6 * osc(self.phase2);
            self.phase2 = (self.phase2 + inc * (1.0 + p.detune)).fract();
        }
        self.phase = (self.phase + inc).fract();
        if self.lp_coef == 0.0 {
            self.lp_coef = 1.0 - (-TAU * p.cutoff * dt).exp();
        }
        self.lp += self.lp_coef * (s - self.lp);
        let e = self.env_step(dt);
        self.age += dt;
        if self.released.is_some() && e < 1e-4 {
            self.done = true;
        }
        if self.released.is_none() && p.sustain == 0.0 && self.age > p.attack + p.decay * 3.0 {
            self.done = true;
        }
        self.lp * e * self.vel * p.gain
    }

    fn release(&mut self) {
        if self.released.is_none() {
            self.release_level = self.level;
            if self.rel_coef == 0.0 {
                self.rel_coef = (-4.0 / (self.patch.release.max(1e-3) * SAMPLE_RATE as f32)).exp();
            }
            self.released = Some(self.age);
        }
    }
}

fn render_builtin(midi: &[u8]) -> Result<(Vec<f32>, Vec<f32>), String> {
    let (events, length) = timeline(midi)?;
    let n = (length * SAMPLE_RATE as f64).ceil() as usize + 1;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut chans = [Channel::default(); 16];
    let pan_of = |p: f32| {
        let a = p * std::f32::consts::FRAC_PI_2;
        (a.cos(), a.sin())
    };
    let mut pan_gains = [pan_of(0.5); 16];
    let mut voices: Vec<Voice> = Vec::new();
    let dt = 1.0 / SAMPLE_RATE as f32;
    let mut ei = 0;
    let mut seed = 0x1234_5678u32;
    for i in 0..n {
        let now = i as f64 / SAMPLE_RATE as f64;
        while ei < events.len() && events[ei].0 <= now {
            match events[ei].1 {
                Ev::On(c, k, v) => {
                    // Retrigger: release a voice already playing this key.
                    for vo in voices.iter_mut().filter(|vo| vo.ch == c && vo.key == k) {
                        vo.release();
                    }
                    if voices.len() >= 48 {
                        // Steal the oldest released (or oldest) voice.
                        let idx = voices
                            .iter()
                            .enumerate()
                            .max_by(|a, b| {
                                let ka = (a.1.released.is_some(), a.1.age);
                                let kb = (b.1.released.is_some(), b.1.age);
                                ka.partial_cmp(&kb).unwrap()
                            })
                            .map(|x| x.0)
                            .unwrap();
                        voices.swap_remove(idx);
                    }
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    voices.push(Voice {
                        ch: c,
                        key: k,
                        vel: (v as f32 / 127.0).powf(1.5),
                        patch: patch(chans[c as usize].program),
                        drum: (c == 9).then_some(k),
                        phase: 0.0,
                        phase2: 0.25,
                        age: 0.0,
                        released: None,
                        release_level: 0.0,
                        held_by_pedal: false,
                        level: 0.0,
                        decay_pos: 1.0,
                        decay_coef: 0.0,
                        rel_coef: 0.0,
                        bend_seen: 0.0,
                        inc: 0.0,
                        lp_coef: 0.0,
                        lp: 0.0,
                        noise: seed | 1,
                        done: false,
                    });
                }
                Ev::Off(c, k) => {
                    let sus = chans[c as usize].sustain;
                    for vo in voices
                        .iter_mut()
                        .filter(|vo| vo.ch == c && vo.key == k && vo.released.is_none())
                    {
                        if vo.drum.is_some() {
                            continue;
                        }
                        if sus {
                            vo.held_by_pedal = true;
                        } else {
                            vo.release();
                        }
                    }
                }
                Ev::Program(c, p) => chans[c as usize].program = p,
                Ev::Control(c, cc, v) => {
                    let ch = &mut chans[c as usize];
                    match cc {
                        7 => ch.volume = v as f32 / 127.0,
                        10 => {
                            ch.pan = v as f32 / 127.0;
                            pan_gains[c as usize] = pan_of(ch.pan);
                        }
                        11 => ch.expression = v as f32 / 127.0,
                        64 => {
                            ch.sustain = v >= 64;
                            if !ch.sustain {
                                for vo in voices
                                    .iter_mut()
                                    .filter(|vo| vo.ch == c && vo.held_by_pedal)
                                {
                                    vo.held_by_pedal = false;
                                    vo.release();
                                }
                            }
                        }
                        121 => {
                            *ch = Channel {
                                program: ch.program,
                                ..Channel::default()
                            };
                            pan_gains[c as usize] = pan_of(ch.pan);
                        }
                        120 | 123 => {
                            for vo in voices.iter_mut().filter(|vo| vo.ch == c) {
                                vo.release();
                            }
                        }
                        _ => {}
                    }
                }
                Ev::Bend(c, b) => chans[c as usize].bend = b as f32 / 8192.0,
            }
            ei += 1;
        }
        let (mut sl, mut sr) = (0.0f32, 0.0f32);
        let mut any_done = false;
        for vo in voices.iter_mut() {
            let ch = &chans[vo.ch as usize];
            let s = vo.sample(ch, dt) * ch.volume * ch.expression;
            let (pl, pr) = pan_gains[vo.ch as usize];
            sl += s * pl;
            sr += s * pr;
            any_done |= vo.done;
        }
        if any_done {
            voices.retain(|v| !v.done);
        }
        l[i] = sl;
        r[i] = sr;
    }
    normalize(&mut l, &mut r, 0.85, 3.0);
    Ok((l, r))
}

fn normalize(l: &mut [f32], r: &mut [f32], target: f32, max_gain: f32) {
    let peak = l
        .iter()
        .chain(r.iter())
        .fold(0.0f32, |m, &s| m.max(s.abs()));
    if peak > 1e-6 {
        let g = (target / peak).min(max_gain);
        for s in l.iter_mut().chain(r.iter_mut()) {
            *s *= g;
        }
    }
}

fn encode_wav(l: &[f32], r: &[f32]) -> Vec<u8> {
    let n = l.len().min(r.len());
    let data_len = (n * 4) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    w.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..n {
        for s in [l[i], r[i]] {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            w.extend_from_slice(&v.to_le_bytes());
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_well_formed() {
        let w = encode_wav(&[0.0, 0.5], &[0.0, -0.5]);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(w.len(), 44 + 8);
    }
}

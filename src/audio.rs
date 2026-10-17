//! `JSoundList` / `JMidiList` replacements.
//!
//! Sound effects keep the original semantics: every effect owns `count`
//! DirectSound buffers used round-robin, and playing restarts the next
//! buffer from the beginning. Volume 0..=100 maps to DirectSound attenuation
//! `(v/100 * 10000) - 10000` hundredths of a dB.
//!
//! Music: the original streamed `Music0X.mid` through MCI. Here each song is
//! rendered to PCM by [`crate::midi`] on a worker thread and then looped.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound, stop_sound};

use crate::assets::Assets;

struct Effect {
    buffers: Vec<Sound>,
    next: usize,
}

pub struct Audio {
    effects: HashMap<u16, Effect>,
    music: Vec<Option<Sound>>,
    pending: Option<Receiver<(usize, Vec<u8>)>>,
    current_music: Option<usize>,
    wanted_music: Option<usize>,
    pub sfx_volume: f32,
    pub music_volume: f32,
}

/// Effects created in `XApplication::Create` with their buffer counts.
pub const EFFECTS: [(u16, usize); 18] = [
    (0xf3, 4),
    (0xf4, 4),
    (0xf5, 4),
    (0x13e, 4),
    (0x13f, 2),
    (0xeb, 1),
    (0xea, 1),
    (0xff, 1),
    (0xee, 1),
    (0xef, 1),
    (0xf0, 1),
    (0xf1, 1),
    (0xe8, 1),
    (0xe9, 1),
    (0xfe, 1),
    (0xec, 1),
    (0xed, 1),
    (0x13d, 1),
];

pub mod sfx {
    pub const CANNON_FIRE: u16 = 0xf3;
    pub const CANNON_UP_FIRE: u16 = 0xf4;
    pub const NEEDLE_BOX: u16 = 0xf5;
    pub const STEAM: u16 = 0x13e;
    pub const UFO_FIRE: u16 = 0x13f;
    pub const ENEMY_EXPLODE: u16 = 0xeb;
    pub const PAC_FIRE_HIT: u16 = 0xea;
    pub const PAC_JUMP: u16 = 0xff;
    pub const PAC_FIRE: u16 = 0xee;
    pub const PAC_DIE: u16 = 0xef;
    pub const LEVEL_COMPLETE: u16 = 0xf0;
    pub const PAC_VANISH: u16 = 0xf1;
    pub const CRYSTAL_TAKEN: u16 = 0xe8;
    pub const CRYSTAL_VANISH: u16 = 0xe9;
    pub const FIRE_READY: u16 = 0xfe;
    pub const MENU_MOVE: u16 = 0xec;
    pub const MENU_SELECT: u16 = 0xed;
    pub const LOGO: u16 = 0x13d;
}

/// DirectSound attenuation to linear gain.
pub fn volume_to_gain(v: u8) -> f32 {
    let v = v.min(100) as f32;
    let millibel = v / 100.0 * 10000.0 - 10000.0;
    10f32.powf(millibel / 2000.0)
}

impl Audio {
    pub async fn new(
        assets: &Assets,
        enabled: bool,
        soundfont: Option<std::path::PathBuf>,
    ) -> Audio {
        let mut effects = HashMap::new();
        if enabled {
            for (id, count) in EFFECTS {
                let Some(bytes) = assets.wave(id) else {
                    continue;
                };
                let mut buffers = Vec::new();
                for _ in 0..count {
                    match load_sound_from_bytes(bytes).await {
                        Ok(s) => buffers.push(s),
                        Err(e) => eprintln!("sound {id:#x}: {e}"),
                    }
                }
                if !buffers.is_empty() {
                    effects.insert(id, Effect { buffers, next: 0 });
                }
            }
        }
        let mut pending = None;
        if enabled {
            let songs: Vec<Option<Vec<u8>>> = (0..4).map(|i| assets.midi(i)).collect();
            let (tx, rx) = channel();
            // One worker per song; the title theme is the shortest and
            // finishes first.
            for (i, song) in songs.into_iter().enumerate() {
                let Some(data) = song else { continue };
                let tx = tx.clone();
                let sf = soundfont.clone();
                std::thread::spawn(move || {
                    let synth = crate::midi::Renderer::new(sf.as_deref());
                    match synth.render_wav(&data) {
                        Ok(wav) => {
                            let _ = tx.send((i, wav));
                        }
                        Err(e) => eprintln!("Music0{}.mid: {e}", i + 1),
                    }
                });
            }
            drop(tx);
            pending = Some(rx);
        }
        Audio {
            effects,
            music: vec![None, None, None, None],
            pending,
            current_music: None,
            wanted_music: None,
            sfx_volume: 1.0,
            music_volume: 0.8,
        }
    }

    /// `JSoundList::Play(id, volume, loop=0)`.
    pub fn play(&mut self, id: u16, volume: u8) {
        if volume == 0 {
            return;
        }
        let gain = volume_to_gain(volume) * self.sfx_volume;
        if let Some(e) = self.effects.get_mut(&id) {
            let s = &e.buffers[e.next];
            stop_sound(s);
            play_sound(
                s,
                PlaySoundParams {
                    looped: false,
                    volume: gain,
                },
            );
            e.next = (e.next + 1) % e.buffers.len();
        }
    }

    /// Polls the music worker; call once per frame.
    pub async fn update(&mut self) {
        if let Some(rx) = &self.pending {
            let mut done = false;
            loop {
                match rx.try_recv() {
                    Ok((i, wav)) => match load_sound_from_bytes(&wav).await {
                        Ok(s) => self.music[i] = Some(s),
                        Err(e) => eprintln!("music {i}: {e}"),
                    },
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(_) => {
                        done = true;
                        break;
                    }
                }
            }
            if done {
                self.pending = None;
            }
        }
        if let Some(i) = self.wanted_music
            && self.wanted_music != self.current_music
            && let Some(s) = &self.music[i]
        {
            play_sound(
                s,
                PlaySoundParams {
                    looped: true,
                    volume: self.music_volume,
                },
            );
            self.current_music = Some(i);
        }
    }

    /// `JMidiList::Play(index)`: switching songs only when it differs.
    pub fn play_music(&mut self, index: usize) {
        if self.wanted_music == Some(index) {
            return;
        }
        self.stop_music();
        self.wanted_music = Some(index);
    }

    /// `JMidiList::Stop`.
    pub fn stop_music(&mut self) {
        if let Some(i) = self.current_music.take()
            && let Some(s) = &self.music[i]
        {
            stop_sound(s);
        }
        self.wanted_music = None;
    }
}

//! Ports of the `X*` game classes from Hopmon.exe.

pub mod camera;
pub mod crystal;
pub mod enemy;
pub mod fire;
pub mod map;
pub mod mapobj;
pub mod pac;
pub mod particle;
pub mod scenes;
pub mod status;
pub mod world;

use crate::assets::Assets;
use crate::audio::Audio;
use crate::gfx::{Gfx, SCREEN_H, SCREEN_W};
use crate::input::Input;
use crate::sprite::Sprite;

/// `XApplication::STATE` scene requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppState {
    Exit,
    InProcess,
    StartLogo,
    StartTitle,
    StartSelLevel,
    StartGame,
    StartEnding,
}

/// Persistent application data owned by `XApplication`.
pub struct AppData {
    pub current_level: i32,
    pub max_sel_level: i32,
    pub max_level: i32,
    pub state: AppState,
    pub save: crate::save::Save,
}

impl AppData {
    /// `GetCurrentWorld`: 15 levels per world.
    pub fn world(&self) -> i32 {
        self.current_level / 15
    }

    pub fn set_state(&mut self, s: AppState) {
        self.state = s;
    }

    /// `SaveWinRegData`.
    pub fn save(&mut self) {
        self.save.max_level = self.max_sel_level;
        self.save.write();
    }
}

/// MSVC `rand()` (same LCG and 15-bit output) used by `Random(n)`.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed)
    }

    pub fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        ((self.0 >> 16) & 0x7fff) as i32
    }

    /// `Random(n)`: `rand() % n`, 0 for n < 1.
    pub fn random(&mut self, n: i32) -> i32 {
        if n < 1 { 0 } else { self.rand() % n }
    }
}

pub struct Ctx<'a> {
    pub assets: &'a Assets,
    pub gfx: &'a mut Gfx,
    pub audio: &'a mut Audio,
    pub input: &'a mut Input,
    pub rng: &'a mut Rng,
    pub app: &'a mut AppData,
}

/// `GetMapResId`: resource id of the map for the current level.
pub fn map_res_id(app: &AppData) -> u16 {
    const W0: [u16; 15] = [
        0x11c, 0x115, 0x116, 0x117, 0x118, 0x119, 0x11a, 0x11b, 0x114, 0x109, 0x10a, 0x10b, 0x10c,
        0x10d, 0x10e,
    ];
    let i = (app.current_level % 15) as usize;
    match app.world() {
        0 => W0[i],
        1 => 0x11d + i as u16,
        _ => 300 + i as u16,
    }
}

/// `XProcessObject` state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PState {
    Non,
    FadeIn,
    FadeOut,
    Process,
}

/// Shared scene base: the sliding curtain fade and its timers.
pub struct Process {
    pub state: PState,
    pub tick: u32,
    pub fade_left: Sprite,
    pub fade_right: Sprite,
    pub process_type: i32,
}

impl Process {
    pub fn new(ctx: &mut Ctx) -> Process {
        Process {
            state: PState::Non,
            tick: 0,
            fade_left: Sprite::new(ctx.gfx, ctx.assets, 0xc3, 1, false),
            fade_right: Sprite::new(ctx.gfx, ctx.assets, 0xc2, 1, false),
            process_type: 0,
        }
    }

    fn reset_fade_in(&mut self) {
        let half = SCREEN_W / 2;
        self.fade_left.draw_w = half;
        self.fade_left.draw_h = SCREEN_H;
        self.fade_left.set_pos(0, 0);
        self.fade_right.draw_w = half;
        self.fade_right.draw_h = SCREEN_H;
        self.fade_right.set_pos(half, 0);
    }

    fn reset_fade_out(&mut self) {
        let half = SCREEN_W / 2;
        self.fade_left.draw_w = half;
        self.fade_left.draw_h = SCREEN_H;
        self.fade_left.set_pos(-half, 0);
        self.fade_right.draw_w = half;
        self.fade_right.draw_h = SCREEN_H;
        self.fade_right.set_pos(SCREEN_W, 0);
    }

    pub fn start_fade_in(&mut self, t: i32) {
        self.reset_fade_in();
        self.process_type = t;
        self.state = PState::FadeIn;
        self.tick = 0;
    }

    pub fn start_fade_out(&mut self, t: i32) {
        self.reset_fade_out();
        self.process_type = t;
        self.state = PState::FadeOut;
        self.tick = 0;
    }

    pub fn start_process(&mut self, t: i32) {
        self.process_type = t;
        self.state = PState::Process;
        self.tick = 0;
    }

    pub fn process_fade_in(&mut self) {
        self.reset_fade_in();
        let t = (self.tick as i32 - 20).clamp(0, 100);
        let f = t as f32 / 100.0 * (SCREEN_W / 2) as f32;
        self.fade_left.x -= f;
        self.fade_right.x += f;
    }

    pub fn process_fade_out(&mut self) {
        self.reset_fade_out();
        let t = (self.tick as i32).min(100);
        let f = t as f32 / 100.0 * (SCREEN_W / 2) as f32;
        self.fade_left.x += f;
        self.fade_right.x -= f;
    }

    pub fn draw_fade(&self, gfx: &mut Gfx, assets: &Assets) {
        if matches!(self.state, PState::FadeIn | PState::FadeOut) {
            self.fade_left.draw(gfx, assets);
            self.fade_right.draw(gfx, assets);
        }
    }
}

/// Every scene (`XProcessObject` subclass) implements this.
pub trait Scene {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32);
    fn render(&mut self, ctx: &mut Ctx);
}

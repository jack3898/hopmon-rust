//! `XPac`: Hopmon himself.

use super::Ctx;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{Axis, HALF_PI, PI, Vec3, dir_vector, v3};
use crate::obj3d::{Obj3D, ObjX};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacState {
    Non,
    Wait,
    Move,
    OnArrow,
    LevelCompleted,
    Dying,
}

pub struct Pac {
    pub obj: ObjX,
    pub state: PacState,
    pub local_time: u32,
    pub mx: i32,
    pub mz: i32,
    pub face: i32,
    pub move_dir: i32,
    pub ref_pos: Vec3,
    pub shadow: Obj3D,
    pub move_frames: u32,
    pub fire_energy: i32,
}

impl Pac {
    pub fn new(ctx: &mut Ctx) -> Pac {
        let mut obj = ObjX::new(ctx.assets, Some(0x94));
        obj.append_frame(ctx.assets, 0x95);
        obj.append_frame(ctx.assets, 0x96);
        obj.append_frame(ctx.assets, 199);
        obj.rotate_by(Axis::X, -HALF_PI);
        obj.scale_by(45.0);
        obj.base.radius = 13.0;
        let mut shadow = Obj3D::shadow(40.0, 40.0);
        shadow.set_rect_texture(0xa3, 0.0, 0.0, 1.0, 1.0);
        Pac {
            obj,
            state: PacState::Non,
            local_time: 0,
            mx: -1,
            mz: -1,
            face: 0,
            move_dir: 0,
            ref_pos: Vec3::ZERO,
            shadow,
            move_frames: 0,
            fire_energy: 0,
        }
    }

    pub fn pos(&self) -> Vec3 {
        self.obj.base.pos
    }

    /// `XPac::Reset`.
    pub fn reset(&mut self) {
        self.obj.on_ground();
        self.obj.base.pos.y += 3.0;
        self.obj.rotate_to(Axis::Y, 0.0);
        self.face = 0;
        self.move_dir = 0;
        self.obj.frame = 0;
        self.state = PacState::Wait;
        self.local_time = 0;
    }

    pub fn set_face(&mut self, d: i32) {
        self.face = d;
        let a = match d {
            0 => 0.0,
            1 => HALF_PI,
            2 => PI,
            3 => 4.712_389,
            _ => return,
        };
        self.obj.rotate_to(Axis::Y, a);
    }

    pub fn render(&mut self, gfx: &mut Gfx) {
        if self.state != PacState::Non {
            self.obj.render(gfx);
            self.shadow.base.pos = v3(self.obj.base.pos.x, 1.0, self.obj.base.pos.z);
            self.shadow.render(gfx);
        }
    }

    pub fn process_rotate(&mut self, ticks: u32) {
        match self.state {
            PacState::LevelCompleted => self.obj.rotate_by(Axis::Y, ticks as f32 * 0.069_813_2),
            PacState::Dying => {
                let f = ticks as f32 * 0.139_626_3;
                match self.face {
                    0 => self.obj.rotate_by(Axis::X, -f),
                    2 => self.obj.rotate_by(Axis::X, f),
                    1 => self.obj.rotate_by(Axis::Z, f),
                    3 => self.obj.rotate_by(Axis::Z, -f),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl World {
    /// `XPac::StartMoving`.
    pub fn pac_start_moving(&mut self, ctx: &mut Ctx, dir: i32) {
        if dir < 0 || self.pac.state != PacState::Wait {
            return;
        }
        self.pac.set_face(dir);
        self.pac.move_dir = dir;
        let (x, z) = super::map::step(self.pac.mx, self.pac.mz, dir);
        if !self.map.is_blocked_pac(x, z) {
            ctx.audio.play(sfx::PAC_JUMP, 100);
            self.pac.ref_pos = self.pac.obj.base.pos;
            self.pac.mx = x;
            self.pac.mz = z;
            self.pac.move_frames = (self.taken_count * 3 + 40) as u32;
            self.pac.state = PacState::Move;
            self.pac.local_time = 0;
        }
    }

    /// `XPac::StartArrow`.
    pub fn pac_start_arrow(&mut self, dir: i32) {
        if dir < 0 || self.pac.state != PacState::Wait {
            return;
        }
        self.pac.move_dir = dir;
        let (x, z) = super::map::step(self.pac.mx, self.pac.mz, dir);
        if !self.map.is_blocked_pac(x, z) {
            self.pac.obj.frame = 3;
            self.pac.ref_pos = self.pac.obj.base.pos;
            self.pac.mx = x;
            self.pac.mz = z;
            self.pac.state = PacState::OnArrow;
            self.pac.local_time = 0;
        }
    }

    /// `XPac::StartDying`.
    pub fn pac_start_dying(&mut self, ctx: &mut Ctx) {
        if matches!(self.pac.state, PacState::Non | PacState::Dying) {
            return;
        }
        ctx.audio.stop_music();
        ctx.audio.play(sfx::PAC_DIE, 100);
        self.camera.start_game_over();
        self.crystals_start_game_over(ctx);
        self.pac.obj.frame = 3;
        self.pac.ref_pos = self.pac.obj.base.pos;
        self.pac.state = PacState::Dying;
        self.pac.local_time = 0;
    }

    /// `XPac::ProcessFrame`.
    pub fn pac_process_frame(&mut self, ctx: &mut Ctx) {
        let chip = self.map.chip;
        let p = &mut self.pac;
        p.local_time += 1;
        if p.fire_energy < 1000 {
            p.fire_energy += 3;
            if p.fire_energy > 1000 {
                ctx.audio.play(sfx::FIRE_READY, 100);
                p.fire_energy = 1000;
            }
        }
        match p.state {
            PacState::Move => {
                let (dist, height);
                if p.local_time < p.move_frames {
                    let f = p.local_time as f32 / p.move_frames as f32;
                    dist = f * chip;
                    height = (f * PI).sin() * 15.0;
                    let fifth = p.move_frames / 5;
                    p.obj.frame = if p.local_time < fifth * 2 {
                        1
                    } else if p.local_time < fifth * 3 {
                        0
                    } else {
                        2
                    };
                } else {
                    dist = chip;
                    height = 0.0;
                    p.obj.frame = 0;
                }
                let mut d = dir_vector(p.move_dir) * dist;
                d.y = height;
                p.obj.base.pos = p.ref_pos + d;
                if p.move_frames <= p.local_time {
                    p.state = PacState::Wait;
                    p.local_time = 0;
                }
            }
            PacState::OnArrow => {
                let dist = if p.local_time < 20 {
                    p.local_time as f32 / 20.0 * chip
                } else {
                    chip
                };
                p.obj.base.pos = p.ref_pos + dir_vector(p.move_dir) * dist;
                if p.local_time > 19 {
                    p.obj.frame = 0;
                    p.state = PacState::Wait;
                    p.local_time = 0;
                }
            }
            PacState::LevelCompleted => {
                p.obj.base.pos.y += 0.7;
                if p.obj.base.pos.y > 500.0 {
                    let pos = p.obj.base.pos;
                    p.state = PacState::Non;
                    self.particles_create_pac_vanish(ctx, pos);
                    self.process.start_process(1);
                }
            }
            PacState::Dying => {
                let h = (p.local_time as f32 / 200.0 * PI).sin() * 140.0;
                p.obj.base.pos = p.ref_pos + v3(0.0, h, 0.0);
                if p.obj.base.pos.y - p.obj.base.radius < 0.0 {
                    let pos = p.obj.base.pos;
                    p.state = PacState::Non;
                    self.particles_create_enemy_explode(ctx, pos);
                    self.process.start_process(2);
                }
            }
            _ => {}
        }
        let p = &mut self.pac;
        if p.state == PacState::Wait {
            if self.map.is_warp_zone(p.mx, p.mz) {
                if p.local_time != 0 && p.local_time.is_multiple_of(80) {
                    if self.crystal_left == 0 {
                        ctx.audio.play(sfx::LEVEL_COMPLETE, 100);
                        self.camera.start_level_completed();
                        p.obj.frame = 3;
                        p.state = PacState::LevelCompleted;
                        p.local_time = 0;
                    } else {
                        self.crystals_do_vanish(ctx);
                    }
                }
            } else if self.map.is_arrow(p.mx, p.mz) {
                let d = self.map.arrow_direction(p.mx, p.mz);
                self.pac_start_arrow(d);
            }
        }
    }
}

//! `XCrystal` / `XCrystalList`.

use super::Ctx;
use super::pac::PacState;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{Axis, HALF_PI, PI, Vec3, v3};
use crate::obj3d::{Obj3D, ObjX};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrystalState {
    Non,
    Process,
    Taken,
    Vanishing,
    GameOver,
}

pub struct Crystal {
    pub obj: ObjX,
    pub state: CrystalState,
    pub local_time: u32,
    pub shadow: Obj3D,
    pub taken_pos: i32,
    pub dir: Vec3,
    pub ref_pos: Vec3,
}

pub const MAX_CRYSTALS: usize = 30;

impl Crystal {
    pub fn new(ctx: &mut Ctx) -> Crystal {
        let mut obj = ObjX::new(ctx.assets, Some(0xab));
        obj.rotate_by(Axis::X, -HALF_PI);
        obj.scale_by(30.0);
        obj.on_ground();
        obj.base.pos.y += 30.0;
        obj.base.radius = 8.0;
        obj.flat = true;
        Crystal {
            obj,
            state: CrystalState::Process,
            local_time: 0,
            shadow: Obj3D::shadow(20.0, 20.0),
            taken_pos: 0,
            dir: Vec3::ZERO,
            ref_pos: Vec3::ZERO,
        }
    }

    pub fn render(&mut self, gfx: &mut Gfx) {
        if self.state == CrystalState::Non {
            return;
        }
        self.obj.render(gfx);
        if self.state == CrystalState::Process {
            self.shadow.base.pos = v3(self.obj.base.pos.x, 1.0, self.obj.base.pos.z);
            self.shadow.render(gfx);
        }
    }

    fn taken_position(&self) -> i32 {
        if self.state == CrystalState::Taken {
            self.taken_pos
        } else {
            0
        }
    }

    fn process_rotate(&mut self, ticks: u32) {
        let t = ticks as f32;
        match self.state {
            CrystalState::Process => self.obj.rotate_by(Axis::Y, t * 0.012_217_3),
            CrystalState::Vanishing | CrystalState::GameOver => {
                self.obj.rotate_by(Axis::Y, t * 0.069_813_2)
            }
            _ => {}
        }
    }

    fn process_frame(&mut self, pac_pos: Vec3) {
        self.local_time += 1;
        match self.state {
            CrystalState::Taken => {
                self.obj.base.pos = pac_pos;
                self.obj.base.pos.y += (self.taken_pos - 1) as f32 * 12.0 + 20.0;
            }
            CrystalState::Vanishing => {
                self.obj.base.pos.y += 0.8;
                if self.obj.base.pos.y > 800.0 {
                    self.state = CrystalState::Non;
                    self.local_time = 0;
                }
            }
            CrystalState::GameOver => {
                let lt = self.local_time as f32;
                let dist = lt / 160.0 * 160.0;
                let h = if self.local_time < 160 {
                    (lt / 160.0 * PI).sin() * 60.0
                } else {
                    (self.local_time - 160) as f32 * -1.4
                };
                let mut d = self.dir * dist;
                d.y = h;
                self.obj.base.pos = self.ref_pos + d;
                if self.obj.base.pos.y < -20.0 {
                    self.state = CrystalState::Non;
                    self.local_time = 0;
                }
            }
            _ => {}
        }
    }
}

impl World {
    pub fn crystals_create(&mut self, ctx: &mut Ctx, x: f32, z: f32) {
        if let Some(slot) = self.crystals.iter().position(|c| c.is_none()) {
            self.crystal_left += 1;
            let mut c = Crystal::new(ctx);
            c.obj.base.pos.x = x;
            c.obj.base.pos.z = z;
            self.crystals[slot] = Some(c);
        }
    }

    pub fn crystals_check_collision(&mut self, ctx: &mut Ctx) {
        if matches!(
            self.pac.state,
            PacState::Non | PacState::LevelCompleted | PacState::Dying | PacState::Wait
        ) {
            return;
        }
        for c in self.crystals.iter_mut().flatten() {
            if c.state == CrystalState::Process && self.pac.obj.base.collides(&c.obj.base) {
                self.taken_count += 1;
                ctx.audio.play(sfx::CRYSTAL_TAKEN, 100);
                c.taken_pos = self.taken_count;
                c.state = CrystalState::Taken;
                c.local_time = 0;
            }
        }
    }

    pub fn crystals_do_vanish(&mut self, ctx: &mut Ctx) {
        if self.taken_count <= 0 {
            return;
        }
        let tc = self.taken_count;
        if let Some(c) = self
            .crystals
            .iter_mut()
            .flatten()
            .find(|c| c.state == CrystalState::Taken && c.taken_position() == tc)
        {
            ctx.audio.play(sfx::CRYSTAL_VANISH, 100);
            c.state = CrystalState::Vanishing;
            c.local_time = 0;
            self.taken_count -= 1;
            self.crystal_left -= 1;
        }
    }

    pub fn crystals_start_game_over(&mut self, ctx: &mut Ctx) {
        for c in self.crystals.iter_mut().flatten() {
            if c.state == CrystalState::Taken {
                let a = ctx.rng.random(360) as f32 * 0.017_453_3;
                c.dir = v3(a.sin(), 0.0, a.cos());
                c.ref_pos = c.obj.base.pos;
                c.state = CrystalState::GameOver;
                c.local_time = 0;
                self.taken_count -= 1;
            }
        }
    }

    pub fn crystals_process_rotate(&mut self, ticks: u32) {
        for c in self.crystals.iter_mut().flatten() {
            c.process_rotate(ticks);
        }
    }

    pub fn crystals_process_frame(&mut self) {
        let pac_pos = self.pac.pos();
        for slot in self.crystals.iter_mut() {
            if let Some(c) = slot {
                c.process_frame(pac_pos);
                if c.state == CrystalState::Non {
                    *slot = None;
                }
            }
        }
    }
}

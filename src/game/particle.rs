//! `XParticleList` and the `XParticle_*` effects (billboarded sprites).

use super::Ctx;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{HALF_PI, Mat, Vec3, v3};
use crate::obj3d::Obj3D;

pub const MAX_PARTICLES: usize = 150;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleKind {
    EnemyExplode {
        dir: Vec3,
    },
    SmokePurple,
    SmokeGreen,
    /// `XParticle_PacFireHit` (also used for the vanish effect).
    FireHit {
        frames: u32,
        layers: u32,
    },
}

pub struct Particle {
    pub obj: Obj3D,
    pub alive: bool,
    pub local_time: u32,
    pub ref_pos: Vec3,
    pub kind: ParticleKind,
}

impl Particle {
    fn new(pos: Vec3, size: f32, kind: ParticleKind) -> Particle {
        let mut obj = Obj3D::particle(size, size);
        obj.base.pos = pos;
        Particle {
            obj,
            alive: true,
            local_time: 0,
            ref_pos: pos,
            kind,
        }
    }

    fn process_frame(&mut self) {
        self.local_time += 1;
        if !self.alive {
            return;
        }
        let lt = self.local_time;
        match &mut self.kind {
            ParticleKind::EnemyExplode { dir } => {
                let (scale, dist) = if lt < 120 {
                    (
                        1.0 - lt as f32 / 120.0,
                        (lt as f32 / 120.0 * HALF_PI).sin() * 55.0,
                    )
                } else {
                    (0.0, 0.0)
                };
                self.obj.scale_to(scale);
                self.obj.base.pos = self.ref_pos + *dir * dist;
                if lt > 119 {
                    self.alive = false;
                }
            }
            ParticleKind::SmokePurple | ParticleKind::SmokeGreen => {
                let s = if lt < 50 { 1.0 - lt as f32 / 50.0 } else { 0.0 };
                self.obj.scale_to(s);
                if lt > 49 {
                    self.alive = false;
                }
            }
            ParticleKind::FireHit { frames, layers } => {
                let s = if lt < *frames {
                    1.0 - (lt as f32 / *frames as f32) * 0.8
                } else {
                    0.2
                };
                self.obj.scale_to(s);
                if *frames >> 1 <= lt {
                    *layers = 1;
                }
                if *frames <= lt {
                    self.alive = false;
                }
            }
        }
    }

    fn render(&mut self, gfx: &mut Gfx, view: &Mat, rng: &mut super::Rng) {
        if !self.alive {
            return;
        }
        match self.kind {
            ParticleKind::FireHit { layers, .. } => {
                let m = self.obj.billboard_matrix(view);
                for _ in 0..layers {
                    let uv = random_fire_cell(rng);
                    self.obj.render2_layer(gfx, &m, 0xa0, uv);
                }
            }
            _ => self.obj.render2(gfx, view),
        }
    }
}

/// `_SetTextureRandomly`: one of the 3x3 cells of the fire texture.
pub fn random_fire_cell(rng: &mut super::Rng) -> (f32, f32, f32, f32) {
    const T: f32 = 0.333_333_3;
    const TT: f32 = 0.666_666_7;
    let r = rng.random(9);
    let col = [0.0, T, TT][(r % 3) as usize];
    let row = [0.0, T, TT][(r / 3) as usize];
    let end = |v: f32| {
        if v == TT {
            1.0
        } else if v == T {
            TT
        } else {
            T
        }
    };
    (col, row, end(col), end(row))
}

impl World {
    fn particle_slot(&mut self) -> Option<usize> {
        let start = self.particle_ptr;
        let mut found = None;
        loop {
            if found.is_some() {
                return found;
            }
            if self.particles[self.particle_ptr].is_none() {
                found = Some(self.particle_ptr);
            }
            self.particle_ptr = (self.particle_ptr + 1) % MAX_PARTICLES;
            if self.particle_ptr == start {
                return found;
            }
        }
    }

    pub fn particles_create_enemy_explode(&mut self, ctx: &mut Ctx, pos: Vec3) {
        let vol = self.volume_of(pos);
        ctx.audio.play(sfx::ENEMY_EXPLODE, vol);
        for _ in 0..30 {
            let Some(slot) = self.particle_slot() else {
                break;
            };
            let a = ctx.rng.random(180) as f32 * 0.017_453_3;
            let b = ctx.rng.random(360) as f32 * 0.017_453_3;
            let dir = v3(b.sin() * a.cos(), a.sin(), b.cos() * a.cos());
            let mut p = Particle::new(pos, 50.0, ParticleKind::EnemyExplode { dir });
            p.obj.set_rect_texture(0xa2, 0.0, 0.0, 1.0, 1.0);
            self.particles[slot] = Some(p);
        }
    }

    pub fn particles_create_smoke(&mut self, pos: Vec3, green: bool) {
        if let Some(slot) = self.particle_slot() {
            let (size, tex, kind) = if green {
                (17.0, 0xb7, ParticleKind::SmokeGreen)
            } else {
                (18.0, 0xaa, ParticleKind::SmokePurple)
            };
            let mut p = Particle::new(pos, size, kind);
            p.obj.set_rect_texture(tex, 0.0, 0.0, 1.0, 1.0);
            self.particles[slot] = Some(p);
        }
    }

    pub fn particles_create_fire_hit(&mut self, ctx: &mut Ctx, pos: Vec3) {
        if let Some(slot) = self.particle_slot() {
            let vol = self.volume_of(pos);
            ctx.audio.play(sfx::PAC_FIRE_HIT, vol);
            self.particles[slot] = Some(Particle::new(
                pos,
                70.0,
                ParticleKind::FireHit {
                    frames: 16,
                    layers: 2,
                },
            ));
        }
    }

    pub fn particles_create_pac_vanish(&mut self, ctx: &mut Ctx, pos: Vec3) {
        if let Some(slot) = self.particle_slot() {
            ctx.audio.play(sfx::PAC_VANISH, 100);
            self.particles[slot] = Some(Particle::new(
                pos,
                150.0,
                ParticleKind::FireHit {
                    frames: 150,
                    layers: 2,
                },
            ));
        }
    }

    pub fn particles_process_frame(&mut self) {
        for slot in self.particles.iter_mut() {
            if let Some(p) = slot {
                p.process_frame();
                if !p.alive {
                    *slot = None;
                }
            }
        }
    }

    pub fn particles_render(&mut self, gfx: &mut Gfx, rng: &mut super::Rng) {
        let view = self.camera.view;
        for p in self.particles.iter_mut().flatten() {
            p.render(gfx, &view, rng);
        }
    }
}

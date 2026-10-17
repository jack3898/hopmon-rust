//! `XPacFireList`/`XPacFire` and `XEnemyFireList`/`XEnemyFire_*`.

use super::Ctx;
use super::enemy::EnemyState;
use super::pac::PacState;
use super::particle::random_fire_cell;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{Mat, PI, Vec3, dir_vector, v3};
use crate::obj3d::{AlphaState, Filter, Obj3D};

pub const MAX_PAC_FIRES: usize = 3;
pub const MAX_ENEMY_FIRES: usize = 60;

pub struct PacFire {
    pub obj: Obj3D,
    pub alive: bool,
    pub local_time: u32,
    pub dir: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnemyFireKind {
    /// Lobbed shell from an up-cannon or volcano.
    CannonUp {
        distance: f32,
        dir: Vec3,
    },
    Cannon {
        dir: Vec3,
    },
    Ufo {
        dir: Vec3,
        turned: bool,
    },
    Steam,
}

pub struct EnemyFire {
    pub obj: Obj3D,
    pub alive: bool,
    pub local_time: u32,
    pub ref_pos: Vec3,
    pub kind: EnemyFireKind,
}

impl EnemyFire {
    fn new(pos: Vec3, size: f32, kind: EnemyFireKind) -> EnemyFire {
        let mut obj = Obj3D::particle(size, size);
        obj.base.pos = pos;
        EnemyFire {
            obj,
            alive: true,
            local_time: 0,
            ref_pos: pos,
            kind,
        }
    }
}

impl World {
    /// `XPacFireList::CreateFire`.
    pub fn pac_fire_create(&mut self, ctx: &mut Ctx) {
        let Some(slot) = self.pac_fires.iter().position(|f| f.is_none()) else {
            return;
        };
        ctx.audio.play(sfx::PAC_FIRE, 100);
        let mut obj = Obj3D::particle(24.0, 24.0);
        obj.base.radius = 8.0;
        obj.base.pos = self.pac.pos() + dir_vector(self.pac.face) * 20.0;
        self.pac_fires[slot] = Some(PacFire {
            obj,
            alive: true,
            local_time: 0,
            dir: self.pac.face,
        });
    }

    /// `XPacFireList::CheckCollisionWithEnemy`.
    pub fn pac_fire_check_collision(&mut self, ctx: &mut Ctx) {
        for fi in 0..MAX_PAC_FIRES {
            // As in the original, liveness is only checked before scanning the
            // enemies, so one shot can hit several overlapping enemies.
            let Some(f) = &self.pac_fires[fi] else {
                continue;
            };
            if !f.alive {
                continue;
            }
            let fire_base = f.obj.base;
            let fire_dir = f.dir;
            for ei in 0..self.enemies.len() {
                let hit = match &self.enemies[ei] {
                    Some(e) => {
                        e.state != EnemyState::Non
                            && e.state != EnemyState::Dying
                            && e.obj.base.collides(&fire_base)
                    }
                    None => false,
                };
                if hit {
                    if self.enemies[ei].as_ref().unwrap().breakable {
                        self.enemy_decrease_shield(ei, fire_dir);
                        self.particles_create_fire_hit(ctx, fire_base.pos);
                    }
                    if let Some(f) = &mut self.pac_fires[fi] {
                        f.alive = false;
                    }
                }
            }
        }
    }

    pub fn pac_fire_process_frame(&mut self) {
        for i in 0..MAX_PAC_FIRES {
            let Some(f) = &mut self.pac_fires[i] else {
                continue;
            };
            f.local_time += 1;
            if f.alive {
                f.obj.base.pos += dir_vector(f.dir) * 2.5;
                let mx = self.map.pos_x_to_map(f.obj.base.pos.x);
                let mz = self.map.pos_z_to_map(f.obj.base.pos.z);
                if self.map.is_blocked_fire(mx, mz) {
                    f.alive = false;
                }
            }
            if !f.alive {
                self.pac_fires[i] = None;
            }
        }
    }

    pub fn pac_fire_render(&mut self, gfx: &mut Gfx, view: &Mat, rng: &mut super::Rng) {
        for f in self.pac_fires.iter_mut().flatten() {
            if !f.alive {
                continue;
            }
            let m = f.obj.billboard_matrix(view);
            for layer in 0..3 {
                let (id, uv) = if layer == 0 {
                    (0xa1, (0.0, 0.0, 1.0, 1.0))
                } else {
                    (0xa0, random_fire_cell(rng))
                };
                f.obj.render2_layer(gfx, &m, id, uv);
            }
        }
    }

    fn enemy_fire_slot(&self) -> Option<usize> {
        self.enemy_fires.iter().position(|f| f.is_none())
    }

    fn lobbed_fire(&mut self, ctx: &mut Ctx, pos: Vec3, distance: f32) {
        let Some(slot) = self.enemy_fire_slot() else {
            return;
        };
        let vol = self.volume_of(pos);
        ctx.audio.play(sfx::CANNON_UP_FIRE, vol);
        let a = ctx.rng.random(360) as f32 * 0.017_453_3;
        let dir = v3(a.sin(), 0.0, a.cos());
        let mut f = EnemyFire::new(pos, 18.0, EnemyFireKind::CannonUp { distance, dir });
        f.obj.set_rect_texture(0xa9, 0.0, 0.0, 1.0, 1.0);
        f.obj.base.radius = 7.0;
        self.enemy_fires[slot] = Some(f);
    }

    pub fn enemy_fire_cannon_up(&mut self, ctx: &mut Ctx, pos: Vec3) {
        self.lobbed_fire(ctx, pos, 45.0);
    }

    pub fn enemy_fire_volcano(&mut self, ctx: &mut Ctx, pos: Vec3) {
        self.lobbed_fire(ctx, pos, 80.0);
    }

    pub fn enemy_fire_cannon(&mut self, ctx: &mut Ctx, pos: Vec3, dir: i32) {
        let Some(slot) = self.enemy_fire_slot() else {
            return;
        };
        let vol = self.volume_of(pos);
        ctx.audio.play(sfx::CANNON_FIRE, vol);
        let mut f = EnemyFire::new(
            pos,
            18.0,
            EnemyFireKind::Cannon {
                dir: dir_vector(dir),
            },
        );
        f.obj.set_rect_texture(0xa9, 0.0, 0.0, 1.0, 1.0);
        f.obj.base.radius = 7.0;
        self.enemy_fires[slot] = Some(f);
    }

    pub fn enemy_fire_ufo(&mut self, ctx: &mut Ctx, pos: Vec3) {
        let Some(slot) = self.enemy_fire_slot() else {
            return;
        };
        let vol = self.volume_of(pos);
        ctx.audio.play(sfx::CANNON_FIRE, vol);
        let mut f = EnemyFire::new(
            pos,
            17.0,
            EnemyFireKind::Ufo {
                dir: v3(0.0, -1.0, 0.0),
                turned: false,
            },
        );
        f.obj.set_rect_texture(0xb6, 0.0, 0.0, 1.0, 1.0);
        f.obj.base.radius = 7.0;
        self.enemy_fires[slot] = Some(f);
    }

    pub fn enemy_fire_steam(&mut self, pos: Vec3) {
        let Some(slot) = self.enemy_fire_slot() else {
            return;
        };
        let mut f = EnemyFire::new(pos, 18.0, EnemyFireKind::Steam);
        f.obj.alpha = AlphaState::Transparent;
        f.obj.filter = Filter::Point;
        f.obj.set_rect_texture(0xdc, 0.0, 0.0, 1.0, 1.0);
        f.obj.base.radius = 7.0;
        self.enemy_fires[slot] = Some(f);
    }

    /// `XEnemyFireList::CheckCollisionWithPac`.
    pub fn enemy_fire_check_collision(&mut self, ctx: &mut Ctx) {
        if matches!(
            self.pac.state,
            PacState::Non | PacState::LevelCompleted | PacState::Dying
        ) {
            return;
        }
        for i in 0..MAX_ENEMY_FIRES {
            let hit = match &self.enemy_fires[i] {
                Some(f) => f.alive && self.pac.obj.base.collides(&f.obj.base),
                None => false,
            };
            if hit {
                self.enemy_fires[i].as_mut().unwrap().alive = false;
                self.pac_start_dying(ctx);
                return;
            }
        }
    }

    pub fn enemy_fire_process_frame(&mut self, ctx: &mut Ctx) {
        for i in 0..MAX_ENEMY_FIRES {
            let Some(mut f) = self.enemy_fires[i].take() else {
                continue;
            };
            f.local_time += 1;
            if f.alive {
                self.enemy_fire_step(ctx, &mut f);
            }
            if f.alive {
                self.enemy_fires[i] = Some(f);
            }
        }
    }

    fn enemy_fire_step(&mut self, ctx: &mut Ctx, f: &mut EnemyFire) {
        let lt = f.local_time;
        match &mut f.kind {
            EnemyFireKind::CannonUp { distance, dir } => {
                let d = lt as f32 / 200.0 * *distance;
                let h = (lt as f32 / 200.0 * PI).sin() * 100.0;
                let mut v = *dir * d;
                v.y = h;
                f.obj.base.pos = f.ref_pos + v;
                if lt.is_multiple_of(8) {
                    self.particles_create_smoke(f.obj.base.pos, false);
                }
                if f.obj.base.pos.y < 0.0 {
                    f.alive = false;
                }
            }
            EnemyFireKind::Cannon { dir } => {
                f.obj.base.pos += *dir * 1.6;
                if lt.is_multiple_of(8) {
                    self.particles_create_smoke(f.obj.base.pos, false);
                }
                let mx = self.map.pos_x_to_map(f.obj.base.pos.x);
                let mz = self.map.pos_z_to_map(f.obj.base.pos.z);
                if lt > 15 && self.map.is_blocked_fire(mx, mz) {
                    f.alive = false;
                }
            }
            EnemyFireKind::Ufo { dir, turned } => {
                f.obj.base.pos += *dir * 1.2;
                if lt.is_multiple_of(9) {
                    self.particles_create_smoke(f.obj.base.pos, true);
                }
                let mx = self.map.pos_x_to_map(f.obj.base.pos.x);
                let mz = self.map.pos_z_to_map(f.obj.base.pos.z);
                if !*turned {
                    if f.obj.base.pos.y < 20.0 {
                        let (px, pz) = (self.pac.mx, self.pac.mz);
                        let dx = (px - mx).abs();
                        let dz = (pz - mz).abs();
                        let d = if dx == 0 && dz == 0 {
                            ctx.rng.random(4)
                        } else if dz < dx {
                            if mx < px { 1 } else { 3 }
                        } else if dx < dz {
                            if mz < pz { 2 } else { 0 }
                        } else if ctx.rng.random(2) == 0 {
                            if mx < px { 1 } else { 3 }
                        } else if mz < pz {
                            2
                        } else {
                            0
                        };
                        *dir = dir_vector(d);
                        *turned = true;
                    }
                } else if lt > 15 && self.map.is_blocked_fire(mx, mz) {
                    f.alive = false;
                }
            }
            EnemyFireKind::Steam => {
                let t = lt as f32 / 80.0;
                f.obj.base.pos.y = f.ref_pos.y + t * 120.0;
                f.obj.set_alpha_factor((1.0 - t) * 0.9 + 0.1);
                if lt > 79 {
                    f.alive = false;
                }
            }
        }
    }

    pub fn enemy_fire_render(&mut self, gfx: &mut Gfx, view: &Mat) {
        for f in self.enemy_fires.iter().flatten() {
            if f.alive {
                f.obj.render2(gfx, view);
            }
        }
    }
}

//! `XEnemyList`, `XEnemyBase` and every enemy subclass.
//!
//! Map letters: `a` ghost, `b` cube (2 hits), `c` jellyfish (kurage), `d` UFO,
//! `e` sesame (2 hits), `m`/`n` needle ball moving north/south, `o`/`p` fast
//! needle ball, `q` needle box (periodic), `r` needle box that drops when
//! Hopmon steps below it, `x`/`y` gates.

use super::Ctx;
use super::pac::PacState;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{Axis, HALF_PI, PI, Vec3, dir_vector, v3};
use crate::obj3d::{Obj3D, ObjX};

pub const MAX_ENEMIES: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyState {
    Non,
    Wait,
    Move,
    Fire,
    Dying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Ghost,
    Cube,
    Kurage,
    Ufo { move_count: i32, countdown: i32 },
    Sesame { countdown: i32, phase: i32 },
    NeedleBall { fast: bool },
    NeedleBox,
    NeedleBoxDrop,
    Gate,
}

pub struct Enemy {
    pub obj: ObjX,
    pub state: EnemyState,
    pub local_time: u32,
    pub mx: i32,
    pub mz: i32,
    pub ref_pos: Vec3,
    pub dir: i32,
    pub shadow: Obj3D,
    pub face_to_dir: bool,
    pub breakable: bool,
    pub shield: i32,
    pub kind: EnemyKind,
}

impl Enemy {
    fn base(ctx: &mut Ctx, mesh: u16, kind: EnemyKind) -> Enemy {
        Enemy {
            obj: ObjX::new(ctx.assets, Some(mesh)),
            state: EnemyState::Wait,
            local_time: 0,
            mx: -1,
            mz: -1,
            ref_pos: Vec3::ZERO,
            dir: -1,
            shadow: Obj3D::shadow(1.0, 1.0),
            face_to_dir: true,
            breakable: true,
            shield: 1,
            kind,
        }
    }

    /// Common constructor tail: rotate upright, scale, sit on the ground.
    fn setup(&mut self, shadow: f32, scale: f32, lift: f32, radius: f32) {
        self.shadow.scale_by(shadow);
        self.obj.rotate_by(Axis::X, -HALF_PI);
        self.obj.scale_by(scale);
        self.obj.on_ground();
        self.obj.base.pos.y += lift;
        self.obj.base.radius = radius;
    }

    /// `XEnemyList::CreateEnemy`.
    pub fn create(ctx: &mut Ctx, c: u8) -> Option<Enemy> {
        let e = match c {
            b'a' => {
                let mut e = Enemy::base(ctx, 0x9d, EnemyKind::Ghost);
                e.setup(40.0, 50.0, 9.0, 15.0);
                e.face_to_dir = true;
                let d = ctx.rng.random(4);
                e.set_direction(d);
                e.shield = 1;
                e
            }
            b'b' => {
                let mut e = Enemy::base(ctx, 0x149, EnemyKind::Cube);
                e.obj.append_frame(ctx.assets, 0xa4);
                e.obj.frame = 1;
                e.setup(40.0, 63.0, 10.0, 15.0);
                e.face_to_dir = false;
                let d = ctx.rng.random(4);
                e.set_direction(d);
                e.shield = 2;
                e
            }
            b'c' => {
                let mut e = Enemy::base(ctx, 0xaf, EnemyKind::Kurage);
                e.obj.append_frame(ctx.assets, 0xae);
                e.setup(40.0, 80.0, 3.0, 15.0);
                e.face_to_dir = false;
                let d = ctx.rng.random(4);
                e.set_direction(d);
                e.shield = 1;
                e
            }
            b'd' => {
                let mut e = Enemy::base(
                    ctx,
                    0xad,
                    EnemyKind::Ufo {
                        move_count: 0,
                        countdown: 0,
                    },
                );
                e.setup(50.0, 60.0, 100.0, 18.0);
                e.face_to_dir = false;
                let d = ctx.rng.random(4);
                e.set_direction(d);
                e.breakable = false;
                let cd = ctx.rng.random(6) + 3;
                e.kind = EnemyKind::Ufo {
                    move_count: 0,
                    countdown: cd,
                };
                e
            }
            b'e' => {
                let mut e = Enemy::base(
                    ctx,
                    0x14a,
                    EnemyKind::Sesame {
                        countdown: 0,
                        phase: 0,
                    },
                );
                e.obj.append_frame(ctx.assets, 0x14b);
                e.obj.append_frame(ctx.assets, 0xd2);
                e.obj.append_frame(ctx.assets, 0xd4);
                e.obj.frame = 2;
                e.setup(40.0, 53.0, 9.0, 15.0);
                e.face_to_dir = true;
                let d = ctx.rng.random(4);
                e.set_direction(d);
                e.shield = 2;
                let cd = ctx.rng.random(15) + 15;
                e.kind = EnemyKind::Sesame {
                    countdown: cd,
                    phase: 0,
                };
                e
            }
            b'm' | b'n' | b'o' | b'p' => {
                let fast = c == b'o' || c == b'p';
                let mesh = if fast { 0xb3 } else { 0xa5 };
                let mut e = Enemy::base(ctx, mesh, EnemyKind::NeedleBall { fast });
                e.setup(40.0, 30.0, 3.0, 15.0);
                e.face_to_dir = false;
                e.set_direction(if c == b'm' || c == b'o' { 0 } else { 2 });
                e.breakable = false;
                e
            }
            b'q' | b'r' => {
                let drop = c == b'r';
                let mesh = if drop { 0xb1 } else { 0xb2 };
                let kind = if drop {
                    EnemyKind::NeedleBoxDrop
                } else {
                    EnemyKind::NeedleBox
                };
                let mut e = Enemy::base(ctx, mesh, kind);
                e.setup(40.0, 35.0, 60.0, 15.0);
                e.breakable = false;
                e
            }
            b'x' | b'y' => {
                let mut e = Enemy::base(ctx, 0xb4, EnemyKind::Gate);
                e.shadow.scale_by(0.0);
                e.obj.rotate_by(Axis::X, -HALF_PI);
                e.obj.scale_by(35.0);
                e.obj.on_ground();
                e.obj.base.radius = 10.0;
                e.face_to_dir = true;
                e.set_direction(if c == b'x' { 0 } else { 1 });
                e.shield = 1;
                e
            }
            _ => return None,
        };
        Some(e)
    }

    pub fn set_direction(&mut self, d: i32) {
        if self.face_to_dir {
            let a = match d {
                0 => Some(0.0),
                1 => Some(HALF_PI),
                2 => Some(PI),
                3 => Some(4.712_389),
                _ => None,
            };
            if let Some(a) = a {
                self.obj.rotate_to(Axis::Y, a);
            }
        }
        self.dir = d;
    }

    pub fn render(&mut self, gfx: &mut Gfx) {
        if self.state == EnemyState::Non {
            return;
        }
        self.obj.render(gfx);
        self.shadow.base.pos = v3(self.obj.base.pos.x, 1.0, self.obj.base.pos.z);
        self.shadow.render(gfx);
    }

    fn process_rotate(&mut self, ticks: u32) {
        let t = ticks as f32;
        match self.state {
            EnemyState::Dying => {
                let f = t * 0.139_626_3;
                match self.dir {
                    0 => self.obj.rotate_by(Axis::X, -f),
                    2 => self.obj.rotate_by(Axis::X, f),
                    1 => self.obj.rotate_by(Axis::Z, f),
                    3 => self.obj.rotate_by(Axis::Z, -f),
                    _ => {}
                }
            }
            EnemyState::Move => {
                let roll = match self.kind {
                    EnemyKind::Cube => Some(0.026_851_2),
                    EnemyKind::NeedleBall { fast: false } => Some(0.031_415_9),
                    EnemyKind::NeedleBall { fast: true } => Some(0.044_879_9),
                    EnemyKind::Ufo { .. } => {
                        self.obj.rotate_by(Axis::Y, t * 0.017_453_3);
                        None
                    }
                    _ => None,
                };
                if let Some(k) = roll {
                    let f = t * k;
                    match self.dir {
                        0 => self.obj.rotate_by(Axis::X, f),
                        2 => self.obj.rotate_by(Axis::X, -f),
                        1 => self.obj.rotate_by(Axis::Z, -f),
                        3 => self.obj.rotate_by(Axis::Z, f),
                        _ => {}
                    }
                }
            }
            EnemyState::Fire => {
                if let EnemyKind::Ufo { .. } = self.kind {
                    self.obj.rotate_by(Axis::Y, t * 0.043_633_2);
                }
            }
            _ => {}
        }
    }

    /// Linear slide by one map cell over `frames` ticks.
    fn slide(&mut self, chip: f32, frames: u32) {
        let d = if self.local_time < frames {
            self.local_time as f32 / frames as f32 * chip
        } else {
            chip
        };
        self.obj.base.pos = self.ref_pos + dir_vector(self.dir) * d;
    }
}

impl World {
    pub fn enemy_create(&mut self, ctx: &mut Ctx, c: u8, x: f32, z: f32, mx: i32, mz: i32) {
        let Some(slot) = self.enemies.iter().position(|e| e.is_none()) else {
            return;
        };
        if let Some(mut e) = Enemy::create(ctx, c) {
            e.obj.base.pos.x = x;
            e.obj.base.pos.z = z;
            e.mx = mx;
            e.mz = mz;
            self.enemies[slot] = Some(e);
        }
    }

    pub fn enemies_check_collision(&mut self, ctx: &mut Ctx) {
        if matches!(
            self.pac.state,
            PacState::Non | PacState::LevelCompleted | PacState::Dying
        ) {
            return;
        }
        let hit = self.enemies.iter().flatten().any(|e| {
            e.state != EnemyState::Non
                && e.state != EnemyState::Dying
                && self.pac.obj.base.collides(&e.obj.base)
        });
        if hit {
            self.pac_start_dying(ctx);
        }
    }

    pub fn enemies_process_rotate(&mut self, ticks: u32) {
        for e in self.enemies.iter_mut().flatten() {
            e.process_rotate(ticks);
        }
    }

    pub fn enemy_decrease_shield(&mut self, i: usize, dir: i32) {
        let Some(e) = &mut self.enemies[i] else {
            return;
        };
        e.shield -= 1;
        if e.shield < 1 {
            self.map.zero_enemy_map(e.mx, e.mz);
            e.obj.frame = 0;
            e.set_direction((dir + 2).rem_euclid(4));
            e.ref_pos = e.obj.base.pos;
            e.state = EnemyState::Dying;
            e.local_time = 0;
        }
    }

    pub fn enemies_process_frame(&mut self, ctx: &mut Ctx) {
        for i in 0..MAX_ENEMIES {
            let Some(mut e) = self.enemies[i].take() else {
                continue;
            };
            e.local_time += 1;
            match e.state {
                EnemyState::Dying => self.enemy_dying(ctx, &mut e),
                EnemyState::Move => self.enemy_move(ctx, &mut e),
                EnemyState::Fire => self.enemy_fire(ctx, &mut e),
                _ => {}
            }
            if e.state == EnemyState::Wait {
                self.enemy_wait(ctx, &mut e);
            }
            if e.state != EnemyState::Non {
                self.enemies[i] = Some(e);
            }
        }
    }

    fn enemy_dying(&mut self, ctx: &mut Ctx, e: &mut Enemy) {
        let h = (e.local_time as f32 / 160.0 * PI).sin() * 120.0;
        e.obj.base.pos = e.ref_pos + v3(0.0, h, 0.0);
        if e.obj.base.pos.y - e.obj.base.radius < 0.0 {
            self.particles_create_enemy_explode(ctx, e.obj.base.pos);
            e.state = EnemyState::Non;
            e.local_time = 0;
        }
    }

    fn enemy_is_blocked(&self, e: &Enemy, dir: i32) -> bool {
        let (x, z) = super::map::step(e.mx, e.mz, dir);
        self.map.is_blocked_enemy(x, z)
    }

    fn enemy_start_moving(&mut self, e: &mut Enemy, dir: i32) {
        if e.state != EnemyState::Wait {
            return;
        }
        let (x, z) = super::map::step(e.mx, e.mz, dir);
        self.map.update_enemy_map(e.mx, e.mz, x, z);
        e.mx = x;
        e.mz = z;
        e.set_direction(dir);
        e.ref_pos = e.obj.base.pos;
        e.state = EnemyState::Move;
        e.local_time = 0;
    }

    fn find_direction_to_pac(&self, ctx: &mut Ctx, e: &Enemy) -> i32 {
        let (px, pz) = (self.pac.mx, self.pac.mz);
        let dx = (px - e.mx).abs();
        let dz = (pz - e.mz).abs();
        if dx == 0 && dz == 0 {
            -1
        } else if dz < dx {
            if e.mx < px { 1 } else { 3 }
        } else if dx < dz {
            if e.mz < pz { 2 } else { 0 }
        } else if ctx.rng.random(2) == 0 {
            if e.mx < px { 1 } else { 3 }
        } else if e.mz < pz {
            2
        } else {
            0
        }
    }

    /// `XEnemyBase::DefaultProcessWait`: wander, preferring straight ahead
    /// and sometimes turning towards Hopmon; reverse only when stuck.
    fn default_wait(&mut self, ctx: &mut Ctx, e: &mut Enemy) {
        let right = (e.dir + 1).rem_euclid(4);
        let back = (e.dir + 2).rem_euclid(4);
        let left = (e.dir + 3).rem_euclid(4);
        let mut choices = Vec::new();
        if !self.enemy_is_blocked(e, e.dir) {
            choices.push(e.dir);
            choices.push(e.dir);
        }
        if !self.enemy_is_blocked(e, right) {
            choices.push(right);
        }
        if !self.enemy_is_blocked(e, left) {
            choices.push(left);
        }
        let to_pac = self.find_direction_to_pac(ctx, e);
        if to_pac >= 0 && to_pac != back && !self.enemy_is_blocked(e, to_pac) {
            choices.push(to_pac);
        }
        if choices.is_empty() {
            if !self.enemy_is_blocked(e, back) {
                self.enemy_start_moving(e, back);
            }
        } else {
            let d = choices[ctx.rng.random(choices.len() as i32) as usize];
            self.enemy_start_moving(e, d);
        }
    }

    fn enemy_move(&mut self, ctx: &mut Ctx, e: &mut Enemy) {
        let chip = self.map.chip;
        let lt = e.local_time;
        match e.kind {
            EnemyKind::Ghost => {
                e.slide(chip, 60);
                if lt > 59 {
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::Cube => {
                e.slide(chip, 65);
                e.obj.frame = (e.shield > 1) as usize;
                if lt > 64 {
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::Kurage => {
                let (d, h) = if lt < 80 {
                    e.obj.frame = 1;
                    (
                        lt as f32 / 80.0 * chip,
                        (lt as f32 / 80.0 * PI).sin() * 47.0,
                    )
                } else {
                    e.obj.frame = 0;
                    (chip, 0.0)
                };
                let mut v = dir_vector(e.dir) * d;
                v.y = h;
                e.obj.base.pos = e.ref_pos + v;
                if lt > 79 {
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::Ufo {
                move_count,
                countdown,
            } => {
                e.slide(chip, 100);
                if lt > 99 {
                    e.kind = EnemyKind::Ufo {
                        move_count: move_count + 1,
                        countdown,
                    };
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::Sesame { countdown, phase } => {
                let base = if e.shield < 2 { 0 } else { 2 };
                let frames = if phase == 0 {
                    e.obj.frame = if lt < 35 { base } else { base + 1 };
                    70
                } else {
                    e.obj.frame = base + 1;
                    25
                };
                e.slide(chip, frames);
                if frames <= lt {
                    e.kind = EnemyKind::Sesame {
                        countdown: countdown - 1,
                        phase,
                    };
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::NeedleBall { fast } => {
                let frames = if fast { 35 } else { 50 };
                e.slide(chip, frames);
                if lt >= frames {
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::NeedleBox => {
                let mut h = 0.0;
                if lt >= 200 {
                    if lt < 240 {
                        h = 60.0 - ((lt - 200) as f32 / 40.0 * HALF_PI).cos() * 60.0;
                    } else if lt < 390 {
                        if lt == 240 {
                            let vol = self.volume_of(e.obj.base.pos);
                            ctx.audio.play(sfx::NEEDLE_BOX, vol);
                        }
                        h = 60.0;
                    } else if lt < 540 {
                        h = 60.0 - (lt - 390) as f32 * 0.4;
                    }
                }
                e.obj.base.pos = e.ref_pos + v3(0.0, -h, 0.0);
                if lt > 539 {
                    e.state = EnemyState::Wait;
                    e.local_time = 0;
                }
            }
            EnemyKind::NeedleBoxDrop => {
                let mut h = 0.0;
                if lt >= 80 {
                    if lt < 120 {
                        h = 60.0 - ((lt - 80) as f32 / 40.0 * HALF_PI).cos() * 60.0;
                    } else {
                        if lt == 120 {
                            let vol = self.volume_of(e.obj.base.pos);
                            ctx.audio.play(sfx::NEEDLE_BOX, vol);
                        }
                        h = 60.0;
                    }
                }
                e.obj.base.pos = e.ref_pos + v3(0.0, -h, 0.0);
            }
            EnemyKind::Gate => {}
        }
    }

    fn enemy_wait(&mut self, ctx: &mut Ctx, e: &mut Enemy) {
        match e.kind {
            EnemyKind::Ghost | EnemyKind::Cube => self.default_wait(ctx, e),
            EnemyKind::Kurage => {
                if e.local_time > 20 {
                    self.default_wait(ctx, e);
                }
            }
            EnemyKind::Ufo {
                mut move_count,
                mut countdown,
            } => {
                let mut d = e.dir;
                if move_count > 2 {
                    move_count = 0;
                    countdown -= 1;
                    if countdown < 1 {
                        countdown = ctx.rng.random(5) + 2;
                        if !self.map.is_blocked_fire(e.mx, e.mz) {
                            let vol = self.volume_of(e.obj.base.pos);
                            ctx.audio.play(sfx::UFO_FIRE, vol);
                            e.kind = EnemyKind::Ufo {
                                move_count,
                                countdown,
                            };
                            e.ref_pos = e.obj.base.pos;
                            e.state = EnemyState::Fire;
                            e.local_time = 0;
                            return;
                        }
                    }
                    if ctx.rng.random(2) == 0 {
                        d = self.find_direction_to_pac(ctx, e);
                        if d < 0 {
                            d = ctx.rng.random(4);
                        }
                    } else {
                        d = ctx.rng.random(4);
                    }
                }
                e.kind = EnemyKind::Ufo {
                    move_count,
                    countdown,
                };
                let blocked = |w: &World, d: i32| {
                    let (x, z) = super::map::step(e.mx, e.mz, d);
                    w.map.is_blocked_ufo(x, z)
                };
                if blocked(self, d) {
                    d = ctx.rng.random(4);
                }
                if !blocked(self, d) && e.state == EnemyState::Wait {
                    let (x, z) = super::map::step(e.mx, e.mz, d);
                    self.map.update_ufo_map(e.mx, e.mz, x, z);
                    e.mx = x;
                    e.mz = z;
                    e.set_direction(d);
                    e.ref_pos = e.obj.base.pos;
                    e.state = EnemyState::Move;
                    e.local_time = 0;
                }
            }
            EnemyKind::Sesame { countdown, phase } => match phase {
                0 => {
                    if countdown < 1 {
                        e.kind = EnemyKind::Sesame {
                            countdown,
                            phase: 1,
                        };
                    } else {
                        self.default_wait(ctx, e);
                    }
                }
                1 => {
                    e.obj.frame = if e.shield < 2 { 0 } else { 2 };
                    if e.local_time > 249 {
                        e.kind = EnemyKind::Sesame {
                            countdown: 10,
                            phase: 2,
                        };
                    }
                }
                _ => {
                    if countdown < 1 {
                        let cd = ctx.rng.random(15) + 15;
                        e.kind = EnemyKind::Sesame {
                            countdown: cd,
                            phase: 0,
                        };
                    }
                    self.default_wait(ctx, e);
                }
            },
            EnemyKind::NeedleBall { .. } => {
                let d = e.dir;
                for nd in [
                    d,
                    (d + 1).rem_euclid(4),
                    (d + 3).rem_euclid(4),
                    (d + 2).rem_euclid(4),
                ] {
                    if !self.enemy_is_blocked(e, nd) {
                        self.enemy_start_moving(e, nd);
                        break;
                    }
                }
            }
            EnemyKind::NeedleBox => {
                e.ref_pos = e.obj.base.pos;
                e.state = EnemyState::Move;
                e.local_time = 0;
            }
            EnemyKind::NeedleBoxDrop => {
                if self.pac.mx == e.mx && self.pac.mz == e.mz {
                    e.ref_pos = e.obj.base.pos;
                    e.state = EnemyState::Move;
                    e.local_time = 0;
                }
            }
            EnemyKind::Gate => {}
        }
    }

    fn enemy_fire(&mut self, ctx: &mut Ctx, e: &mut Enemy) {
        if let EnemyKind::Ufo { .. } = e.kind {
            if e.local_time == 170 {
                self.enemy_fire_ufo(ctx, e.obj.base.pos);
            } else if e.local_time > 299 {
                e.state = EnemyState::Wait;
                e.local_time = 0;
            }
        }
    }

    pub fn enemies_render(&mut self, gfx: &mut Gfx) {
        for e in self.enemies.iter_mut().flatten() {
            e.render(gfx);
        }
    }
}

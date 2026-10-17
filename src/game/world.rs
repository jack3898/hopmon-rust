//! `XGameObject`: one level in play.

use super::camera::{CamState, Camera};
use super::crystal::{Crystal, MAX_CRYSTALS};
use super::enemy::{Enemy, MAX_ENEMIES};
use super::fire::{EnemyFire, MAX_ENEMY_FIRES, MAX_PAC_FIRES, PacFire};
use super::map::{Map, Spawn, world_texture};
use super::mapobj::{MAX_MAP_OBJECTS, MapObj};
use super::pac::{Pac, PacState};
use super::particle::{MAX_PARTICLES, Particle};
use super::status::Status;
use super::{AppState, Ctx, PState, Process, Scene, map_res_id};
use crate::input::dik;
use crate::math::{Vec3, distance};

pub struct World {
    pub process: Process,
    pub camera: Camera,
    pub map: Map,
    pub pac: Pac,
    pub enemies: Vec<Option<Enemy>>,
    pub map_objs: Vec<Option<MapObj>>,
    pub pac_fires: Vec<Option<PacFire>>,
    pub enemy_fires: Vec<Option<EnemyFire>>,
    pub particles: Vec<Option<Particle>>,
    pub particle_ptr: usize,
    pub crystals: Vec<Option<Crystal>>,
    pub taken_count: i32,
    pub crystal_left: i32,
    pub status: Status,
}

fn slots<T>(n: usize) -> Vec<Option<T>> {
    (0..n).map(|_| None).collect()
}

impl World {
    pub fn new(ctx: &mut Ctx) -> World {
        let mut w = World {
            process: Process::new(ctx),
            camera: Camera::default(),
            map: Map::build(&[], 40.0, 0x9c, true).0,
            pac: Pac::new(ctx),
            enemies: slots(MAX_ENEMIES),
            map_objs: slots(MAX_MAP_OBJECTS),
            pac_fires: slots(MAX_PAC_FIRES),
            enemy_fires: slots(MAX_ENEMY_FIRES),
            particles: slots(MAX_PARTICLES),
            particle_ptr: 0,
            crystals: slots(MAX_CRYSTALS),
            taken_count: 0,
            crystal_left: 0,
            status: Status::new(ctx),
        };
        w.init_level(ctx);
        w
    }

    /// `XGameObject::InitLevel`.
    pub fn init_level(&mut self, ctx: &mut Ctx) {
        let music = match ctx.app.world() {
            0 => 1,
            1 => 2,
            _ => 3,
        };
        ctx.audio.play_music(music);
        self.pac.reset();
        self.enemies = slots(MAX_ENEMIES);
        self.map_objs = slots(MAX_MAP_OBJECTS);
        self.crystals = slots(MAX_CRYSTALS);
        self.taken_count = 0;
        self.crystal_left = 0;
        self.pac_fires = slots(MAX_PAC_FIRES);
        self.enemy_fires = slots(MAX_ENEMY_FIRES);
        self.particles = slots(MAX_PARTICLES);
        let lines = ctx.assets.text_lines(map_res_id(ctx.app));
        let (map, spawns) = Map::build(&lines, 40.0, world_texture(ctx.app.world()), false);
        self.map = map;
        for s in spawns {
            match s {
                Spawn::Pac { x, z, mx, mz } => {
                    self.pac.obj.base.pos.x = x;
                    self.pac.obj.base.pos.z = z;
                    self.pac.mx = mx;
                    self.pac.mz = mz;
                }
                Spawn::Crystal { x, z } => self.crystals_create(ctx, x, z),
                Spawn::Enemy { kind, x, z, mx, mz } => self.enemy_create(ctx, kind, x, z, mx, mz),
                Spawn::MapObject { kind, x, z } => self.map_object_create(ctx, kind, x, z),
            }
        }
        self.camera.reset_for_game(self.pac.pos());
        self.process.start_fade_in(0);
    }

    /// `XGameObject::GetVolumeOfPosition`: positional attenuation for sounds.
    pub fn volume_of(&self, pos: Vec3) -> u8 {
        if self.pac.state == PacState::Non {
            return 0;
        }
        let d = distance(pos, self.pac.pos());
        if d < 130.0 {
            100
        } else if d <= 550.0 {
            (((550.0 - d) / 420.0) * 30.0) as i32 as u8 + 70
        } else {
            0
        }
    }
}

impl Scene for World {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32) {
        if self.process.state == PState::Non {
            return;
        }
        self.process.tick += ticks;
        match self.process.state {
            PState::FadeIn => {
                self.process.process_fade_in();
                if self.process.tick > 119 {
                    self.process.start_process(0);
                }
            }
            PState::FadeOut => {
                self.process.process_fade_out();
                if self.process.tick > 119 {
                    if self.process.process_type == 0 {
                        ctx.app.set_state(AppState::StartSelLevel);
                        self.process.state = PState::Non;
                        return;
                    }
                    ctx.app.current_level += 1;
                    if ctx.app.max_level < ctx.app.current_level {
                        ctx.app.current_level = 0;
                        ctx.app.set_state(AppState::StartEnding);
                        self.process.state = PState::Non;
                        return;
                    }
                    if ctx.app.max_sel_level < ctx.app.current_level {
                        ctx.app.max_sel_level = ctx.app.current_level;
                        ctx.app.save();
                    }
                    self.init_level(ctx);
                    return;
                }
            }
            PState::Process => {
                if self.process.process_type == 1 && self.process.tick > 300 {
                    self.process.start_fade_out(1);
                } else if self.process.process_type == 2 && self.process.tick > 300 {
                    self.process.start_fade_out(0);
                }
            }
            PState::Non => {}
        }

        self.pac.process_rotate(ticks);
        self.enemies_process_rotate(ticks);
        self.crystals_process_rotate(ticks);
        for _ in 0..ticks {
            self.enemies_check_collision(ctx);
            self.enemy_fire_check_collision(ctx);
            self.crystals_check_collision(ctx);
            self.pac_fire_check_collision(ctx);
            self.pac_process_frame(ctx);
            let pac_pos = self.pac.pos();
            self.camera.process_frame(pac_pos);
            self.crystals_process_frame();
            self.pac_fire_process_frame();
            self.enemy_fire_process_frame(ctx);
            self.particles_process_frame();
            self.enemies_process_frame(ctx);
            self.map_objects_process_frame(ctx);
        }

        let inp = &mut *ctx.input;
        inp.update();
        let up = inp.key(dik::UP) || inp.joy_dir(0);
        let right = inp.key(dik::RIGHT) || inp.joy_dir(1);
        let down = inp.key(dik::DOWN) || inp.joy_dir(2);
        let left = inp.key(dik::LEFT) || inp.joy_dir(3);
        let fire = inp.key_pressed(dik::LSHIFT)
            || inp.key_pressed(dik::RSHIFT)
            || inp.key_pressed(dik::Z)
            || inp.joy_button_pressed(0);
        let camera_mod = inp.key(dik::LCONTROL)
            || inp.key(dik::RCONTROL)
            || inp.key(dik::X)
            || inp.joy_button(1);
        let escape = inp.escape_pressed();

        let active = |s: PacState| matches!(s, PacState::Wait | PacState::Move | PacState::OnArrow);
        if active(self.pac.state) && escape {
            self.pac_start_dying(ctx);
        }
        if !active(self.pac.state) {
            return;
        }
        if fire && self.pac.fire_energy > 999 {
            self.pac_fire_create(ctx);
            self.pac.fire_energy = 0;
        }
        if camera_mod {
            if up {
                self.camera.incr_vertical(ticks);
            } else if down {
                self.camera.decr_vertical(ticks);
            }
        }
        if self.camera.state == CamState::Wait {
            if camera_mod {
                if right {
                    self.camera.start_rotating_left();
                } else if left {
                    self.camera.start_rotating_right();
                }
            } else {
                let rel = if up {
                    0
                } else if right {
                    1
                } else if down {
                    2
                } else if left {
                    3
                } else {
                    -1
                };
                if rel >= 0 {
                    let dir = (self.camera.view_dir + rel).rem_euclid(4);
                    if self.pac.state == PacState::Wait {
                        self.pac_start_moving(ctx, dir);
                    } else if self.pac.state == PacState::OnArrow {
                        self.pac.set_face(dir);
                    }
                }
            }
        }
    }

    fn render(&mut self, ctx: &mut Ctx) {
        if self.process.state == PState::Non {
            return;
        }
        let gfx = &mut *ctx.gfx;
        gfx.set_3d(self.camera.view);
        self.map.render(gfx);
        self.map_objects_render(gfx);
        for c in self.crystals.iter_mut().flatten() {
            c.render(gfx);
        }
        self.enemies_render(gfx);
        self.pac.render(gfx);
        self.particles_render(gfx, ctx.rng);
        let view = self.camera.view;
        self.enemy_fire_render(gfx, &view);
        self.pac_fire_render(gfx, &view, ctx.rng);
        gfx.set_2d();
        let (level, left, energy) = (
            ctx.app.current_level,
            self.crystal_left,
            self.pac.fire_energy,
        );
        self.status.draw(ctx, level, left, energy);
        self.process.draw_fade(ctx.gfx, ctx.assets);
    }
}

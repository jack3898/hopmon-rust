//! `XMapObjectList` and the static map objects (cannons, volcano, poles,
//! trees, steam vents).

use super::Ctx;
use super::world::World;
use crate::audio::sfx;
use crate::gfx::Gfx;
use crate::math::{Axis, HALF_PI, PI, v3};
use crate::obj3d::{Obj3D, ObjX};

pub const MAX_MAP_OBJECTS: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapObjKind {
    CannonUp,
    Cannon,
    Volcano,
    Steam,
    Pole,
    ShortPole,
    Tree,
}

pub struct MapObj {
    pub obj: ObjX,
    pub alive: bool,
    pub local_time: u32,
    pub dir: i32,
    pub shadow: Obj3D,
    pub kind: MapObjKind,
}

impl MapObj {
    /// `XMapObjectList::CreateMapObject`.
    pub fn create(ctx: &mut Ctx, c: u8) -> Option<MapObj> {
        let (kind, mesh, shadow, scale, dir) = match c {
            b'A' => (MapObjKind::CannonUp, Some(0xa6), 60.0, 80.0, -1),
            b'N' => (MapObjKind::Cannon, Some(0xac), 55.0, 80.0, 0),
            b'S' => (MapObjKind::Cannon, Some(0xac), 55.0, 80.0, 2),
            b'E' => (MapObjKind::Cannon, Some(0xac), 55.0, 80.0, 1),
            b'W' => (MapObjKind::Cannon, Some(0xac), 55.0, 80.0, 3),
            b'V' => (MapObjKind::Volcano, Some(0xb0), 0.0, 135.0, -1),
            b'!' => (MapObjKind::Steam, None, 0.0, 1.0, -1),
            b'P' => match ctx.app.world() {
                0 => (MapObjKind::Pole, Some(0xa7), 60.0, 120.0, -1),
                1 => (MapObjKind::Tree, Some(0xa8), 90.0, 130.0, -1),
                _ => (MapObjKind::ShortPole, Some(0x13b), 55.0, 145.0, -1),
            },
            _ => return None,
        };
        let mut m = MapObj {
            obj: ObjX::new(ctx.assets, mesh),
            alive: true,
            local_time: 0,
            dir: -1,
            shadow: Obj3D::shadow(1.0, 1.0),
            kind,
        };
        m.obj.rotate_by(Axis::X, -HALF_PI);
        m.shadow.scale_by(shadow);
        if kind == MapObjKind::Steam {
            m.obj.base.pos.y = 0.0;
        } else {
            m.obj.scale_by(scale);
            m.obj.on_ground();
            m.obj.base.pos.y += match kind {
                MapObjKind::Volcano => 2.0,
                _ => 3.0,
            };
        }
        if kind == MapObjKind::Cannon {
            let a = match dir {
                1 => HALF_PI,
                2 => PI,
                3 => 4.712_389,
                _ => 0.0,
            };
            m.obj.rotate_to(Axis::Y, a);
            m.dir = dir;
        }
        Some(m)
    }

    pub fn render(&mut self, gfx: &mut Gfx) {
        if !self.alive {
            return;
        }
        self.obj.render(gfx);
        self.shadow.base.pos = v3(self.obj.base.pos.x, 1.0, self.obj.base.pos.z);
        self.shadow.render(gfx);
    }
}

impl World {
    pub fn map_object_create(&mut self, ctx: &mut Ctx, c: u8, x: f32, z: f32) {
        if c == b'Z' {
            return;
        }
        let Some(slot) = self.map_objs.iter().position(|o| o.is_none()) else {
            return;
        };
        if let Some(mut m) = MapObj::create(ctx, c) {
            m.obj.base.pos.x = x;
            m.obj.base.pos.z = z;
            self.map_objs[slot] = Some(m);
        }
    }

    pub fn map_objects_process_frame(&mut self, ctx: &mut Ctx) {
        for i in 0..MAX_MAP_OBJECTS {
            let Some(mut m) = self.map_objs[i].take() else {
                continue;
            };
            m.local_time += 1;
            let pos = m.obj.base.pos;
            if m.alive {
                match m.kind {
                    MapObjKind::Volcano => {
                        if m.local_time > 1499 {
                            if m.local_time < 2500 {
                                if m.local_time.is_multiple_of(50) {
                                    self.enemy_fire_volcano(ctx, pos + v3(0.0, 20.0, 0.0));
                                }
                            } else {
                                m.local_time = 0;
                            }
                        }
                    }
                    MapObjKind::CannonUp => {
                        if m.local_time.is_multiple_of(200) {
                            self.enemy_fire_cannon_up(ctx, pos + v3(0.0, 10.0, 0.0));
                        }
                    }
                    MapObjKind::Cannon => {
                        if m.local_time.is_multiple_of(450) {
                            self.enemy_fire_cannon(ctx, pos, m.dir);
                        }
                    }
                    MapObjKind::Steam => {
                        if m.local_time == 200 {
                            let vol = self.volume_of(pos);
                            ctx.audio.play(sfx::STEAM, vol);
                        }
                        if m.local_time > 200 && m.local_time.is_multiple_of(10) {
                            self.enemy_fire_steam(pos);
                        }
                        if m.local_time > 400 {
                            m.local_time = 0;
                        }
                    }
                    _ => {}
                }
            }
            if m.alive {
                self.map_objs[i] = Some(m);
            }
        }
    }

    pub fn map_objects_render(&mut self, gfx: &mut Gfx) {
        for m in self.map_objs.iter_mut().flatten() {
            m.render(gfx);
        }
    }
}

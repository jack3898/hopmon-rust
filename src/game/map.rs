//! `XMap` / `XPreviewMap`: text level layout, collision queries and the
//! merged floor/wall mesh.
//!
//! Map legend (from `XMap::_CreateMapObject` and the `IsBlocked_*` checks):
//! * `.` void, `-` floor, `+` floor that blocks enemies, `#` `=` floor tiles
//! * `@` Hopmon start, `$` crystal, `*` warp zone (star)
//! * `[` `>` `]` `<` conveyor arrows (north/east/south/west)
//! * `1` `2` `3` walls of that height
//! * `A`..`Z` map objects (`A` up-cannon, `N`/`E`/`S`/`W` cannons, `V`
//!   volcano, `P` pole/tree), `!` steam vent on floor
//! * `a`..`z` enemies (see `XEnemyList::CreateEnemy`)

use crate::gfx::Gfx;
use crate::math::{Axis, HALF_PI, PI, v3};
use crate::obj3d::{Filter, Obj3D};

pub enum Spawn {
    Pac {
        x: f32,
        z: f32,
        mx: i32,
        mz: i32,
    },
    Crystal {
        x: f32,
        z: f32,
    },
    Enemy {
        kind: u8,
        x: f32,
        z: f32,
        mx: i32,
        mz: i32,
    },
    MapObject {
        kind: u8,
        x: f32,
        z: f32,
    },
}

pub struct Map {
    pub map: Vec<u8>,
    pub enemy_map: Vec<u8>,
    pub ufo_map: Vec<u8>,
    pub size_x: i32,
    pub size_z: i32,
    pub chip: f32,
    pub texture: u16,
    pub mesh: Obj3D,
    pub background: Option<Obj3D>,
}

/// World texture for each world (`mapTextureId`).
pub fn world_texture(world: i32) -> u16 {
    match world {
        0 => 0x9c,
        1 => 0xfb,
        _ => 0xfc,
    }
}

fn convert_item(c: u8) -> i8 {
    if (b'1'..=b'3').contains(&c) {
        (c - b'0') as i8
    } else {
        0
    }
}

impl Map {
    /// `_LoadMapArray` + `_CreateMapObject` (+ `_CreateBackground` for the
    /// in-game map). The preview variant skips entity spawning.
    pub fn build(lines: &[String], chip: f32, texture: u16, preview: bool) -> (Map, Vec<Spawn>) {
        let mut rows: Vec<&[u8]> = Vec::new();
        for l in lines {
            if l.is_empty() || l.starts_with('_') {
                break;
            }
            rows.push(l.as_bytes());
        }
        let size_x = rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32 + 2;
        let size_z = rows.len() as i32 + 2;
        let n = (size_x * size_z) as usize;
        let mut map = vec![b'.'; n];
        for z in 1..size_z - 1 {
            for x in 1..size_x - 1 {
                let row = rows[(z - 1) as usize];
                if let Some(&c) = row.get((x - 1) as usize) {
                    map[(z * size_x + x) as usize] = c;
                }
            }
        }
        let mut m = Map {
            map,
            enemy_map: vec![b'0'; n],
            ufo_map: vec![b'0'; n],
            size_x,
            size_z,
            chip,
            texture,
            mesh: Obj3D::new(0, 0),
            background: None,
        };
        let spawns = m.create_objects(preview);
        if !preview {
            m.create_background();
        }
        (m, spawns)
    }

    fn create_objects(&mut self, preview: bool) -> Vec<Spawn> {
        let half_x = self.size_x as f32 * self.chip / 2.0;
        let half_z = self.size_z as f32 * self.chip / 2.0;
        let mut parts: Vec<Obj3D> = Vec::new();
        let mut spawns = Vec::new();
        let sx = self.size_x;
        for z in 0..self.size_z {
            for x in 0..self.size_x {
                let i = (z * sx + x) as usize;
                let px = x as f32 * self.chip + self.chip / 2.0 - half_x;
                let pz = (self.size_z - z) as f32 * self.chip - self.chip / 2.0 - half_z;
                let c = self.map[i];
                if c == b'.' {
                    continue;
                }
                if c == b'@' {
                    if !preview {
                        spawns.push(Spawn::Pac {
                            x: px,
                            z: pz,
                            mx: x,
                            mz: z,
                        });
                    }
                    self.map[i] = b'-';
                    parts.push(self.floor(b'-', px, pz));
                } else if c == b'$' {
                    if !preview {
                        spawns.push(Spawn::Crystal { x: px, z: pz });
                    }
                    self.map[i] = b'-';
                    parts.push(self.floor(b'-', px, pz));
                } else if c.is_ascii_uppercase() {
                    if !preview {
                        spawns.push(Spawn::MapObject {
                            kind: c,
                            x: px,
                            z: pz,
                        });
                    }
                    parts.push(self.floor(b'-', px, pz));
                    if c == b'V' && !preview {
                        for (dx, dz) in [
                            (-1, -1),
                            (0, -1),
                            (1, -1),
                            (-1, 0),
                            (1, 0),
                            (-1, 1),
                            (0, 1),
                            (1, 1),
                        ] {
                            let j = ((z + dz) * sx + x + dx) as usize;
                            if j < self.map.len() {
                                self.map[j] = b'Z';
                            }
                        }
                    }
                } else if c.is_ascii_lowercase() {
                    if !preview {
                        spawns.push(Spawn::Enemy {
                            kind: c,
                            x: px,
                            z: pz,
                            mx: x,
                            mz: z,
                        });
                        if c == b'd' {
                            self.ufo_map[i] = c;
                        } else {
                            self.enemy_map[i] = c;
                        }
                    }
                    self.map[i] = b'-';
                    parts.push(self.floor(b'-', px, pz));
                } else if b"-+![]><#=*".contains(&c) {
                    parts.push(self.floor(c, px, pz));
                    if c == b'!' && !preview {
                        spawns.push(Spawn::MapObject {
                            kind: c,
                            x: px,
                            z: pz,
                        });
                    }
                } else if (b'1'..=b'3').contains(&c) {
                    let levels = (c - b'0') as i32;
                    for lv in 1..=levels {
                        self.walls(&mut parts, lv, x, z, px, pz);
                    }
                    let mut top = Obj3D::rect(self.chip, self.chip);
                    top.set_rect_texture(self.texture, 0.25, 0.0, 0.5, 0.25);
                    top.shift(v3(px, self.chip * levels as f32, pz));
                    parts.push(top);
                }
            }
        }
        // Merge into one mesh like the original.
        let mut mesh = Obj3D::new(0, 0);
        for p in parts {
            let base = mesh.verts.len() as u16;
            mesh.idx.extend(p.idx.iter().map(|i| i + base));
            mesh.verts.extend(p.verts);
        }
        mesh.tex = self.texture;
        self.mesh = mesh;
        spawns
    }

    /// `_CreateFloor`: one tile from the 4x4 texture atlas.
    fn floor(&self, c: u8, x: f32, z: f32) -> Obj3D {
        let mut o = Obj3D::rect(self.chip, self.chip);
        let uv = match c {
            b'!' => (0.75, 0.25, 1.0, 0.5),
            b'[' => (0.0, 0.5, 0.25, 0.75),
            b'>' => (0.25, 0.5, 0.5, 0.75),
            b']' => (0.5, 0.5, 0.75, 0.75),
            b'<' => (0.75, 0.5, 1.0, 0.75),
            b'#' => (0.0, 0.25, 0.25, 0.5),
            b'=' => (0.25, 0.25, 0.5, 0.5),
            b'*' => (0.5, 0.25, 0.75, 0.5),
            _ => (0.0, 0.0, 0.25, 0.25),
        };
        o.set_rect_texture(self.texture, uv.0, uv.1, uv.2, uv.3);
        o.shift(v3(x, 0.0, z));
        o
    }

    /// `_CreateWalls`: side faces for wall level `lv` where the neighbour is
    /// lower.
    fn walls(&self, parts: &mut Vec<Obj3D>, lv: i32, mx: i32, mz: i32, x: f32, z: f32) {
        let c = self.chip;
        let y = (lv - 1) as f32 * c + c / 2.0;
        let at = |dx: i32, dz: i32| -> i8 {
            let j = ((mz + dz) * self.size_x + mx + dx) as usize;
            convert_item(self.map.get(j).copied().unwrap_or(b'.')) - lv as i8
        };
        let (w, e, n, s) = (at(-1, 0), at(1, 0), at(0, -1), at(0, 1));
        if w < 0 {
            let mut o = Obj3D::rect(c, c);
            o.set_rect_texture(self.texture, 0.5, 0.0, 0.75, 0.25);
            o.rotate_by(Axis::Y, HALF_PI);
            o.rotate_by(Axis::Z, HALF_PI);
            o.shift(v3(x - c / 2.0, y, z));
            parts.push(o);
        }
        if e < 0 {
            let mut o = Obj3D::rect(c, c);
            o.set_rect_texture(self.texture, 0.75, 0.0, 1.0, 0.25);
            o.rotate_by(Axis::Y, -HALF_PI);
            o.rotate_by(Axis::Z, -HALF_PI);
            o.shift(v3(c / 2.0 + x, y, z));
            parts.push(o);
        }
        if n < 0 {
            let mut o = Obj3D::rect(c, c);
            o.set_rect_texture(self.texture, 0.75, 0.0, 1.0, 0.25);
            o.rotate_by(Axis::Y, PI);
            o.rotate_by(Axis::X, HALF_PI);
            o.shift(v3(x, y, c / 2.0 + z));
            parts.push(o);
        }
        if s < 0 {
            let mut o = Obj3D::rect(c, c);
            o.set_rect_texture(self.texture, 0.5, 0.0, 0.75, 0.25);
            o.rotate_by(Axis::X, -HALF_PI);
            o.shift(v3(x, y, z - c / 2.0));
            parts.push(o);
        }
    }

    /// `_CreateBackground`: big scrolling-texture plane below the level.
    fn create_background(&mut self) {
        let w = (self.size_x + 4) as f32 * self.chip;
        let h = (self.size_z + 4) as f32 * self.chip;
        let mut bg = Obj3D::rect(w, h);
        bg.base.pos.y -= 100.0;
        bg.set_rect_texture(
            0xc1,
            0.0,
            0.0,
            (self.size_x + 4) as f32,
            (self.size_z + 4) as f32,
        );
        bg.filter = Filter::Linear;
        self.background = Some(bg);
    }

    pub fn render(&self, gfx: &mut Gfx) {
        if let Some(bg) = &self.background {
            bg.render(gfx);
        }
        self.mesh.render(gfx);
    }

    fn outside(&self, x: i32, z: i32) -> bool {
        x < 0 || x >= self.size_x || z < 0 || z >= self.size_z
    }

    fn at(&self, x: i32, z: i32) -> u8 {
        self.map[(z * self.size_x + x) as usize]
    }

    fn idx(&self, x: i32, z: i32) -> usize {
        (z * self.size_x + x) as usize
    }

    pub fn is_blocked_pac(&self, x: i32, z: i32) -> bool {
        if self.outside(x, z) {
            return true;
        }
        let c = self.at(x, z);
        let e = self.enemy_map[self.idx(x, z)];
        c == b'.' || c.is_ascii_uppercase() || (b'1'..=b'3').contains(&c) || e == b'x' || e == b'y'
    }

    pub fn is_blocked_fire(&self, x: i32, z: i32) -> bool {
        if self.outside(x, z) {
            return true;
        }
        let c = self.at(x, z);
        c.is_ascii_uppercase() || (b'1'..=b'3').contains(&c)
    }

    pub fn is_blocked_enemy(&self, x: i32, z: i32) -> bool {
        if self.outside(x, z) {
            return true;
        }
        let c = self.at(x, z);
        if c == b'.' || c == b'+' || c.is_ascii_uppercase() || (b'1'..=b'3').contains(&c) {
            return true;
        }
        self.enemy_map[self.idx(x, z)] != b'0'
    }

    pub fn is_blocked_ufo(&self, x: i32, z: i32) -> bool {
        if self.outside(x, z) {
            return true;
        }
        let c = self.at(x, z);
        if c == b'.' || c == b'P' || c == b'3' {
            return true;
        }
        self.ufo_map[self.idx(x, z)] != b'0'
    }

    pub fn is_warp_zone(&self, x: i32, z: i32) -> bool {
        !self.outside(x, z) && self.at(x, z) == b'*'
    }

    pub fn arrow_direction(&self, x: i32, z: i32) -> i32 {
        if self.outside(x, z) {
            return -1;
        }
        match self.at(x, z) {
            b'[' => 0,
            b'>' => 1,
            b']' => 2,
            b'<' => 3,
            _ => -1,
        }
    }

    pub fn is_arrow(&self, x: i32, z: i32) -> bool {
        self.arrow_direction(x, z) >= 0
    }

    pub fn pos_x_to_map(&self, x: f32) -> i32 {
        ((x + self.size_x as f32 * self.chip / 2.0) / self.chip) as i32
    }

    pub fn pos_z_to_map(&self, z: f32) -> i32 {
        (self.size_z - 1) - ((z + self.size_z as f32 * self.chip / 2.0) / self.chip) as i32
    }

    pub fn update_enemy_map(&mut self, x0: i32, z0: i32, x1: i32, z1: i32) {
        let (a, b) = (self.idx(x0, z0), self.idx(x1, z1));
        let v = self.enemy_map[a];
        self.enemy_map[a] = b'0';
        self.enemy_map[b] = v;
    }

    pub fn zero_enemy_map(&mut self, x: i32, z: i32) {
        let i = self.idx(x, z);
        self.enemy_map[i] = b'0';
    }

    pub fn update_ufo_map(&mut self, x0: i32, z0: i32, x1: i32, z1: i32) {
        let (a, b) = (self.idx(x0, z0), self.idx(x1, z1));
        let v = self.ufo_map[a];
        self.ufo_map[a] = b'0';
        self.ufo_map[b] = v;
    }
}

/// Helper so callers can step one cell in a direction.
pub fn step(x: i32, z: i32, dir: i32) -> (i32, i32) {
    match dir {
        0 => (x, z - 1),
        1 => (x + 1, z),
        2 => (x, z + 1),
        3 => (x - 1, z),
        _ => (x, z),
    }
}

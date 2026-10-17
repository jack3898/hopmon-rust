//! Ports of the `JObject3D*` engine classes.
//!
//! The original code transforms vertex data in place for every `RotateBy`,
//! `ScaleBy` and `Shift`. [`ObjX`] keeps an accumulated affine matrix instead
//! (mathematically identical, including operation order), while [`Obj3D`]
//! keeps mutating its small vertex arrays like the original did.

use std::rc::Rc;

use crate::assets::Assets;
use crate::assets::xfile::XMesh;
use crate::gfx::{Blend, Gfx, LVertex};
use crate::math::{Axis, Mat, Vec2, Vec3, distance, v3};

/// `JObject3DBase` state.
#[derive(Clone, Copy, Debug)]
pub struct Base {
    pub pos: Vec3,
    pub radius: f32,
    pub rot: [f32; 3],
    pub scaled: f32,
}

impl Default for Base {
    fn default() -> Self {
        Base {
            pos: Vec3::ZERO,
            radius: 0.0,
            rot: [0.0; 3],
            scaled: 1.0,
        }
    }
}

impl Base {
    /// `CheckCollisionRadius`.
    pub fn collides(&self, other: &Base) -> bool {
        distance(self.pos, other.pos) < self.radius + other.radius
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaState {
    None,
    Particle,
    Shadow,
    Transparent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    None,
    Point,
    Linear,
}

/// `JObject3D`: pre-lit (unlit) vertex mesh, typically a textured quad.
#[derive(Clone)]
pub struct Obj3D {
    pub base: Base,
    pub verts: Vec<LVertex>,
    pub idx: Vec<u16>,
    pub tex: u16,
    pub alpha: AlphaState,
    pub filter: Filter,
}

impl Obj3D {
    pub fn new(nverts: usize, nidx: usize) -> Obj3D {
        Obj3D {
            base: Base::default(),
            verts: vec![
                LVertex {
                    pos: Vec3::ZERO,
                    color: [255, 255, 255, 255],
                    uv: Vec2::ZERO
                };
                nverts
            ],
            idx: vec![0; nidx],
            tex: 0,
            alpha: AlphaState::None,
            filter: Filter::None,
        }
    }

    /// `JObject3D_Rect`: a w x h quad in the XZ plane.
    pub fn rect(w: f32, h: f32) -> Obj3D {
        let mut o = Obj3D::new(4, 6);
        let (hw, hh) = (w / 2.0, h / 2.0);
        o.verts[0].pos = v3(-hw, 0.0, hh);
        o.verts[1].pos = v3(hw, 0.0, hh);
        o.verts[2].pos = v3(-hw, 0.0, -hh);
        o.verts[3].pos = v3(hw, 0.0, -hh);
        o.idx.copy_from_slice(&[0, 1, 3, 0, 3, 2]);
        o
    }

    /// `JObject3D_Shadow`.
    pub fn shadow(w: f32, h: f32) -> Obj3D {
        let mut o = Obj3D::rect(w, h);
        o.alpha = AlphaState::Shadow;
        o.filter = Filter::Linear;
        o.set_rect_texture(0xa3, 0.0, 0.0, 1.0, 1.0);
        o
    }

    /// `JObject3D_Particle`: a camera facing quad with additive blending.
    pub fn particle(w: f32, h: f32) -> Obj3D {
        let mut o = Obj3D::rect(w, h);
        o.rotate_by(Axis::X, -crate::math::HALF_PI);
        o.alpha = AlphaState::Particle;
        o.filter = Filter::Linear;
        o
    }

    fn apply(&mut self, m: &Mat) {
        for v in &mut self.verts {
            v.pos = m.transform_point(v.pos);
        }
    }

    pub fn shift(&mut self, d: Vec3) {
        self.apply(&Mat::translate(d));
    }

    pub fn scale_by(&mut self, s: f32) {
        self.base.scaled *= s;
        self.apply(&Mat::scale(s));
    }

    pub fn rotate_by(&mut self, axis: Axis, a: f32) {
        self.base.rot[axis as usize] += a;
        self.apply(&Mat::rot_axis(axis, a));
    }

    pub fn scale_to(&mut self, s: f32) {
        let inv = 1.0 / self.base.scaled;
        self.scale_by(inv);
        self.scale_by(s);
    }

    /// `JObject3D_Rect::SetTexture` (UVs inset by 0.0001 like the original).
    pub fn set_rect_texture(&mut self, id: u16, u0: f32, v0: f32, u1: f32, v1: f32) {
        self.tex = id;
        let (a, b, c, d) = (u0 + 0.0001, v0 + 0.0001, u1 - 0.0001, v1 - 0.0001);
        self.verts[0].uv = Vec2::new(a, b);
        self.verts[1].uv = Vec2::new(c, b);
        self.verts[2].uv = Vec2::new(a, d);
        self.verts[3].uv = Vec2::new(c, d);
    }

    pub fn set_alpha(&mut self, a: u8) {
        for v in &mut self.verts {
            v.color[3] = a;
        }
    }

    /// `SetAlphaFactor`.
    pub fn set_alpha_factor(&mut self, f: f32) {
        self.set_alpha(crate::math::round(f * 255.0).clamp(0, 255) as u8);
    }

    fn blend(&self) -> Blend {
        match self.alpha {
            AlphaState::None => Blend::Opaque,
            AlphaState::Particle => Blend::Additive,
            AlphaState::Shadow => Blend::Shadow,
            AlphaState::Transparent => Blend::Transparent,
        }
    }

    fn draw_with(&self, gfx: &mut Gfx, world: &Mat, tex: u16) {
        let verts: Vec<LVertex> = self
            .verts
            .iter()
            .map(|v| LVertex {
                pos: world.transform_point(v.pos),
                ..*v
            })
            .collect();
        gfx.draw_tris(
            self.blend(),
            tex,
            self.filter == Filter::Linear,
            &verts,
            &self.idx,
        );
    }

    /// `JObject3D::Render`: translated by the world position.
    pub fn render(&self, gfx: &mut Gfx) {
        self.draw_with(gfx, &Mat::translate(self.base.pos), self.tex);
    }

    /// `JObject3D_Rect::_SetWorldTransform2`: inverse view rotation (billboard)
    /// with the world position as translation.
    pub fn billboard_matrix(&self, view: &Mat) -> Mat {
        let mut m = view.invert_affine();
        m.0[3][0] = self.base.pos.x;
        m.0[3][1] = self.base.pos.y;
        m.0[3][2] = self.base.pos.z;
        m
    }

    /// `JObject3D_Rect::Render2`.
    pub fn render2(&self, gfx: &mut Gfx, view: &Mat) {
        let m = self.billboard_matrix(view);
        self.draw_with(gfx, &m, self.tex);
    }

    /// Draws with a given texture/UV set under an existing billboard matrix
    /// (used by the multi-layer fire sprites).
    pub fn render2_layer(&mut self, gfx: &mut Gfx, m: &Mat, id: u16, uv: (f32, f32, f32, f32)) {
        self.set_rect_texture(id, uv.0, uv.1, uv.2, uv.3);
        self.draw_with(gfx, m, id);
    }
}

// ---------------------------------------------------------------------------

/// Device lighting from `XApplication::InitDeviceObjects`.
pub const AMBIENT: f32 = 0x89 as f32 / 255.0;
pub const LIGHT_DIFFUSE: f32 = 0.77;

fn light_dir() -> Vec3 {
    // Direction the light travels; lighting uses the reversed vector.
    -v3(0.5, -1.0, 0.5).normalize()
}

/// `JObject3DX`: lit material meshes with up to 10 animation frames.
#[derive(Clone)]
pub struct ObjX {
    pub base: Base,
    pub frames: Vec<Rc<XMesh>>,
    pub frame: usize,
    pub flat: bool,
    /// Accumulated vertex transform (rotate, scale, shift).
    pub xf: Mat,
    /// Accumulated normal transform (rotations only).
    pub nf: Mat,
}

impl ObjX {
    pub fn new(assets: &Assets, id: Option<u16>) -> ObjX {
        ObjX {
            base: Base::default(),
            frames: id.map(|i| vec![assets.mesh(i)]).unwrap_or_default(),
            frame: 0,
            flat: false,
            xf: Mat::IDENTITY,
            nf: Mat::IDENTITY,
        }
    }

    pub fn append_frame(&mut self, assets: &Assets, id: u16) {
        if self.frames.len() < 10 {
            self.frames.push(assets.mesh(id));
        }
    }

    pub fn scale_by(&mut self, s: f32) {
        self.base.scaled *= s;
        self.xf = self.xf.mul(&Mat::scale(s));
    }

    pub fn rotate_by(&mut self, axis: Axis, a: f32) {
        self.base.rot[axis as usize] += a;
        let r = Mat::rot_axis(axis, a);
        self.xf = self.xf.mul(&r);
        self.nf = self.nf.mul(&r);
    }

    pub fn rotate_to(&mut self, axis: Axis, a: f32) {
        let cur = self.base.rot[axis as usize];
        self.rotate_by(axis, -cur);
        self.rotate_by(axis, a);
    }

    pub fn scale_to(&mut self, s: f32) {
        let inv = 1.0 / self.base.scaled;
        self.scale_by(inv);
        self.scale_by(s);
    }

    /// `CD3DFile::GetBoxMin/Max` over frame 0.
    pub fn get_box(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::ZERO;
        if let Some(m) = self.frames.first() {
            for p in &m.positions {
                let q = self.xf.transform_point(*p);
                mn = mn.min(q);
                mx = mx.max(q);
            }
        } else {
            mn = Vec3::ZERO;
        }
        (mn, mx)
    }

    pub fn on_ground(&mut self) {
        self.base.pos.y = -self.get_box().0.y;
    }

    /// `JObject3DX::Render` with D3D7 fixed-function vertex lighting.
    pub fn render(&self, gfx: &mut Gfx) {
        let Some(mesh) = self.frames.get(self.frame) else {
            return;
        };
        let world = self.xf.mul(&Mat::translate(self.base.pos));
        let l = light_dir();
        let pos: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|p| world.transform_point(*p))
            .collect();
        let nrm: Vec<Vec3> = mesh
            .normals
            .iter()
            .map(|n| self.nf.transform_dir(*n))
            .collect();
        let blend = if mesh.has_alpha {
            Blend::AlphaWrite
        } else {
            Blend::Opaque
        };
        for (mat, idx) in &mesh.groups {
            if idx.is_empty() {
                continue;
            }
            let shade = |n: Vec3| -> [u8; 4] {
                let d = n.dot(l).max(0.0) * LIGHT_DIFFUSE;
                let c = |i: usize| {
                    let v = mat.emissive[i] + mat.diffuse[i] * (AMBIENT + d);
                    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                };
                [
                    c(0),
                    c(1),
                    c(2),
                    (mat.diffuse[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                ]
            };
            let (verts, indices): (Vec<LVertex>, Vec<u16>) = if self.flat {
                // D3DSHADE_FLAT: the first vertex of each triangle sets the colour.
                let mut vs = Vec::with_capacity(idx.len());
                for t in idx.chunks_exact(3) {
                    let col = shade(nrm[t[0] as usize]);
                    for &i in t {
                        vs.push(LVertex {
                            pos: pos[i as usize],
                            color: col,
                            uv: Vec2::ZERO,
                        });
                    }
                }
                let n = vs.len() as u16;
                (vs, (0..n).collect())
            } else {
                let vs = pos
                    .iter()
                    .zip(&nrm)
                    .map(|(p, n)| LVertex {
                        pos: *p,
                        color: shade(*n),
                        uv: Vec2::ZERO,
                    })
                    .collect();
                (vs, idx.clone())
            };
            gfx.draw_tris(blend, 0, false, &verts, &indices);
        }
    }
}

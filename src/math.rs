//! Direct3D 7 style math: left-handed coordinates, row vectors (`v * M`).
//!
//! The original game mutates vertex data with these exact matrix helpers
//! (`D3DUtil_SetRotateXMatrix` and friends), so they are mirrored here to keep
//! the same orientation conventions.

pub use macroquad::math::{Vec2, Vec3};

pub fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat(pub [[f32; 4]; 4]);

impl Default for Mat {
    fn default() -> Self {
        Mat::IDENTITY
    }
}

impl Mat {
    pub const IDENTITY: Mat = Mat([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    pub fn translate(t: Vec3) -> Mat {
        let mut m = Mat::IDENTITY;
        m.0[3][0] = t.x;
        m.0[3][1] = t.y;
        m.0[3][2] = t.z;
        m
    }

    pub fn scale(s: f32) -> Mat {
        let mut m = Mat::IDENTITY;
        m.0[0][0] = s;
        m.0[1][1] = s;
        m.0[2][2] = s;
        m
    }

    pub fn rot_x(a: f32) -> Mat {
        let (s, c) = a.sin_cos();
        let mut m = Mat::IDENTITY;
        m.0[1][1] = c;
        m.0[1][2] = s;
        m.0[2][1] = -s;
        m.0[2][2] = c;
        m
    }

    pub fn rot_y(a: f32) -> Mat {
        let (s, c) = a.sin_cos();
        let mut m = Mat::IDENTITY;
        m.0[0][0] = c;
        m.0[0][2] = -s;
        m.0[2][0] = s;
        m.0[2][2] = c;
        m
    }

    pub fn rot_z(a: f32) -> Mat {
        let (s, c) = a.sin_cos();
        let mut m = Mat::IDENTITY;
        m.0[0][0] = c;
        m.0[0][1] = s;
        m.0[1][0] = -s;
        m.0[1][1] = c;
        m
    }

    pub fn rot_axis(axis: Axis, a: f32) -> Mat {
        match axis {
            Axis::X => Mat::rot_x(a),
            Axis::Y => Mat::rot_y(a),
            Axis::Z => Mat::rot_z(a),
        }
    }

    pub fn mul(&self, b: &Mat) -> Mat {
        let mut r = [[0.0f32; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| self.0[i][k] * b.0[k][j]).sum();
            }
        }
        Mat(r)
    }

    /// `D3DMath_VectorMatrixMultiply`: point transform with w divide.
    pub fn transform_point(&self, v: Vec3) -> Vec3 {
        let m = &self.0;
        let x = v.x * m[0][0] + v.y * m[1][0] + v.z * m[2][0] + m[3][0];
        let y = v.x * m[0][1] + v.y * m[1][1] + v.z * m[2][1] + m[3][1];
        let z = v.x * m[0][2] + v.y * m[1][2] + v.z * m[2][2] + m[3][2];
        let w = v.x * m[0][3] + v.y * m[1][3] + v.z * m[2][3] + m[3][3];
        if w.abs() < 1e-30 {
            v3(x, y, z)
        } else {
            v3(x / w, y / w, z / w)
        }
    }

    pub fn transform_dir(&self, v: Vec3) -> Vec3 {
        let m = &self.0;
        v3(
            v.x * m[0][0] + v.y * m[1][0] + v.z * m[2][0],
            v.x * m[0][1] + v.y * m[1][1] + v.z * m[2][1],
            v.x * m[0][2] + v.y * m[1][2] + v.z * m[2][2],
        )
    }

    /// `D3DMath_MatrixInvert` (affine matrices only, like the original).
    pub fn invert_affine(&self) -> Mat {
        let a = &self.0;
        let f_det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        let f = if f_det.abs() < 1e-20 {
            0.0
        } else {
            1.0 / f_det
        };
        let mut q = Mat::IDENTITY;
        let r = &mut q.0;
        r[0][0] = f * (a[1][1] * a[2][2] - a[1][2] * a[2][1]);
        r[0][1] = -f * (a[0][1] * a[2][2] - a[0][2] * a[2][1]);
        r[0][2] = f * (a[0][1] * a[1][2] - a[0][2] * a[1][1]);
        r[1][0] = -f * (a[1][0] * a[2][2] - a[1][2] * a[2][0]);
        r[1][1] = f * (a[0][0] * a[2][2] - a[0][2] * a[2][0]);
        r[1][2] = -f * (a[0][0] * a[1][2] - a[0][2] * a[1][0]);
        r[2][0] = f * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        r[2][1] = -f * (a[0][0] * a[2][1] - a[0][1] * a[2][0]);
        r[2][2] = f * (a[0][0] * a[1][1] - a[0][1] * a[1][0]);
        r[3][0] = -(a[3][0] * r[0][0] + a[3][1] * r[1][0] + a[3][2] * r[2][0]);
        r[3][1] = -(a[3][0] * r[0][1] + a[3][1] * r[1][1] + a[3][2] * r[2][1]);
        r[3][2] = -(a[3][0] * r[0][2] + a[3][1] * r[1][2] + a[3][2] * r[2][2]);
        q
    }

    /// `D3DUtil_SetViewMatrix` (left-handed look-at).
    pub fn look_at(from: Vec3, at: Vec3, world_up: Vec3) -> Mat {
        let mut view = at - from;
        let len = view.length();
        if len < 1e-6 {
            return Mat::IDENTITY;
        }
        view /= len;
        let dot = world_up.dot(view);
        let mut up = world_up - view * dot;
        let mut l = up.length();
        if l < 1e-6 {
            up = v3(0.0, 1.0, 0.0) - view * view.y;
            l = up.length();
            if l < 1e-6 {
                up = v3(0.0, 0.0, 1.0) - view * view.z;
                l = up.length();
                if l < 1e-6 {
                    return Mat::IDENTITY;
                }
            }
        }
        up /= l;
        let right = up.cross(view);
        let mut m = Mat::IDENTITY;
        m.0[0][0] = right.x;
        m.0[0][1] = up.x;
        m.0[0][2] = view.x;
        m.0[1][0] = right.y;
        m.0[1][1] = up.y;
        m.0[1][2] = view.y;
        m.0[2][0] = right.z;
        m.0[2][1] = up.z;
        m.0[2][2] = view.z;
        m.0[3][0] = -from.dot(right);
        m.0[3][1] = -from.dot(up);
        m.0[3][2] = -from.dot(view);
        m
    }

    /// `D3DUtil_SetProjectionMatrix`.
    pub fn projection(fov: f32, aspect: f32, near: f32, far: f32) -> Mat {
        let c = (fov / 2.0).cos() / (fov / 2.0).sin();
        let q = far / (far - near);
        let mut m = Mat([[0.0; 4]; 4]);
        m.0[0][0] = c * aspect;
        m.0[1][1] = c;
        m.0[2][2] = q;
        m.0[2][3] = 1.0;
        m.0[3][2] = -q * near;
        m
    }

    /// Converts a D3D `world*view*proj` matrix into an OpenGL column-major
    /// clip matrix (z remapped from [0,1] to [-1,1]).
    pub fn to_gl(self) -> macroquad::math::Mat4 {
        // Row-vector clip coords: z_gl = 2 z - w, i.e. column 2 = 2*col2 - col3.
        let mut m = self.0;
        for row in m.iter_mut() {
            row[2] = 2.0 * row[2] - row[3];
        }
        // Rows of a row-vector matrix are the columns of the equivalent
        // column-vector matrix.
        let flat: Vec<f32> = m.iter().flatten().copied().collect();
        macroquad::math::Mat4::from_cols_slice(&flat)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}

/// `GetVectorFromDir`: 0 = +z (north), 1 = +x, 2 = -z, 3 = -x.
pub fn dir_vector(dir: i32) -> Vec3 {
    match dir {
        0 => v3(0.0, 0.0, 1.0),
        1 => v3(1.0, 0.0, 0.0),
        2 => v3(0.0, 0.0, -1.0),
        3 => v3(-1.0, 0.0, 0.0),
        _ => Vec3::ZERO,
    }
}

/// `D3DFindDistance`.
pub fn distance(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

/// `Round` from the original code (round half away from zero).
pub fn round(f: f32) -> i32 {
    if f < 0.0 {
        (f - 0.5) as i32
    } else {
        (f + 0.5) as i32
    }
}

pub const PI: f32 = std::f32::consts::PI;
pub const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

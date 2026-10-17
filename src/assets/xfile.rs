//! Parser for the text DirectX `.x` meshes embedded in the executable.
//!
//! Mirrors the DirectX 7 SDK `CD3DFile` loader used by the game: faces are
//! fan-triangulated, faces are grouped by material, per-vertex normals are
//! used only when their count matches the vertex count and are otherwise
//! recomputed by averaging face normals (`CD3DFileObject::ComputeNormals`).

use crate::math::{Vec2, Vec3, v3};

#[derive(Clone, Copy, Debug)]
pub struct Material {
    /// Diffuse (and ambient) RGBA.
    pub diffuse: [f32; 4],
    // Specular highlights are disabled in the original's render states
    // (D3DRENDERSTATE_SPECULARENABLE stays FALSE), so these are unused.
    #[allow(dead_code)]
    pub power: f32,
    #[allow(dead_code)]
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
}

impl Default for Material {
    fn default() -> Self {
        // `CD3DFileObject` zeroes its materials.
        Material {
            diffuse: [0.0; 4],
            power: 0.0,
            specular: [0.0; 3],
            emissive: [0.0; 3],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct XMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// Triangle index lists, one per material slot.
    pub groups: Vec<(Material, Vec<u16>)>,
    /// True when any material is translucent; the whole mesh is then drawn in
    /// the alpha-blended pass.
    pub has_alpha: bool,
}

#[derive(Debug)]
enum Tok {
    Ident(String),
    Num(f64),
    Str,
    Open,
    Close,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() || c == b';' || c == b',' {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') || c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'{' {
            out.push(Tok::Open);
            i += 1;
        } else if c == b'}' {
            out.push(Tok::Close);
            i += 1;
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
            i += 1;
            out.push(Tok::Str);
        } else if c == b'-' || c == b'.' || c.is_ascii_digit() {
            let s = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_digit() || b"eE.+-".contains(&b[i])) {
                i += 1;
            }
            out.push(Tok::Num(src[s..i].parse().unwrap_or(0.0)));
        } else {
            let s = i;
            while i < b.len() && !b[i].is_ascii_whitespace() && !b";,{}\"".contains(&b[i]) {
                i += 1;
            }
            out.push(Tok::Ident(src[s..i].to_string()));
        }
    }
    out
}

#[derive(Debug, Default)]
struct Node {
    kind: String,
    nums: Vec<f64>,
    children: Vec<Node>,
}

fn parse_nodes(toks: &[Tok], pos: &mut usize, until_close: bool) -> Vec<Node> {
    let mut nodes = Vec::new();
    while *pos < toks.len() {
        match &toks[*pos] {
            Tok::Close => {
                *pos += 1;
                if until_close {
                    break;
                }
            }
            Tok::Ident(kind) => {
                let kind = kind.clone();
                *pos += 1;
                if let Some(Tok::Ident(_)) = toks.get(*pos) {
                    *pos += 1; // object name
                }
                if let Some(Tok::Open) = toks.get(*pos) {
                    *pos += 1;
                    let mut node = Node {
                        kind,
                        ..Default::default()
                    };
                    parse_body(toks, pos, &mut node);
                    nodes.push(node);
                }
            }
            _ => *pos += 1,
        }
    }
    nodes
}

fn parse_body(toks: &[Tok], pos: &mut usize, node: &mut Node) {
    while *pos < toks.len() {
        match &toks[*pos] {
            Tok::Close => {
                *pos += 1;
                return;
            }
            Tok::Num(n) => {
                node.nums.push(*n);
                *pos += 1;
            }
            Tok::Str => *pos += 1,
            Tok::Open => {
                // Reference block `{ Name }`.
                *pos += 1;
                let mut r = Node::default();
                parse_body(toks, pos, &mut r);
            }
            Tok::Ident(_) => {
                let mut kids = parse_nodes_one(toks, pos);
                node.children.append(&mut kids);
            }
        }
    }
}

fn parse_nodes_one(toks: &[Tok], pos: &mut usize) -> Vec<Node> {
    let Tok::Ident(kind) = &toks[*pos] else {
        return vec![];
    };
    let kind = kind.clone();
    *pos += 1;
    if let Some(Tok::Ident(_)) = toks.get(*pos) {
        *pos += 1;
    }
    if let Some(Tok::Open) = toks.get(*pos) {
        *pos += 1;
        let mut node = Node {
            kind,
            ..Default::default()
        };
        parse_body(toks, pos, &mut node);
        vec![node]
    } else {
        vec![]
    }
}

fn find_meshes<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in nodes {
        if n.kind == "Mesh" {
            out.push(n);
        } else if n.kind == "Frame" {
            find_meshes(&n.children, out);
        }
    }
}

pub fn parse(src: &[u8]) -> Result<XMesh, String> {
    let text = String::from_utf8_lossy(src);
    let body = text
        .strip_prefix("xof ")
        .map(|s| &s[12.min(s.len())..])
        .unwrap_or(&text);
    let toks = tokenize(body);
    let mut pos = 0;
    let nodes = parse_nodes(&toks, &mut pos, false);
    let mut meshes = Vec::new();
    find_meshes(&nodes, &mut meshes);
    let mesh = meshes.first().ok_or("no Mesh in .x file")?;
    build_mesh(mesh)
}

fn build_mesh(node: &Node) -> Result<XMesh, String> {
    let n = &node.nums;
    let mut it = n.iter().copied();
    let mut next = || it.next().ok_or_else(|| "truncated mesh".to_string());
    let nverts = next()? as usize;
    let mut positions = Vec::with_capacity(nverts);
    for _ in 0..nverts {
        positions.push(v3(next()? as f32, next()? as f32, next()? as f32));
    }
    let nfaces = next()? as usize;
    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(nfaces);
    for _ in 0..nfaces {
        let c = next()? as usize;
        let mut f = Vec::with_capacity(c);
        for _ in 0..c {
            f.push(next()? as u32);
        }
        faces.push(f);
    }

    let mut out = XMesh {
        positions,
        normals: vec![Vec3::ZERO; nverts],
        uvs: vec![Vec2::ZERO; nverts],
        groups: Vec::new(),
        has_alpha: false,
    };
    let mut slots: Vec<Vec<u16>> = Vec::new();
    let add_face = |slots: &mut Vec<Vec<u16>>, mat: usize, f: &[u32]| {
        if mat >= 16 {
            return;
        }
        while slots.len() <= mat {
            slots.push(Vec::new());
        }
        for i in 2..f.len() {
            slots[mat].extend_from_slice(&[f[0] as u16, f[i - 1] as u16, f[i] as u16]);
        }
    };
    let mut materials: Vec<Material> = Vec::new();
    let mut had_list = false;
    let mut had_normals = false;
    for child in &node.children {
        match child.kind.as_str() {
            "MeshMaterialList" => {
                had_list = true;
                let c = &child.nums;
                let nmat = c.first().copied().unwrap_or(0.0) as usize;
                let nidx = c.get(1).copied().unwrap_or(0.0) as usize;
                if nmat == 1 || nidx != nfaces {
                    for f in &faces {
                        add_face(&mut slots, 0, f);
                    }
                } else {
                    for m in 0..nmat {
                        for (fi, f) in faces.iter().enumerate() {
                            if c.get(2 + fi).copied().unwrap_or(-1.0) as usize == m {
                                add_face(&mut slots, m, f);
                            }
                        }
                    }
                }
                for mnode in child.children.iter().filter(|k| k.kind == "Material") {
                    let v = |i: usize| mnode.nums.get(i).copied().unwrap_or(0.0) as f32;
                    let mat = Material {
                        diffuse: [v(0), v(1), v(2), v(3)],
                        power: v(4),
                        specular: [v(5), v(6), v(7)],
                        emissive: [v(8), v(9), v(10)],
                    };
                    if mat.diffuse[3] < 1.0 {
                        out.has_alpha = true;
                    }
                    materials.push(mat);
                }
            }
            "MeshNormals" => {
                let c = &child.nums;
                let nn = c.first().copied().unwrap_or(0.0) as usize;
                if nn == nverts {
                    for i in 0..nn {
                        out.normals[i] = v3(
                            c[1 + i * 3] as f32,
                            c[2 + i * 3] as f32,
                            c[3 + i * 3] as f32,
                        );
                    }
                    had_normals = true;
                }
            }
            "MeshTextureCoords" => {
                let c = &child.nums;
                let nt = c.first().copied().unwrap_or(0.0) as usize;
                if nt == nverts {
                    for i in 0..nt {
                        out.uvs[i] = Vec2::new(c[1 + i * 2] as f32, c[2 + i * 2] as f32);
                    }
                }
            }
            _ => {}
        }
    }
    if !had_list {
        for f in &faces {
            add_face(&mut slots, 0, f);
        }
    }
    for (i, idx) in slots.into_iter().enumerate() {
        let m = materials.get(i).copied().unwrap_or_default();
        out.groups.push((m, idx));
    }
    if !had_normals {
        compute_normals(&mut out);
    }
    Ok(out)
}

fn compute_normals(m: &mut XMesh) {
    let mut acc = vec![Vec3::ZERO; m.positions.len()];
    for (_, idx) in &m.groups {
        for t in idx.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let (p0, p1, p2) = (m.positions[a], m.positions[b], m.positions[c]);
            let n = (p1 - p0).cross(p2 - p1).normalize_or_zero();
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
    }
    for (dst, mut n) in m.normals.iter_mut().zip(acc) {
        if n.length() < 0.1 {
            n = v3(0.0, 0.0, 1.0);
        }
        *dst = n.normalize_or_zero();
    }
}

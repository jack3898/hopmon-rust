//! Unit tests for the engine conventions and game rules, plus tests that run
//! against the real game data when `HOPMON_DATA` points at a directory
//! containing `Hopmon.exe`.

use crate::assets::xfile;
use crate::audio::volume_to_gain;
use crate::game::Rng;
use crate::game::map::{Map, Spawn};
use crate::math::{Mat, PI, Vec3, v3};

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn d3d_rotations_are_left_handed_row_vector() {
    // D3DUtil_SetRotateYMatrix: x' = x cos + z sin, z' = -x sin + z cos.
    assert!(close(
        Mat::rot_y(PI / 2.0).transform_point(v3(1.0, 0.0, 0.0)),
        v3(0.0, 0.0, -1.0)
    ));
    assert!(close(
        Mat::rot_x(PI / 2.0).transform_point(v3(0.0, 1.0, 0.0)),
        v3(0.0, 0.0, 1.0)
    ));
    assert!(close(
        Mat::rot_z(PI / 2.0).transform_point(v3(1.0, 0.0, 0.0)),
        v3(0.0, 1.0, 0.0)
    ));
}

#[test]
fn view_and_projection_map_into_gl_clip_space() {
    let view = Mat::look_at(v3(0.0, 0.0, -500.0), Vec3::ZERO, v3(0.0, 1.0, 0.0));
    assert!(close(view.transform_point(Vec3::ZERO), v3(0.0, 0.0, 500.0)));
    let proj = Mat::projection(0.733, 0.75, 10.0, 2000.0);
    let m = view.mul(&proj).to_gl();
    let near = m * macroquad::math::Vec4::new(0.0, 0.0, -490.0, 1.0);
    let far = m * macroquad::math::Vec4::new(0.0, 0.0, 1500.0, 1.0);
    assert!((near.z / near.w + 1.0).abs() < 1e-3);
    assert!((far.z / far.w - 1.0).abs() < 1e-3);
}

#[test]
fn affine_inverse_round_trips() {
    let m = Mat::rot_y(0.7)
        .mul(&Mat::rot_x(0.3))
        .mul(&Mat::translate(v3(4.0, -2.0, 9.0)));
    let p = v3(1.0, 2.0, 3.0);
    assert!(close(
        m.invert_affine().transform_point(m.transform_point(p)),
        p
    ));
}

#[test]
fn rng_matches_msvc_rand() {
    // srand(1); rand() x3 with the Microsoft C runtime.
    let mut r = Rng::new(1);
    assert_eq!([r.rand(), r.rand(), r.rand()], [41, 18467, 6334]);
    assert_eq!(Rng::new(5).random(0), 0);
}

#[test]
fn directsound_volume_curve() {
    assert!((volume_to_gain(100) - 1.0).abs() < 1e-6);
    assert!((volume_to_gain(80) - 0.1).abs() < 1e-6);
    assert!(volume_to_gain(0) < 1e-4);
}

const CUBE_X: &str = "xof 0303txt 0032
Mesh Box {
 4;
 0.0;0.0;0.0;, 1.0;0.0;0.0;, 1.0;1.0;0.0;, 0.0;1.0;0.0;;
 1;
 4;0,1,2,3;;
 MeshMaterialList {
  1;
  1;
  0;;
  Material Red {
   1.0;0.0;0.0;0.5;;
   0.0;
   0.0;0.0;0.0;;
   0.0;0.0;0.0;;
  }
 }
}";

#[test]
fn xfile_fan_triangulates_and_computes_normals() {
    let m = xfile::parse(CUBE_X.as_bytes()).unwrap();
    assert_eq!(m.positions.len(), 4);
    assert_eq!(m.groups.len(), 1);
    assert_eq!(m.groups[0].1, vec![0, 1, 2, 0, 2, 3]);
    assert!(m.has_alpha);
    // No MeshNormals: the face normal is averaged in.
    assert!(close(m.normals[0], v3(0.0, 0.0, 1.0)) || close(m.normals[0], v3(0.0, 0.0, -1.0)));
}

fn lines(s: &str) -> Vec<String> {
    s.lines().map(str::to_string).collect()
}

#[test]
fn map_rules_and_spawns() {
    let src = lines("1111\n1@$1\n1a*1\n1111\n____");
    let (map, spawns) = Map::build(&src, 40.0, 0x9c, false);
    // One cell of padding on every side.
    assert_eq!((map.size_x, map.size_z), (6, 6));
    let pac = spawns.iter().find_map(|s| match s {
        Spawn::Pac { mx, mz, .. } => Some((*mx, *mz)),
        _ => None,
    });
    assert_eq!(pac, Some((2, 2)));
    assert!(spawns.iter().any(|s| matches!(s, Spawn::Crystal { .. })));
    assert!(
        spawns
            .iter()
            .any(|s| matches!(s, Spawn::Enemy { kind: b'a', .. }))
    );
    assert!(map.is_blocked_pac(1, 1)); // wall
    assert!(!map.is_blocked_pac(2, 2)); // start tile became floor
    assert!(map.is_blocked_enemy(2, 3)); // occupied by the enemy
    assert!(map.is_warp_zone(3, 3));
    assert!(map.is_blocked_pac(0, 0)); // void padding
    // World <-> map conversion round trip for the cell centre of (3, 3).
    let x = 3.0 * 40.0 + 20.0 - 6.0 * 40.0 / 2.0;
    let z = (6 - 3) as f32 * 40.0 - 20.0 - 6.0 * 40.0 / 2.0;
    assert_eq!((map.pos_x_to_map(x), map.pos_z_to_map(z)), (3, 3));
}

fn data_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOPMON_DATA").map(Into::into)
}

#[test]
fn real_data_loads_every_level_and_mesh() {
    let Some(dir) = data_dir() else {
        eprintln!("HOPMON_DATA not set; skipping");
        return;
    };
    let assets = crate::assets::Assets::locate(Some(&dir)).expect("Hopmon.exe");
    let mut app = crate::game::AppData {
        current_level: 0,
        max_sel_level: 0,
        max_level: 44,
        state: crate::game::AppState::StartLogo,
        save: crate::save::Save::load(44, Some(std::env::temp_dir().join("hopmon-test.txt"))),
    };
    for lvl in 0..=44 {
        app.current_level = lvl;
        let l = assets.text_lines(crate::game::map_res_id(&app));
        let (map, spawns) = Map::build(&l, 40.0, 0x9c, false);
        assert!(map.size_x > 4 && map.size_z > 4, "level {lvl}");
        let pacs = spawns
            .iter()
            .filter(|s| matches!(s, Spawn::Pac { .. }))
            .count();
        assert_eq!(pacs, 1, "level {lvl} start positions");
        assert!(
            spawns.iter().any(|s| matches!(s, Spawn::Crystal { .. })),
            "level {lvl}"
        );
    }
    for id in [
        0x94u16, 0x95, 0x96, 199, 0x9d, 0x149, 0xa4, 0xaf, 0xae, 0xad, 0x14a, 0x14b, 0xd2, 0xd4,
        0xa5, 0xb3, 0xb2, 0xb1, 0xb4, 0xa6, 0xac, 0xb0, 0xa7, 0xa8, 0x13b, 0xab,
    ] {
        let m = assets.mesh(id);
        assert!(!m.positions.is_empty(), "mesh {id:#x}");
        assert!(m.groups.iter().any(|g| !g.1.is_empty()), "mesh {id:#x}");
    }
    for id in [0x9c, 0xfb, 0xfc, 0xc1, 0xa0, 0xba, 0xc2, 0xc3] {
        assert!(assets.bitmap(id).is_some(), "bitmap {id:#x}");
    }
    for (id, _) in crate::audio::EFFECTS {
        assert!(
            assets.wave(id).is_some_and(|w| w.starts_with(b"RIFF")),
            "wave {id:#x}"
        );
    }
}

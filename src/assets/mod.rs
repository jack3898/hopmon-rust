//! Game data access. Everything is read at runtime from the user's own copy
//! of the original freeware release (`Hopmon.exe` + `Music0X.mid`); no
//! original assets are redistributed with this port.

pub mod bmp;
pub mod pe;
pub mod xfile;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pe::{ResType, Resources};

pub struct Assets {
    pub dir: PathBuf,
    res: Resources,
    meshes: std::cell::RefCell<HashMap<u16, Rc<xfile::XMesh>>>,
}

pub const MUSIC_FILES: [&str; 4] = ["Music01.mid", "Music02.mid", "Music03.mid", "Music04.mid"];

/// Finds a file case-insensitively inside `dir`.
pub fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.is_file() && p.file_name()?.to_str()?.eq_ignore_ascii_case(name)).then_some(p)
    })
}

/// Candidate data directories, in priority order.
pub fn candidate_dirs(cli: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = cli {
        v.push(p.to_path_buf());
    }
    if let Ok(p) = std::env::var("HOPMON_DATA") {
        v.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(d) = exe.parent()
    {
        v.push(d.to_path_buf());
        v.push(d.join("data"));
        // macOS .app bundle: Contents/MacOS/hopmon -> Contents/Resources
        v.push(d.join("../Resources"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        v.push(cwd.clone());
        v.push(cwd.join("data"));
        v.push(cwd.join("assets"));
    }
    v
}

impl Assets {
    pub fn locate(cli: Option<&Path>) -> Result<Assets, String> {
        let dirs = candidate_dirs(cli);
        for d in &dirs {
            if let Some(exe) = find_file(d, "Hopmon.exe") {
                return Assets::load(d, &exe);
            }
        }
        let list: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        Err(format!(
            "Could not find Hopmon.exe. Searched:\n  {}\n\nCopy Hopmon.exe and Music01-04.mid from the original \
             freeware release next to this program, or pass --data <dir>.",
            list.join("\n  ")
        ))
    }

    pub fn load(dir: &Path, exe: &Path) -> Result<Assets, String> {
        let bytes = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
        let res = pe::parse(&bytes).map_err(|e| format!("{}: {e}", exe.display()))?;
        if res.get(&ResType::Id(pe::RT_RCDATA), 0x94).is_none() {
            return Err(format!("{} does not look like Hopmon.exe", exe.display()));
        }
        Ok(Assets {
            dir: dir.to_path_buf(),
            res,
            meshes: Default::default(),
        })
    }

    pub fn bitmap(&self, id: u16) -> Option<bmp::Image> {
        let d = self.res.get(&ResType::Id(pe::RT_BITMAP), id as u32)?;
        match bmp::decode(d) {
            Ok(i) => Some(i),
            Err(e) => {
                eprintln!("bitmap {id}: {e}");
                None
            }
        }
    }

    pub fn mesh(&self, id: u16) -> Rc<xfile::XMesh> {
        if let Some(m) = self.meshes.borrow().get(&id) {
            return m.clone();
        }
        let m = self
            .res
            .get(&ResType::Id(pe::RT_RCDATA), id as u32)
            .and_then(|d| match xfile::parse(d) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("mesh {id}: {e}");
                    None
                }
            })
            .unwrap_or_default();
        let m = Rc::new(m);
        self.meshes.borrow_mut().insert(id, m.clone());
        m
    }

    /// `GetStringListFromResource`: the level text split into lines.
    pub fn text_lines(&self, id: u16) -> Vec<String> {
        let Some(d) = self.res.get(&ResType::Id(pe::RT_RCDATA), id as u32) else {
            return Vec::new();
        };
        let end = d.iter().position(|&b| b == 0).unwrap_or(d.len());
        let s = String::from_utf8_lossy(&d[..end]);
        s.split('\n')
            .map(|l| l.trim_end_matches(['\r', '\n']).to_string())
            .collect()
    }

    pub fn wave(&self, id: u16) -> Option<&[u8]> {
        self.res.get(&ResType::Name(pe::RT_WAVE.into()), id as u32)
    }

    pub fn midi(&self, index: usize) -> Option<Vec<u8>> {
        let name = MUSIC_FILES.get(index)?;
        std::fs::read(find_file(&self.dir, name)?).ok()
    }
}

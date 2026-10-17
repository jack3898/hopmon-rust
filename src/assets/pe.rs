//! Minimal PE (Portable Executable) resource directory reader.
//!
//! Hopmon.exe embeds all of its game data (textures, sounds, meshes and the
//! level maps) as Win32 resources. This module walks the `.rsrc` section and
//! returns the raw payload of every resource keyed by `(type, id)`.

use std::collections::HashMap;

pub const RT_BITMAP: u32 = 2;
pub const RT_RCDATA: u32 = 10;
/// Custom resource type used by the game for wave files ("WAVE").
pub const RT_WAVE: &str = "WAVE";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResType {
    Id(u32),
    Name(String),
}

#[derive(Default)]
pub struct Resources {
    pub entries: HashMap<(ResType, u32), Vec<u8>>,
}

impl Resources {
    pub fn get(&self, ty: &ResType, id: u32) -> Option<&[u8]> {
        self.entries.get(&(ty.clone(), id)).map(|v| v.as_slice())
    }
}

fn rd16(d: &[u8], o: usize) -> Result<u16, String> {
    d.get(o..o + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| format!("truncated PE at {o:#x}"))
}

fn rd32(d: &[u8], o: usize) -> Result<u32, String> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("truncated PE at {o:#x}"))
}

struct Section {
    va: u32,
    vsize: u32,
    raw_off: u32,
    raw_size: u32,
}

fn rva_to_off(sections: &[Section], rva: u32) -> Option<usize> {
    sections.iter().find_map(|s| {
        let size = s.vsize.max(s.raw_size);
        if rva >= s.va && rva < s.va + size {
            Some((rva - s.va + s.raw_off) as usize)
        } else {
            None
        }
    })
}

pub fn parse(data: &[u8]) -> Result<Resources, String> {
    if data.get(0..2) != Some(b"MZ") {
        return Err("not an MZ executable".into());
    }
    let pe = rd32(data, 0x3c)? as usize;
    if data.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("missing PE signature".into());
    }
    let coff = pe + 4;
    let nsec = rd16(data, coff + 2)? as usize;
    let opt_size = rd16(data, coff + 16)? as usize;
    let opt = coff + 20;
    let magic = rd16(data, opt)?;
    let dd = match magic {
        0x10b => opt + 96,
        0x20b => opt + 112,
        _ => return Err("unknown optional header".into()),
    };
    let rsrc_rva = rd32(data, dd + 2 * 8)?;
    let sec_tab = opt + opt_size;
    let mut sections = Vec::new();
    for i in 0..nsec {
        let s = sec_tab + i * 40;
        sections.push(Section {
            vsize: rd32(data, s + 8)?,
            va: rd32(data, s + 12)?,
            raw_size: rd32(data, s + 16)?,
            raw_off: rd32(data, s + 20)?,
        });
    }
    let base = rva_to_off(&sections, rsrc_rva).ok_or("no resource section")?;
    let mut res = Resources::default();

    let read_name = |off: usize| -> Result<String, String> {
        let len = rd16(data, off)? as usize;
        let mut s = String::new();
        for i in 0..len {
            let c = rd16(data, off + 2 + i * 2)?;
            s.push(char::from_u32(c as u32).unwrap_or('?'));
        }
        Ok(s)
    };
    let entries = |dir: usize| -> Result<Vec<(u32, u32)>, String> {
        let named = rd16(data, dir + 12)? as usize;
        let ids = rd16(data, dir + 14)? as usize;
        let mut v = Vec::new();
        for i in 0..named + ids {
            let e = dir + 16 + i * 8;
            v.push((rd32(data, e)?, rd32(data, e + 4)?));
        }
        Ok(v)
    };

    for (tname, toff) in entries(base)? {
        let ty = if tname & 0x8000_0000 != 0 {
            ResType::Name(read_name(base + (tname & 0x7fff_ffff) as usize)?)
        } else {
            ResType::Id(tname)
        };
        if toff & 0x8000_0000 == 0 {
            continue;
        }
        for (iname, ioff) in entries(base + (toff & 0x7fff_ffff) as usize)? {
            if iname & 0x8000_0000 != 0 || ioff & 0x8000_0000 == 0 {
                continue;
            }
            // First language only.
            if let Some((_, loff)) = entries(base + (ioff & 0x7fff_ffff) as usize)?.first() {
                let de = base + *loff as usize;
                let rva = rd32(data, de)?;
                let size = rd32(data, de + 4)? as usize;
                if let Some(off) = rva_to_off(&sections, rva)
                    && let Some(bytes) = data.get(off..off + size)
                {
                    res.entries.insert((ty.clone(), iname), bytes.to_vec());
                }
            }
        }
    }
    Ok(res)
}

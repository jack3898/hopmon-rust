//! Replacement for the `HKCU\Software\SaitoGames\Hopmon\Level` registry value
//! (highest unlocked level), stored as a small text file in the user's
//! configuration directory.

use std::path::PathBuf;

pub struct Save {
    pub max_level: i32,
    path: Option<PathBuf>,
}

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("SaitoGames").join("Hopmon"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/SaitoGames/Hopmon"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("hopmon"))
    }
}

impl Save {
    pub fn load(max_allowed: i32, path: Option<PathBuf>) -> Save {
        let path = path.or_else(|| config_dir().map(|d| d.join("save.txt")));
        let mut lvl = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| {
                s.lines().find_map(|l| {
                    l.strip_prefix("Level=")
                        .and_then(|v| v.trim().parse::<i32>().ok())
                })
            })
            .unwrap_or(0);
        if lvl < 0 || lvl > max_allowed {
            lvl = 0;
        }
        Save {
            max_level: lvl,
            path,
        }
    }

    pub fn write(&self) {
        let Some(p) = &self.path else { return };
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = std::fs::write(p, format!("Level={}\n", self.max_level)) {
            eprintln!("could not save progress to {}: {e}", p.display());
        }
    }
}

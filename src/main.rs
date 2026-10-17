//! Hopmon — a cross-platform Rust port of Saito Games' freeware 3D arcade
//! game (2001-2009), rebuilt from a reverse-engineered `Hopmon.exe`.

// No console window for release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod audio;
mod game;
mod gfx;
mod input;
mod math;
mod midi;
mod obj3d;
mod save;
mod sprite;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use macroquad::prelude::*;

use game::scenes::{Ending, Logo, SelectLevel, Title};
use game::world::World;
use game::{AppData, AppState, Ctx, Rng, Scene};

const HELP: &str = "\
Hopmon (Rust port)

USAGE: hopmon [OPTIONS]

OPTIONS:
  --data <DIR>         Directory containing Hopmon.exe and Music01-04.mid
                       (default: next to this program, ./data, or $HOPMON_DATA)
  --soundfont <FILE>   SoundFont (.sf2) used to play the MIDI music
                       (default: $HOPMON_SOUNDFONT or a system GM SoundFont;
                       otherwise a built-in synthesizer is used)
  --fullscreen         Start in fullscreen (toggle with Alt+Enter or F11)
  --scale <N>          Initial window size as a multiple of 640x480 (default 1)
  --level <N>          Start directly at level N (1-45)
  --no-sound           Disable sound effects and music
  --save <FILE>        Use this progress file instead of the default location
  --extract <DIR>      Dump the embedded game data (bitmaps, meshes, maps,
                       sounds) to DIR and exit
  --render-music <DIR> Render Music01-04.mid to WAV files in DIR and exit
  --frames <N>         Quit after N frames (testing)
  --screenshot <FILE>  Save a PNG of the last frame on exit (testing)
  --shot-every <N>     With --screenshot, also save FILE-<frame>.png every N frames
  --autoplay           Drive the menus and game with scripted input (testing)
  -h, --help           Show this help

CONTROLS:
  Arrow keys             Move Hopmon (relative to the camera)
  Shift / Z              Fire (when the gauge is full)
  Ctrl / X + Left/Right  Rotate the camera
  Ctrl / X + Up/Down     Tilt the camera
  P / F3                 Pause
  Esc                    Back / give up the level
  Gamepad: stick/d-pad move, A fire, B + direction camera, Start confirm";

#[derive(Default)]
struct Args {
    data: Option<PathBuf>,
    soundfont: Option<PathBuf>,
    fullscreen: bool,
    scale: f32,
    level: Option<i32>,
    no_sound: bool,
    save: Option<PathBuf>,
    extract: Option<PathBuf>,
    render_music: Option<PathBuf>,
    frames: Option<u64>,
    screenshot: Option<PathBuf>,
    shot_every: Option<u64>,
    autoplay: bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        scale: 1.0,
        ..Default::default()
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = |name: &str| {
            it.next().unwrap_or_else(|| {
                eprintln!("missing value for {name}");
                std::process::exit(2)
            })
        };
        match arg.as_str() {
            "--data" => a.data = Some(val("--data").into()),
            "--soundfont" => a.soundfont = Some(val("--soundfont").into()),
            "--fullscreen" => a.fullscreen = true,
            "--scale" => a.scale = val("--scale").parse().unwrap_or(1.0),
            "--level" => a.level = val("--level").parse().ok(),
            "--no-sound" => a.no_sound = true,
            "--save" => a.save = Some(val("--save").into()),
            "--extract" => a.extract = Some(val("--extract").into()),
            "--render-music" => a.render_music = Some(val("--render-music").into()),
            "--frames" => a.frames = val("--frames").parse().ok(),
            "--screenshot" => a.screenshot = Some(val("--screenshot").into()),
            "--shot-every" => a.shot_every = val("--shot-every").parse().ok(),
            "--autoplay" => a.autoplay = true,
            "-h" | "--help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    a
}

fn window_conf() -> macroquad::conf::Conf {
    let a = parse_args();
    let scale = a.scale.clamp(0.5, 8.0);
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: "Hopmon".to_owned(),
            window_width: (gfx::SCREEN_W as f32 * scale) as i32,
            window_height: (gfx::SCREEN_H as f32 * scale) as i32,
            fullscreen: a.fullscreen,
            window_resizable: true,
            ..Default::default()
        },
        draw_call_vertex_capacity: 65536,
        draw_call_index_capacity: 131072,
        ..Default::default()
    }
}

/// Textures loaded by `XApplication::OneTimeSceneInit` (id, D3DTextr flags).
const TEXTURES: [(u16, u32); 14] = [
    (0x9c, 0),
    (0xfb, 0),
    (0xfc, 0),
    (0xa0, 0),
    (0xa1, 0),
    (0xa9, 0),
    (0xaa, 0),
    (0xb6, 0),
    (0xb7, 0),
    (0xdc, gfx::TEXTR_TRANSPARENT_BLACK),
    (0xa2, 0),
    (0xa3, 0),
    (0xc1, 0),
    (0x14c, 0),
];

fn extract(assets: &assets::Assets, dir: &std::path::Path) -> Result<(), String> {
    use assets::pe::{RT_BITMAP, RT_RCDATA, ResType};
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let res = assets::pe::parse(
        &std::fs::read(assets::find_file(&assets.dir, "Hopmon.exe").unwrap())
            .map_err(|e| e.to_string())?,
    )?;
    let mut n = 0;
    for ((ty, id), data) in &res.entries {
        let name = match ty {
            ResType::Id(RT_BITMAP) => {
                // Prepend a BITMAPFILEHEADER so the file opens anywhere.
                let hsize = u32::from_le_bytes(data[0..4].try_into().unwrap());
                let bpp = u16::from_le_bytes(data[14..16].try_into().unwrap());
                let used = u32::from_le_bytes(data[32..36].try_into().unwrap());
                let colors = if bpp <= 8 {
                    if used == 0 { 1 << bpp } else { used }
                } else {
                    0
                };
                let off = 14 + hsize + colors * 4;
                let mut f = b"BM".to_vec();
                f.extend_from_slice(&(14 + data.len() as u32).to_le_bytes());
                f.extend_from_slice(&[0; 4]);
                f.extend_from_slice(&off.to_le_bytes());
                f.extend_from_slice(data);
                std::fs::write(dir.join(format!("bitmap_{id}.bmp")), f)
                    .map_err(|e| e.to_string())?;
                n += 1;
                continue;
            }
            ResType::Id(RT_RCDATA) if data.starts_with(b"xof ") => format!("mesh_{id}.x"),
            ResType::Id(RT_RCDATA) => format!("map_{id}.txt"),
            ResType::Name(s) if s == assets::pe::RT_WAVE => format!("sound_{id}.wav"),
            _ => continue,
        };
        std::fs::write(dir.join(name), data).map_err(|e| e.to_string())?;
        n += 1;
    }
    println!("extracted {n} resources to {}", dir.display());
    Ok(())
}

async fn error_screen(msg: String) {
    eprintln!("{msg}");
    loop {
        clear_background(Color::from_rgba(0x2d, 0x1d, 0x62, 255));
        let mut y = 40.0;
        for line in std::iter::once("HOPMON").chain(msg.lines()) {
            draw_text(line, 20.0, y, 20.0, WHITE);
            y += 24.0;
        }
        draw_text("Press Esc to quit.", 20.0, y + 24.0, 20.0, YELLOW);
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        next_frame().await;
    }
}

/// Scripted input for `--autoplay`: confirms menus and wanders around.
fn autoplay_script() -> Box<dyn FnMut(u64) -> Vec<u8>> {
    use input::dik;
    Box::new(|f: u64| {
        let mut keys = Vec::new();
        if f % 60 < 3 && f < 900 {
            keys.push(dik::RETURN);
        }
        let dirs = [dik::UP, dik::RIGHT, dik::DOWN, dik::LEFT];
        if f > 600 {
            keys.push(dirs[((f / 90) % 4) as usize]);
            if f % 50 < 2 {
                keys.push(dik::Z);
            }
            if (f / 300) % 5 == 3 {
                keys.push(dik::X);
            }
        }
        keys
    })
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    let assets = match assets::Assets::locate(args.data.as_deref()) {
        Ok(a) => a,
        Err(e) => return error_screen(e).await,
    };
    if let Some(dir) = &args.extract {
        if let Err(e) = extract(&assets, dir) {
            eprintln!("extract failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let soundfont = midi::find_soundfont(args.soundfont.as_deref()).or_else(|| {
        std::fs::read_dir(&assets.dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("sf2")))
    });
    if let Some(dir) = &args.render_music {
        let r = midi::Renderer::new(soundfont.as_deref());
        let _ = std::fs::create_dir_all(dir);
        for i in 0..4 {
            let Some(data) = assets.midi(i) else { continue };
            let t = std::time::Instant::now();
            match r.render_wav(&data) {
                Ok(wav) => {
                    let out = dir.join(format!("music0{}.wav", i + 1));
                    let _ = std::fs::write(&out, &wav);
                    println!(
                        "{} ({:.1}s audio, rendered in {:.2?})",
                        out.display(),
                        (wav.len() - 44) as f64 / 4.0 / midi::SAMPLE_RATE as f64,
                        t.elapsed()
                    );
                }
                Err(e) => eprintln!("Music0{}.mid: {e}", i + 1),
            }
        }
        return;
    }

    let mut gfx = gfx::Gfx::new();
    for (id, flags) in TEXTURES {
        gfx.load_texture(&assets, id, flags);
    }
    let mut audio = audio::Audio::new(&assets, !args.no_sound, soundfont).await;
    let mut input = input::Input::new();
    if args.autoplay {
        input.scripted = Some(autoplay_script());
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u32)
        .unwrap_or(1);
    let mut rng = Rng::new(seed);
    let max_level = 44;
    let save = save::Save::load(max_level, args.save.clone());
    let mut app = AppData {
        current_level: 0,
        max_sel_level: save.max_level,
        max_level,
        state: AppState::StartLogo,
        save,
    };
    if let Some(l) = args.level {
        app.current_level = (l - 1).clamp(0, max_level);
        app.max_sel_level = app.max_sel_level.max(app.current_level);
        app.state = AppState::StartGame;
    }

    let mut scene: Option<Box<dyn Scene>> = None;
    let mut last = get_time();
    let mut paused = false;
    let mut frame: u64 = 0;
    loop {
        input.poll();
        if is_key_pressed(KeyCode::P) || is_key_pressed(KeyCode::F3) {
            paused = !paused;
        }
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        if (alt && is_key_pressed(KeyCode::Enter)) || is_key_pressed(KeyCode::F11) {
            static mut FULL: bool = false;
            // SAFETY: only touched from the main thread.
            unsafe {
                FULL = !FULL;
                set_fullscreen(FULL);
            }
        }

        // CD3DApplication::Render3DEnvironment: 10 ms ticks, capped at 20.
        let now = get_time();
        if now < last {
            last = now;
        }
        let elapsed = now - last;
        let ticks = (elapsed * 100.0).floor() as u32;
        // Keep only the sub-tick remainder, like m_dwLastFrameTime.
        last = now - (elapsed - ticks as f64 * 0.01);
        let ticks = if args.frames.is_some() {
            2
        } else {
            ticks.min(20)
        };

        let mut ctx = Ctx {
            assets: &assets,
            gfx: &mut gfx,
            audio: &mut audio,
            input: &mut input,
            rng: &mut rng,
            app: &mut app,
        };
        if !paused && ticks > 0 {
            if ctx.app.state == AppState::InProcess
                && let Some(s) = scene.as_mut()
            {
                s.process_frame(&mut ctx, ticks);
            }
            match ctx.app.state {
                AppState::StartLogo => scene = Some(Box::new(Logo::new(&mut ctx))),
                AppState::StartTitle => scene = Some(Box::new(Title::new(&mut ctx))),
                AppState::StartSelLevel => scene = Some(Box::new(SelectLevel::new(&mut ctx))),
                AppState::StartGame => scene = Some(Box::new(World::new(&mut ctx))),
                AppState::StartEnding => scene = Some(Box::new(Ending::new(&mut ctx))),
                AppState::Exit => break,
                _ => {}
            }
            if !matches!(ctx.app.state, AppState::InProcess) {
                ctx.app.state = AppState::InProcess;
            }
        }

        ctx.gfx.begin_frame();
        if let Some(s) = scene.as_mut() {
            s.render(&mut ctx);
        }
        if paused {
            ctx.gfx.set_2d();
            draw_text("PAUSE", 280.0, 250.0, 40.0, WHITE);
        }
        ctx.gfx.end_frame();
        audio.update().await;

        frame += 1;
        if let (Some(p), Some(n)) = (&args.screenshot, args.shot_every)
            && n > 0
            && frame.is_multiple_of(n)
        {
            let stem = p.with_extension("");
            get_screen_data().export_png(&format!("{}-{frame:05}.png", stem.display()));
        }
        if args.frames.is_some_and(|n| frame >= n) {
            if let Some(p) = &args.screenshot {
                get_screen_data().export_png(&p.to_string_lossy());
                println!("saved {}", p.display());
            }
            break;
        }
        next_frame().await;
    }
    app.save();
}

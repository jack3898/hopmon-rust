# Hopmon (Rust port)

A cross-platform (Windows, macOS, Linux) rewrite in Rust of **HOPMON**, the
freeware 3D arcade puzzle game by Saito Games (2001–2009). The game logic
was recovered by reverse engineering the original `Hopmon.exe`.

Hopmon has to collect every crystal in the level and carry it to the warp
zone (the star tile). There are 45 levels in three worlds, with ghosts,
armoured cubes, jellyfish, UFOs, needle balls, drop traps, cannons, volcanoes
and steam vents in the way.

## You need the original game data

This repository contains **only code**. The game assets (textures, models,
levels, sounds and music) are © Saito Games and are read at runtime from your
own copy of the freeware release:

* `Hopmon.exe`, which embeds all bitmaps, meshes, levels and sound effects
* `Music01.mid` … `Music04.mid`

Put those files next to the `hopmon` executable, in a `data/` folder beside
it or in the current directory. You can also pass `--data <dir>` or set
`HOPMON_DATA`. The PDB and the HTML manuals are not needed.

## Building

You need a stable Rust toolchain (1.88 or newer; edition 2024).

```sh
cargo build --release
./target/release/hopmon --data /path/to/hopmon
```

Linux also needs the ALSA, udev and X11/GL development packages:

```sh
sudo apt-get install libasound2-dev libudev-dev libx11-dev libxi-dev libgl1-mesa-dev
```

On macOS and Windows no extra dependencies are needed. Gamepad support
(`gilrs`) is the default `gamepad` feature; build with
`--no-default-features` to leave it out.

## Controls

| Key | Action |
| --- | --- |
| Arrow keys | Move Hopmon (relative to the camera) |
| Shift / Z | Fire (when the FIRE READY gauge is full) |
| Ctrl / X + Left/Right | Rotate the camera 90° |
| Ctrl / X + Up/Down | Tilt the camera |
| P / F3 | Pause |
| Esc | Back (in a level: give up and return to level select) |
| Alt+Enter / F11 | Toggle fullscreen |

On a gamepad, the stick or d-pad moves, A fires, B is the camera modifier,
Start confirms and Back acts as Esc.

## Command line

```
--data <DIR>         Directory containing Hopmon.exe and Music01-04.mid
--soundfont <FILE>   SoundFont (.sf2) for the MIDI music
--fullscreen         Start fullscreen
--scale <N>          Initial window size as a multiple of 640x480
--level <N>          Jump straight into level N (1-45)
--no-sound           Disable audio
--save <FILE>        Use a different progress file
--extract <DIR>      Dump the embedded game data (BMP, .x meshes, maps, WAV)
--render-music <DIR> Render the four songs to WAV
```

### Music

The original handed the MIDI files to the Windows MCI sequencer, i.e. to
whatever General MIDI synthesizer the system had. This port renders the songs
itself:

* With a **SoundFont** (`--soundfont`, `HOPMON_SOUNDFONT`, a `.sf2` in the
  data folder, or a system GM SoundFont such as FluidR3 on Linux) it uses
  [rustysynth]. This gives the closest match to the original.
* Without one, a small built-in General MIDI approximation is used, so music
  works out of the box.

Songs are rendered on background threads at start-up and loop as they did in
the original.

### Progress

The original stored the highest unlocked level in
`HKCU\Software\SaitoGames\Hopmon\Level`. The port writes `Level=<n>` to:

* Windows: `%APPDATA%\SaitoGames\Hopmon\save.txt`
* macOS: `~/Library/Application Support/SaitoGames/Hopmon/save.txt`
* Linux: `$XDG_CONFIG_HOME/hopmon/save.txt` (default `~/.config/hopmon/`)

## How faithful is it?

The port follows the decompiled code function by function. It keeps:

* the 100 Hz fixed tick (`timeGetTime() / 10`, at most 20 ticks per frame),
  and the split between per-frame "rotate" updates and per-tick updates
* every state machine, timer, speed and constant (movement takes
  `40 + 3 × crystals carried` ticks, the fire gauge charges 3 per tick, and so on)
* the MSVC `rand()` LCG behind `Random(n)`, so enemy AI makes the same kind of
  choices
* the Direct3D 7 look: left-handed coordinates, the same projection, camera
  and directional light, per-vertex material lighting with flat or Gouraud
  shading, point and linear filtering, and the additive, multiplicative and
  alpha blend states
* the DirectDraw colour-keyed HUD and menu sprites, the curtain fades and the
  DirectSound volume curve and round-robin buffers

There are a few differences. The music is synthesized (see above), the window
can be resized with letterboxing, you can skip the logo, and a gamepad
can press Esc. The registration screen (`XRegisterInfoObject`) is left out:
the freeware 2.0 build already hard-codes `IsRegistered()` to return true.

See [docs/REVERSE_ENGINEERING.md](docs/REVERSE_ENGINEERING.md) for how the
binary was analysed and what was found.

## Code layout

```
src/
  main.rs           CLI, window, main loop (XApplication / CD3DApplication)
  assets/           PE resource reader, BMP decoder, DirectX .x parser
  gfx.rs            D3D7-style renderer on miniquad + DirectDraw-style blits
  obj3d.rs          JObject3D / JObject3D_Rect / _Shadow / _Particle / JObject3DX
  sprite.rs         JSprite / JNumber
  audio.rs, midi.rs JSoundList / JMidiList, MIDI renderer
  input.rs          JDInput (keyboard + joystick)
  save.rs           registry replacement
  game/             the X* game classes: map, pac, enemy, crystal, fire,
                    particle, mapobj, camera, status, world (XGameObject), scenes
tools/re/           scripts used for the reverse engineering
```

## Testing

```sh
cargo test                                # unit tests
HOPMON_DATA=/path/to/hopmon cargo test    # also checks all 45 levels and assets
```

For automated runs, `--frames N --screenshot out.png [--shot-every K]
--autoplay` runs the game on scripted input and saves screenshots. It works
under `xvfb-run` on headless Linux.

## Credits

HOPMON: programming and graphics by James Saito, music by T. Kinoshita.
© 2001–2009 Saito Games. This is an unofficial, fan-made port. It does not
contain or redistribute any of the original game's files.

[rustysynth]: https://github.com/sinshu/rustysynth

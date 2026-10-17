# Reverse engineering Hopmon.exe

This document covers how the original game was analysed, what the binary
contains, and the game rules the Rust port reproduces.

## Target

| | |
| --- | --- |
| File | `Hopmon.exe`, PE32 GUI, Intel 80386, 3 201 536 bytes |
| SHA-256 | `6f4acb933e1692d97cc18819204c605287f72a71b9f62e1210ff8908be85930b` |
| Build | MSVC, DirectX 7 SDK framework (`CD3DApplication`, `CD3DFile`, `D3DTextr_*`, `ddutil`) |
| Symbols | `Hopmon.pdb` (RSDS, GUID `249cd78a-afd7-4d10-9da6-7d77512ee01a`, age 2) shipped with the game |
| Source tree (from PDB) | `D:\MyLocalDev\Hopmon\trunk\` |

## Tooling and workflow

1. **REA** ([morluto/rea](https://github.com/morluto/rea), `rea-agents` 6.5.0)
   with the **Ghidra 12.1.4** provider was used for the first overview
   (`rea analyze`, `rea search`). It found 3 853 procedures and 751 strings,
   and reported that the binary references `Hopmon.pdb`. REA's ephemeral
   Ghidra import does not load a PDB that sits next to the binary, so its
   procedure names were the auto-generated `FUN_*` ones.
2. **Ghidra headless** (the same engine REA drives) was then run directly
   with `Hopmon.pdb` next to the executable, so the PDB Universal analyser
   applied the real names and types (`XPac::ProcessFrame`,
   `XEnemyBase::DefaultProcessWait`, …).
3. Ghidra rebuilt the derived classes (`XPac : JObject3DX : JObject3DBase`)
   with an opaque base member, so every inherited field showed up as
   `this->_padding_` with no offset. To fix this, the raw type records were
   dumped with `llvm-pdbutil dump -types`, flattened into per-class field
   layouts (`tools/re/pdb_class_layouts.py`), and pushed back into the Ghidra
   database by a post-script (`tools/re/FixAndExport.java`) before
   decompiling. After that the decompiler printed real field names such as
   `this->game->crystalList->takenCount`.
4. `tools/re/clean_decomp.py` turned the `__real_XXXXXXXX` constant symbols
   into float literals. `tools/re/split_by_class.py` split the output into one
   file per class, about 320 KB of C for the `X*` game classes.
5. A few virtual calls lost their arguments in decompilation, for example
   `SetTexture(id, u0, v0, u1, v1)` on the wall-top quads. Those were read
   straight from the disassembly (`objdump -d -M intel`), together with the
   `.rdata` float constants they push.
6. Every resource was dumped (`tools/re/dump_resources.py`, or
   `hopmon --extract`).

## Program structure (from the PDB modules)

| Module | Contents |
| --- | --- |
| `d3dapp`, `d3dframe`, `d3denum`, `d3dfile`, `d3dtextr`, `d3dutil`, `d3dmath`, `ddutil` | DirectX 7 SDK sample framework |
| `JObject3D*`, `JSprite`, `JSurface*`, `JSound*`, `JMidi`, `JDinput`, `JString*`, `JList`, `JWinReg` | Saito's small engine library |
| `XAppMain` | `XApplication` (scene state machine), `XFPSDisplay` |
| `XTitle` | logo, title, level select, ending screens, scrolling background |
| `XGame` | `XGameObject` (one level), `XProcessObject` (fades), `XStatusDisplay` |
| `XMap` | `XMap`, `XPreviewMap`, `XCamera` |
| `XPac` | Hopmon and his shots |
| `XEnemy` | enemy list, base class and all enemy types |
| `XMapObject` | cannons, volcano, poles and trees, steam vents |
| `XChara` | crystals, particles, enemy shots |
| `XRegister` | registration screen; unreachable because `IsRegistered()` returns 1 |

### Main loop

`CD3DApplication::Render3DEnvironment` measures `timeGetTime()` in 10 ms
units and keeps the remainder. It calls `FrameMove(ticks)`, which
`XApplication::FrameMove` caps at 20, and then `Render()`.
`XGameObject::ProcessFrame(ticks)` runs the "rotate" updates once with the
tick count (spinning, rolling, the dying tumble) and then repeats the
collision checks and every entity's `ProcessFrame` once per tick. Input is
sampled once per call.

### Scenes (`XApplication::STATE`)

`START_LOGO → XLogoObject` (950 ticks) `→ START_TITLE → XTitleObject`
`→ START_SELLEVEL → XSelectLevelObject → START_GAME → XGameObject`. When a
level is cleared the game moves on to the next one, and after level 45 it
goes to `START_ENDING → XEndingObject → START_LOGO`. `XProcessObject` slides
the two curtain bitmaps (`0xc3`, `0xc2`) in or out over ticks 20 to 120.

### Rendering setup

* 640×480, projection `D3DUtil_SetProjectionMatrix(0.7330383, 0.75, 10, 2000)`
* one directional light: diffuse 0.77, specular 0.1, direction
  `(0.5, -1, 0.5)`; ambient render state `0xff898989`
* clear colour `0x2d1d62`
* `.x` meshes are lit with their materials (ambient = diffuse = face colour,
  plus emissive). A translucent material moves the whole mesh into a second,
  alpha-blended pass.
* `JObject3D` quads are pre-lit (`D3DLVERTEX`, lighting off). The blend modes
  are `ALPHA_PARTICLE` (ONE/ONE), `ALPHA_SHADOW` (ZERO/SRCCOLOR) and
  `ALPHA_TRANSPARENT` (SRCALPHA/INVSRCALPHA). None of them writes depth.
* billboards use the inverse view matrix with the translation replaced by
  the world position (`JObject3D_Rect::_SetWorldTransform2`)
* `JObject3D::RotateBy/ScaleBy/Shift` rewrite vertex data in place, so
  rotations accumulate in call order
* HUD and menus are `IDirectDrawSurface7::Blt` calls with source colour keys.
  The key is `CLR_INVALID`, so `DDColorMatch` takes the top-left pixel.

## Resources

| Type | IDs | Contents |
| --- | --- | --- |
| `RT_RCDATA` `xof 0303txt` | 148–150, 199 (Hopmon frames), 157, 164–180, 210, 212, 315, 329–331 | DirectX text meshes, one `Mesh` each with `MeshMaterialList` and `MeshNormals` |
| `RT_RCDATA` text | 265–314 (45 used) | level maps |
| `RT_BITMAP` | 156/251/252 | 256×256 4×4 tile atlases for worlds 1, 2 and 3 |
| `RT_BITMAP` | others | particles, shadow, HUD fonts and digits, menu text, curtain, logo |
| `WAVE` | 232–255, 317–319 | sound effects |
| files | `Music01..04.mid` | title, world 1, world 2, world 3 music |

Level `n` (0-based) is in world `n / 15`. `GetMapResId` maps it to a
resource. World 1 uses the order `0x11c, 0x115…0x11b, 0x114, 0x109…0x10e`,
world 2 uses `0x11d…0x12b` and world 3 uses `300…314`.

### Map format

One line per row, ending at an empty line or one starting with `_`. The
loader pads one void cell around the whole map. The cell size is 40 units.

| Char | Meaning |
| --- | --- |
| `.` | void (blocks Hopmon) |
| `-` `#` `=` | floor tiles |
| `+` | floor that enemies cannot enter |
| `@` | Hopmon's start |
| `$` | crystal |
| `*` | warp zone |
| `[` `>` `]` `<` | conveyor arrows: north, east, south, west |
| `1` `2` `3` | wall of that height; walls block shots |
| `!` | steam vent (floor) |
| `A` | up-cannon (lobs shells in random directions) |
| `N` `E` `S` `W` | cannon facing that direction |
| `V` | volcano (its 8 neighbours become blocking `Z`) |
| `P` | pole (world 1), palm tree (world 2), short pole (world 3) |
| `a` | ghost: wanders, sometimes turns toward Hopmon |
| `b` | cube: rolls, needs 2 hits |
| `c` | jellyfish: hops |
| `d` | UFO: flies over walls, cannot be destroyed, drops homing shots |
| `e` | sesame: alternates walking and resting, needs 2 hits |
| `m` `n` / `o` `p` | needle ball / fast needle ball, starting north / south; bounces |
| `q` | needle box: drops and rises periodically |
| `r` | needle box that drops once when Hopmon is underneath |
| `x` `y` | gate (blocks Hopmon until shot), facing north–south / east–west |

## Game rules (selected constants)

* **Movement:** one cell takes `40 + 3 × crystals carried` ticks, with a jump
  arc of height 15. Arrows slide Hopmon one cell in 20 ticks.
* **Fire:** the gauge charges 3 per tick up to 1000 (sound `0xfe` when full).
  A shot travels 2.5 units per tick, at most 3 at a time, and stops at walls
  and map objects.
* **Crystals:** carried crystals stack 12 units apart above Hopmon. Standing
  on the warp zone drops the top crystal every 80 ticks. With none left, the
  level is complete: Hopmon spins up to y = 500 and the next level starts 300
  ticks later.
* **Death:** touching an enemy or an enemy shot, or pressing Esc. Hopmon
  tumbles, carried crystals scatter, and after 300 ticks the game returns to
  the level select screen.
* **Collision:** sphere test `distance < r1 + r2`. Radii: Hopmon 13,
  enemies 15 (UFO 18, gate 10), shots 7–8, crystals 8.
* **Enemy wandering** (`DefaultProcessWait`): candidate list is
  straight ×2, right, left, and the direction toward Hopmon if it is not a
  U-turn. Pick with `Random(n)`; reverse only when nothing else is free.
* **UFO:** after 3 moves, counts down a fire timer (`Random(6)+3`, then
  `Random(5)+2`). When it fires, the shot drops straight down and turns toward
  Hopmon at y < 20.
* **Sound volume:** `GetVolumeOfPosition` gives 100 within 130 units of
  Hopmon, falls linearly to 70 at 550 units, and is 0 beyond. DirectSound
  attenuation is `(v / 100) × 10000 − 10000` hundredths of a dB.
* **Random:** MSVC `rand()` (`seed = seed × 214013 + 2531011`, returning
  bits 16–30), seeded with `GetTickCount()`.

The Rust modules under `src/game/` follow these classes one to one. Each
doc comment names the original class or function it ports.

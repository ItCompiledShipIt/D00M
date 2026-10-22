# D00M

A Doom-style first person shooter written from scratch in Rust. No engine, no
asset files: the renderer is a textured software raycaster and every texture,
sprite, weapon and sound is generated procedurally at start-up.

![title screen](docs/title.png)
![the hall](docs/hall.png)

## Run

```sh
cargo run --release
```

Rust is installed with `rustup` (toolchain in `~/.cargo`). The only
dependencies are `winit` (window), `softbuffer` (framebuffer), `cpal` (audio)
and `gilrs` (gamepads); the game and all of its art and sound are plain Rust.

## Controls

| Key | Action |
| --- | --- |
| `W` `A` `S` `D` | Move and strafe |
| Mouse / `←` `→` | Turn |
| `Shift` | Run |
| Left mouse / `Space` / `Ctrl` | Fire |
| `E` | Open a door / use the exit switch |
| `1` `2` | Pistol / shotgun |
| `Tab` | Automap |
| `R` | Restart level |
| `Esc` | Pause / back |

Menus use `↑` `↓` (`W` `S`) to select and `Enter` to confirm, `←` `→` to
change a value. In the pause menu `Esc` resumes. `New game` opens the skill
screen first. Quitting from the title screen needs a held key: pick `QUIT` and
hold `Enter`, or hold `Esc`, until the bar under the entry fills.

Gamepads work too: left stick or D-pad moves, right stick turns, right trigger
fires, `X`/`Y` uses doors and switches, `LB`/`RB` swap weapons, `Select` opens
the automap, `Start` pauses and `B` backs out of menus. Menus accept the D-pad
and `A` to confirm. If no controller is present the game simply ignores it; a
note appears when one is plugged in or unplugged.

Each map is cleared by pressing `E` on the green EXIT switch. The episode runs
through five levels (entryway, foundry, gates of hell, crimson depths,
inferno) and ends on an episode-complete screen with the high-score table.

## Front end

- Title screen with animated ember backdrop and the best run so far
- Doom-style skill selection (easy, normal, nightmare) before every new game;
  harder skills give the imps more health, more speed and more damage, fewer
  supplies and a third fewer demons on easy
- Intro crawl that skips with `Enter` and starts the game automatically
- Settings (mouse and gamepad stick sensitivity, inverted stick, fullscreen)
  and a volume menu
  with master, effects and ambient sliders; both are written to disk as you
  change them and restored on the next run (`$XDG_CONFIG_HOME/d00m/config.txt`,
  else `~/.config/d00m/config.txt`)
- A run in progress is saved automatically at every level start and whenever
  you pause or quit, into one of three slots (`save1.txt`..`save3.txt` beside
  the settings). The title screen then offers `CONTINUE`, which picks up the
  most recent save, and `LOAD GAME`, which lists all three slots with their
  level, kills, time and skill. Pausing also offers `SAVE GAME`, which writes
  the current run into whichever slot you pick and continues autosaving there.
  A new game uses the first empty slot, or replaces the oldest save when all
  three are full.
- Pause menu (resume, save game, settings, main menu), level-complete
  intermission with kills and time
- Episode-complete screen with a top-five high-score table (kills, time and
  skill), saved to `scores.txt` beside the settings and highlighted when the
  run places

## What is implemented

- Textured DDA raycaster with distance fog and side shading (480x300 internal
  framebuffer, upscaled to the window)
- Per-row textured floors and ceilings, head bob, view kick
- Billboard sprites with a per-column z-buffer (imps, barrels, pickups,
  fireballs) sorted back to front
- Sliding doors that block sight and movement until opened, and auto-close
- Five linked levels with an episode flow and intermissions
- Three difficulty levels that scale enemy health, speed, fire rate, damage
  taken, how many demons spawn and how much each pickup gives
- Player: collision with sliding, mouse look, health, two weapons with pellet
  spread, muzzle flash and recoil
- Enemies: idle/chase/attack/pain/death states, line of sight, melee and
  projectile attacks, corpses
- Explosive barrels, medkits, ammo, shells, shotgun pickup
- Doom-style status bar, messages, automap, death and level-complete screens
- Procedurally synthesised sound: pistol, shotgun, explosion, player pain,
  pickups, empty click, menu blips, door open/close, imp
  growl/attack/pain/death, plus a generated ambient bed; mixed with stereo
  panning and distance falloff (`cpal`, falls back to silence if no device is
  available)
- Procedural art for 7 walls, 15 sprites and 4 weapon frames, plus a hand-made
  5x7 bitmap font

## Development

```sh
cargo test              # 63 unit tests: maps, doors, combat, save slots, difficulty, audio, gamepad, UI, config, renderer
cargo clippy --all-targets
cargo run --release -- --shot frame.bmp --at 17.2,2.5,0 --steps 20 --fire
cargo run --release -- --shot menu.bmp --screen menu
```
`--shot` renders one frame headlessly to a BMP, which is handy for checks
without opening a window. `--at X,Y,ANGLE` places the player, `--level N`
selects a map, `--map` shows the automap, `--use` presses the use key once,
`--fire` holds the trigger, and `--screen menu|load|notice|skills|settings|volume|intro|pause|intermission|episode`
captures a front-end screen.

### Layout

| File | Contents |
| --- | --- |
| `src/main.rs` | Window, event loop, screen state machine, input mapping, CLI |
| `src/ui.rs` | Title, intro, settings, volume, pause and intermission screens |
| `src/levels.rs` | The five ASCII maps and their names |
| `src/game.rs` | Player, weapons, enemy AI, pickups, explosions |
| `src/map.rs` | ASCII level, doors, collision, line of sight |
| `src/render.rs` | Raycaster, floor casting, sprites, HUD, automap |
| `src/textures.rs` | Procedural wall/sprite/weapon art |
| `src/gfx.rs` | Colours and the texture rasteriser primitives |
| `src/font.rs` | 5x7 bitmap font |
| `src/audio.rs` | Synthesised effects, ambient loop, mixer and cpal stream |
| `src/pad.rs` | Gamepad polling and the mapping from buttons to actions |
| `src/config.rs` | Saving and loading settings, difficulty and the score table |
| `src/save.rs` | Save slots: encoding, decoding and summaries for the load screen |
| `src/shot.rs` | Headless BMP capture |
| `src/math.rs` | `Vec2`, RNG, noise |

## Next steps

- Vertical mouselook / true 3D sectors
- More monsters and weapons, and a proper sprite-sheet animation system
- Rebindable keys
- Mouse look while a gamepad is connected (currently the right stick only)
- Deleting or renaming individual save slots

# D00M

A Doom-style first person shooter written from scratch in Rust. No engine, no
asset files: the renderer is a textured software raycaster and every texture,
sprite and weapon is generated procedurally at start-up.

![the hall](docs/hall.png)

## Run

```sh
cargo run --release
```

Rust is installed with `rustup` (toolchain in `~/.cargo`). The only window
dependencies are `winit` and `softbuffer`; the game itself is plain Rust.

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
| `R` | Restart |
| `Esc` | Quit |

Clear the level by finding the shotgun, killing the imps and pressing `E` on the
green EXIT switch in the south hall.

## What is implemented

- Textured DDA raycaster with distance fog and side shading (480x300 internal
  framebuffer, upscaled to the window)
- Per-row textured floors and ceilings, head bob, view kick
- Billboard sprites with a per-column z-buffer (imps, barrels, pickups,
  fireballs) sorted back to front
- Sliding doors that block sight and movement until opened, and auto-close
- Player: collision with sliding, mouse look, health, two weapons with pellet
  spread, muzzle flash and recoil
- Enemies: idle/chase/attack/pain/death states, line of sight, melee and
  projectile attacks, corpses
- Explosive barrels, medkits, ammo, shells, shotgun pickup
- Doom-style status bar, messages, automap, death and level-complete screens
- Procedural art for 7 walls, 15 sprites and 4 weapon frames, plus a hand-made
  5x7 bitmap font

## Development

```sh
cargo test              # 21 unit tests: map, doors, collision, combat, renderer
cargo clippy --all-targets
cargo run --release -- --shot frame.bmp --at 17.2,2.5,0 --steps 20 --fire
```

`--shot` renders one frame headlessly to a BMP, which is handy for checks
without opening a window. `--at X,Y,ANGLE` places the player, `--map` shows the
automap, `--use` presses the use key once, and `--fire` holds the trigger.

### Layout

| File | Contents |
| --- | --- |
| `src/main.rs` | Window, event loop, input mapping, CLI |
| `src/game.rs` | Player, weapons, enemy AI, pickups, explosions |
| `src/map.rs` | ASCII level, doors, collision, line of sight |
| `src/render.rs` | Raycaster, floor casting, sprites, HUD, automap |
| `src/textures.rs` | Procedural wall/sprite/weapon art |
| `src/gfx.rs` | Colours and the texture rasteriser primitives |
| `src/font.rs` | 5x7 bitmap font |
| `src/shot.rs` | Headless BMP capture |
| `src/math.rs` | `Vec2`, RNG, noise |

## Next steps

- Sound effects and a level-exit intermission
- Multiple levels linked by an episode flow
- Vertical mouselook / true 3D sectors
- More monsters, weapons and a proper sprite-sheet animation system

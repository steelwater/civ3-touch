# FreeC3

An open-source, moddable game engine for Civilization 3. Built in Rust with Lua scripting for game rules.

[![CI](https://github.com/anthropics/fc3/actions/workflows/ci.yml/badge.svg)](https://github.com/anthropics/fc3/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

## What's implemented

**Core engine** — Deterministic game simulation with seeded RNG, command/event protocol, and full serialization support. Given the same seed and command sequence, the engine produces identical results every time.

**Map & terrain** — Procedural map generation using Perlin noise (elevation → terrain types, latitude → biomes). Cylindrical and toroidal wrapping. Terrain types include grassland, plains, desert, tundra, coast, ocean, hills, and mountains. Forest and jungle vegetation overlay.

**Units & movement** — Generational entity handles, structure-of-arrays storage. A\* pathfinding with Lua-defined movement costs, zone of control, movement interrupts (enemy sighting, ZoC entry), and per-tile event emission. Roads reduce movement cost. Scouts ignore terrain costs. Naval units restricted to water tiles.

**Combat** — Civ3-style probabilistic combat rounds. Terrain defense bonuses (hills +50%, forest/jungle +25%), city defense (+50%), and fortification (+25%). All combat logic defined in Lua.

**Cities & production** — City founding (settlers consumed), 21-tile fat cross, population growth from food surplus (Civ3 formula), shield accumulation toward unit production, tile yield assignment, buildings (granary, barracks, palace).

**Worker actions** — Road building (2 turns), mines (+1 shield on hills, 3 turns), irrigation (+1 food, 3 turns).

**Fog of war** — Three-state visibility (unseen/revealed/visible) per player. Sight range affected by terrain elevation. PlayerView never leaks hidden information.

**Lua mod system** — Sandboxed scripting environment with hook-based event system. Mods define unit types, terrain costs, combat modifiers, city growth rules, tech trees, and more. Priority-ordered hooks with cancellation support.

**AI** — Simple agent that explores, founds cities, produces units, and fights. Used for headless testing and as the desktop spectator opponent. Stress-tested over 10,000 games with zero panics.

**Desktop client** — wgpu-based isometric renderer with camera pan/zoom, terrain and unit sprite rendering (from Civ3 .PCX/.FLIC assets), fog of war visualization, city view, tech tree dialog, unit selection, and stack cycling. Currently runs in AI-vs-AI spectator mode.

**Headless client** — CLI runner for AI-vs-AI games with ASCII map output, batch mode, deterministic replay via saved command logs.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Clients                                            │
│  ┌──────────────┐  ┌───────────┐  ┌─────────────┐  │
│  │ fc3_desktop   │  │ fc3_headless│  │ (future     │  │
│  │ wgpu + winit  │  │ CLI + ASCII │  │  server)    │  │
│  └──────┬───────┘  └─────┬─────┘  └──────┬──────┘  │
│         │                │               │          │
│         └────────────────┼───────────────┘          │
│                          │                          │
│              Commands ↓  │  ↑ Events/PlayerView     │
│                          │                          │
│  ┌───────────────────────┴──────────────────────┐   │
│  │              fc3_core (headless)              │   │
│  │                                              │   │
│  │  Engine ← submit_command() → CommandResult   │   │
│  │    │                                         │   │
│  │    ├── World (tiles, units, cities, players) │   │
│  │    ├── Lua scripting (mlua, sandboxed)       │   │
│  │    ├── Pathfinding (A*, cost caching)        │   │
│  │    └── Turn state machine                    │   │
│  └──────────────────────────────────────────────┘   │
│                          │                          │
│              Lua hooks ↕ │                          │
│                          │                          │
│  ┌──────────────────────────────────────────────┐   │
│  │              mods/base/ (Lua)                │   │
│  │                                              │   │
│  │  units.lua · combat.lua · movement.lua       │   │
│  │  growth.lua · production.lua · yields.lua    │   │
│  │  buildings.lua · techs.lua · zoc.lua · ...   │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

**Separation of concerns:** The core engine (`fc3_core`) is a headless Rust library with no I/O, rendering, or platform dependencies. It owns all game state and exposes a command/event protocol. Clients are thin binaries that submit commands and render the resulting player views.

**Game rules live in Lua, not Rust.** The engine doesn't know what a "warrior" is or how combat damage is calculated. It provides storage, hooks, and execution machinery. Lua scripts in `mods/base/` define all unit types, costs, combat formulas, growth thresholds, and building effects. This makes the game fully moddable without recompiling.

**Commands in, events out.** All interaction with the engine goes through `submit_command() → CommandResult`. No direct state mutation from outside. Commands include `MoveUnit`, `AttackUnit`, `FortifyUnit`, `FoundCity`, `SetProduction`, `EndTurn`, etc. The engine returns events describing what happened.

**Deterministic execution.** Seeded RNG only (ChaCha8), no HashMap iteration in game logic, Lua hooks fire in defined order. This enables replay, debugging, and multiplayer synchronization.

## Project structure

```
crates/
  fc3_core/          Core engine library (~13k LOC, 468 tests)
  fc3_desktop/       Desktop client (wgpu + winit, isometric renderer)
  fc3_headless/      CLI client (AI-vs-AI runner, ASCII output)
  fc3_pcx/           Civ3 .PCX image format loader
  fc3_flic/          Civ3 .FLIC animation format loader
mods/
  base/              Vanilla Civ3 rules in Lua
```

## Running

### Desktop client

Renders an isometric map with unit sprites, fog of war, cities, and UI overlays. Currently runs AI-vs-AI in spectator mode.

```bash
cargo run -p fc3_desktop
```

Requires Civ3 asset files for full sprite rendering. Falls back to colored rectangles without them.

### Headless client

Runs AI-vs-AI games in the terminal. Useful for testing, stress-testing, and replay validation.

```bash
# Default game (2 players, 40x25 map, 500 turns)
cargo run -p fc3_headless

# Watch turn-by-turn with ASCII map
cargo run -p fc3_headless -- --verbose

# Large map, more players
cargo run -p fc3_headless -- --width 80 --height 50 --players 4 --max-turns 1000

# Batch mode: run 100 games, print aggregate stats
cargo run -p fc3_headless -- --batch 100

# Save and replay a game
cargo run -p fc3_headless -- --save-log game.json
cargo run -p fc3_headless -- --replay game.json --verbose
```

| Flag | Default | Description |
|------|---------|-------------|
| `--seed <N>` | 42 | RNG seed for map generation and AI |
| `--width <N>` | 40 | Map width in tiles |
| `--height <N>` | 25 | Map height in tiles |
| `--players <N>` | 2 | Number of AI players |
| `--max-turns <N>` | 500 | Turn limit |
| `-v, --verbose` | off | Print ASCII map and events each turn |
| `-q, --quiet` | off | Print only final result |
| `--batch <N>` | — | Run N games with incrementing seeds |
| `--save-log <PATH>` | — | Save command log for replay |
| `--replay <PATH>` | — | Replay a saved game |

### Tests

```bash
# All tests (468 across the workspace)
cargo test --workspace

# Core engine tests only (fastest feedback loop)
cargo test -p fc3_core

# A specific test
cargo test -p fc3_core test_warrior_movement_blocked

# Lint
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --check
```

## License

GPL-3.0

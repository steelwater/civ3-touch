# CLAUDE.md — Development Guide for FreeC3

This file is the source of truth for how to work in this codebase. Read it fully before making any changes.

## Project Overview

FreeC3 is an open-source, moddable game engine for Civilization 3. The architecture has three layers:

1. **Core engine** (`crates/fc3_core/`) — a headless Rust library with no I/O or rendering dependencies. Owns all game state, exposes a Command/Event protocol, embeds Lua for scripting.
2. **Clients** (`crates/fc3_headless/`, `crates/fc3_desktop/`, future `crates/fc3_server/`) — thin binaries that consume the core library.
3. **Mods** (`mods/`) — Lua scripts that define game rules. The `base` mod implements vanilla Civ3 rules.

The core engine must NEVER depend on platform-specific, rendering, or async I/O crates.

## Rust Conventions

### Style

- Run `cargo fmt` before every commit. CI enforces this.
- Run `cargo clippy --workspace -- -D warnings` before every commit. Treat all clippy warnings as errors.
- No `unwrap()` in library code (`fc3_core`). Use `Result` or `Option` with proper error handling. `unwrap()` is acceptable in tests and in binaries (`main.rs`) for top-level setup.
- No `unsafe` unless absolutely necessary and documented with a `// SAFETY:` comment.
- Prefer `&str` over `String` in function parameters. Return `String` when ownership transfer is needed.
- Use `#[inline]` only on trivially small functions called in hot loops (like `TileStore::idx()`). Don't over-annotate.

### Naming

- Types: `PascalCase` — `UnitStore`, `TileCoord`, `PlayerId`
- Functions and methods: `snake_case` — `spawn_unit`, `is_alive`, `get_movement`
- Constants: `SCREAMING_SNAKE_CASE` — `MAX_PLAYERS`, `DEFAULT_SIGHT_RANGE`
- Modules: `snake_case` — `unit_type.rs`, `dynamic.rs`
- Lua API functions exposed to scripts: `PascalCase.camelCase` — `Unit.get()`, `UnitType.define()`, `Engine.random()`

### Module Organization

```
crates/fc3_core/src/
├── lib.rs              # Public re-exports only. No logic here.
├── id.rs               # GenId, UnitId, CityId
├── types.rs            # PlayerId, TileCoord, UnitTypeId, etc.
├── tile.rs             # TileStore, Terrain, Visibility
├── unit.rs             # UnitStore (SoA)
├── unit_type.rs        # UnitType, UnitTypeRegistry
├── dynamic.rs          # DynamicColumns for mod-defined attributes
├── world.rs            # World struct, Player, WorldConfig
├── protocol.rs         # Command, Event, PlayerView, GameError
├── turn.rs             # TurnStateMachine, TurnPhase
├── engine.rs           # Engine struct, submit_command, player_view
└── scripting/
    ├── mod.rs          # ScriptEngine
    ├── hooks.lua       # Hook system (embedded via include_str!)
    ├── api_unit_type.rs
    ├── api_unit.rs
    ├── api_tile.rs
    └── api_engine.rs
```

Keep modules focused. If a file exceeds ~400 lines, consider splitting it.

### Error Handling

Define a crate-level error type:

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),
    #[error("Invalid unit ID: {0:?}")]
    InvalidUnit(UnitId),
    #[error("Game rule violation: {0}")]
    Rule(String),
}
```

Use `thiserror` for the library. Use `anyhow` in binaries if needed.

### Testing

- Every public function in `fc3_core` should have at least one non-trivial test.
- Unit tests go in the same file: `#[cfg(test)] mod tests { ... }` at the bottom.
- Integration tests go in `fc3_core/tests/` — these test cross-module behavior and Lua integration.
- Use `insta` for snapshot testing of complex outputs (PlayerView JSON, combat event sequences).
- Seed all RNG in tests. Never use `thread_rng()` in tests. Use `ChaCha8Rng::seed_from_u64(KNOWN_SEED)`.
- Name tests descriptively: `test_warrior_movement_blocked_by_mountain`, not `test_move_1`.

### Performance Considerations

- The SoA layout in `UnitStore`, `TileStore`, etc. exists for cache performance. Don't add `HashMap<UnitId, T>` lookups in hot paths.
- For known core attributes (movement, hp, position), use direct array access. Dynamic column lookup (HashMap → column index → array access) is for mod-defined attributes only.
- Don't prematurely optimize. The first priority is correctness and clean APIs. Profile before optimizing.
- When you do optimize, add a benchmark in `benches/` using `criterion`.

### Lua Interop Patterns

When registering Lua API functions:

1. Access the `World` via `lua.app_data_ref::<Rc<RefCell<World>>>()`.
2. Borrow it (`borrow()` or `borrow_mut()`) for the minimum duration needed.
3. Never hold a borrow across a Lua callback or hook fire — this will panic.
4. Convert Lua errors to `mlua::Error::external()` for clean error messages.
5. UnitId encoding for Lua: use a userdata type or encode as two integers (index, generation). Don't use a single integer — Lua integers are 64-bit but the encoding is fragile.

```rust
// GOOD: short borrow, clear error
unit_api.set("get", lua.create_function(|lua, (uid, attr): (LuaUnitId, String)| {
    let world = lua.app_data_ref::<Rc<RefCell<World>>>().unwrap();
    let world = world.borrow();
    let idx = world.units.get(uid.into())
        .ok_or_else(|| mlua::Error::external("invalid unit id"))?;
    // read and return
})?)?;

// BAD: holding borrow while calling Lua
let world = world.borrow();
let result = lua.call_function("some_hook", ())?; // PANIC if hook also borrows
```

## Git Conventions

### Commit Messages

Format: `<tag>: <short description>`

Where tags are:
- `feat`: new feature or API
- `fix`: bug fix
- `refactor`: internal change with no behavior change
- `docs`: documentation only
- `test`: adding or fixing tests

Examples:
```
feat: Create workspace and crate structure
feat: Implement TileStore with wrapping and neighbor queries
test: Add UnitStore spawn/destroy tests for generational index 
fix: Resolve incorrect UnitType.define() API in Lua
feat: Implement movement command with Lua cost callbacks
```

Keep the subject line under 72 characters. Add a body if the change is non-obvious:

```
feat: Implement movement command with Lua cost callbacks

Movement cost is determined by firing the on_calculate_movement_cost hook.
The base mod's movement.lua registers a handler that returns costs based on
terrain type. Mountains and oceans are impassable (cost = -1).

Fog of war is recalculated after each move using a simple sight-range check
around the unit's new position.
```

### Branching

- `main` is the primary branch. It should always compile and pass tests.
- For multi-task work sessions, work directly on `main` with one commit per task.
- For experimental or risky changes, use a feature branch: `feature/phase3-movement`, `fix/combat-rng-determinism`.
- Squash-merge feature branches.

### What to Commit

- Commit one logical change at a time.
- Include new tests in the same commit as the code they test.
- Do NOT commit: `target/`, `.DS_Store`, IDE settings, `*.log`.

## Architecture Invariants

These are rules that must never be violated. If a task seems to require violating one, stop and reconsider.

1. **`fc3_core` has no I/O.** No file system access (except in tests). No networking. No rendering. No `std::fs`, no `tokio`, no `wgpu`. The only exception is `mlua` loading Lua from embedded strings or from paths passed in by the binary.

2. **Game rules live in Lua, not Rust.** The engine does not know what a "warrior" is or how combat works. It provides storage, hooks, and execution. Lua provides the rules. If you find yourself writing an `if unit_type == "warrior"` in Rust, you're doing it wrong.

3. **Commands in, Events out.** All player interaction with the engine goes through `submit_command() -> CommandResult`. No direct state mutation from outside the engine. The protocol types are the contract between engine and client.

4. **Fog of war is authoritative.** `PlayerView` never leaks information a player shouldn't see. All client-facing data goes through the fog-of-war filter.

5. **Deterministic execution.** Given the same seed, config, and command sequence, the engine produces identical results. This means: seeded RNG only (no `thread_rng`, no system time), deterministic iteration order (no `HashMap` iteration in game logic — use `BTreeMap` or `Vec` where order matters), and Lua hooks fire in a defined order.

6. **Generational handles for entities.** Never use raw indices as public API. Always use `GenId` (or typed wrappers) so stale references are caught cleanly.

7. **Mods are sandboxed.** Lua scripts cannot access the file system, network, or execute system commands. `require_mod_file` is the only way to load code, and it's path-restricted.

## Common Pitfalls

**"I'll add Lua later."** — No. Even if the Lua hook is trivial, wire it up now. The refactoring cost compounds.

**HashMap iteration order in game logic.** — Rust's `HashMap` has non-deterministic iteration order. For anything that affects game state (processing units, resolving events), use `Vec` with defined ordering, or `BTreeMap`.

**Borrowing World across Lua calls.** — This is the #1 source of panics. The `RefCell<World>` will panic if you hold a `borrow()` while Lua code tries to `borrow_mut()`. Keep borrows short and never hold them across hook fires.

**Overdesigning early.** — Don't build systems you don't need yet. Build what's needed now, not what might be needed later.

**Tests that depend on HashMap order or floating point equality.** — Use integer math for game logic (multiply by 100 instead of using floats). Compare floats with epsilon if unavoidable.

## Running the Project

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Run core tests only (fastest feedback loop)
cargo test -p fc3_core

# Run a specific test
cargo test -p fc3_core test_warrior_movement_blocked

# Run the headless client
cargo run -p fc3_headless

# Check formatting
cargo fmt --check

# Lint
cargo clippy --workspace -- -D warnings
```

## File Locations Quick Reference

| What | Where |
|------|-------|
| This file | `CLAUDE.md` |
| Core engine source | `crates/fc3_core/src/` |
| Core engine tests | `crates/fc3_core/src/*.rs` (unit) + `crates/fc3_core/tests/` (integration) |
| Headless client | `crates/fc3_headless/src/main.rs` |
| Desktop client | `crates/fc3_desktop/src/main.rs` |
| Base mod (Lua) | `mods/base/` |
| CI config | `.github/workflows/ci.yml` |

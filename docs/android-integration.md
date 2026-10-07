# Android/Rust integration

## Milestone 3 extension (2026-10-06)

The [Milestone 3 record](milestone-3.md) supersedes the historical single-player and no-save limits below. The retained single-worker controller and thread-local Rust session remain. JNI adds general legal command submission, snapshot, save and transactional load operations. Rust runs a two-civilization game, the existing AI, pending city production queues and versioned replay validation. Android owns unit selection, native action menus, city/unit/improvement drawing, and atomic app-private manual/recovery file slots. The Milestone 2 importer, validation, terrain/Settler art and audio path are preserved.

Recovery is committed after each completed action, before the new state reaches the screen. Process restart offers explicit manual/recovery loading after import validation. Failed load leaves the existing native session intact; failed persistence is visible and keeps the user in the live session. Exact rules content and the engine/session contract must match before replay. No saved rules path is trusted, and no engine is shared across threads. See [testing.md](testing.md) for device journeys and [milestone-3.md](milestone-3.md) for the approved persistence decision and limitations.

## Historical Milestone 1 extension (2026-10-05)

Canonical [Milestone 1 brief](https://docs.google.com/document/d/1zmHPomRr53cus_xb0WnbSXfWid9S9STP8YGHWO4kGG0/edit). The existing Java/JNI spike is extended without new dependencies or vendor edits. `GameController` owns a single executor. Its Rust thread-local `Session` owns the engine and Lua; JNI exposes new-game, one-step movement, end-turn and close operations. There are no raw engine pointers or cross-thread engine transfers. JSON carries the upstream player view, immediate move options and command results to Java.

`MapView` draws synthetic isometric terrain and the Settler marker, projects touch coordinates, and highlights native move options. Java owns selection/presentation only. Rust filters engine move options to paths containing exactly one step, then submits `MoveUnit`; it rejects invalid/stale/exhausted/distant requests without queuing a destination. End Turn issues available `SkipUnit` commands then `EndTurn`. The prototype has one player, so this advances directly to the next numbered turn. It does not implement victory/opponents.

The Activity retains its controller across configuration changes using the platform retained-instance API. Pending work reports to the current Activity on the main thread. Finishing queues native cleanup on the same worker. There is no saved game or process-death restoration; reopening starts at New Game. This narrow lifecycle implementation uses the existing framework and does not adopt a new UI architecture.

## Historical Milestone 0 decision


Date: 2026-10-02. Owner: Dan. Approved choice: minimal Java/JNI spike with GPL-3.0. Context: prove FreeC3 can execute on Android before choosing a production renderer/UI architecture.

## Boundary

`MainActivity` copies packaged open-source Lua files into its private files directory and invokes `CoreBridge.smokeTest(absoluteRulesPath)` on a worker thread. `System.loadLibrary` loads the Rust cdylib. The target-gated JNI function validates string conversion, catches Rust unwinding panics, and converts errors to Java `IllegalStateException`. Java displays and logs a result; no simulation state crosses JNI.

The shared Rust function loads the actual base mod, generates a seeded 16×16 map, creates two players with Settler/Warrior units, checks wrong-player rejection, skips each player's units according to engine rules, ends both turns, serializes the command log, replays it, and compares both player views. The expected result is turn 2. No Windows executable runs and no proprietary assets are packaged.

The engine's `Rc<RefCell<World>>` stays within one scoped call/thread. Production sessions, persistence/lifecycle restoration, cancellation, an asynchronous event protocol, and rendering remain unimplemented. Recreating the Activity reruns a disposable smoke operation; it does not restore a game.

## Alternatives and consequences

A native-Activity/wgpu port would prematurely couple feasibility to the desktop renderer. A generalized binding generator would add infrastructure for an API that is not settled. The selected small JNI adapter uses the `jni` crate only on Android and leaves FreeC3 source unchanged. It can be replaced or extended after Milestone 1 requirements are approved.

Only `arm64-v8a` / `aarch64-linux-android` is built. Compile/target SDK 36, minimum API 26, NDK r27c (`27.2.12479018`), Rust 1.94.0. API 26 and physical devices have not been runtime-tested; the observed runtime is an ARM64 API-36 emulator. The minimum is a build choice, not a verified support claim.

The upstream core reads real Lua filesystem paths. Android APK assets are copied to private storage to satisfy that unchanged contract. GOG import/storage is not implemented. The separate PCX/FLIC crates accept bytes and can remain platform-independent; case-aware GOG path resolution is required before a real Android import flow.

See the source architecture table in `milestone-0-audit.md`, `build.md` for commands, and `testing.md` for evidence and limits.

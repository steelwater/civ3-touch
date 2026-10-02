# Android/Rust integration decision and architecture

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

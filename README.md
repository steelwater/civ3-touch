# Civ3Touch

An independent, open-source investigation of an Android client built around the FreeC3 simulation engine, using user-supplied GOG Civilization III Complete data. **Milestone 2 imports a narrow set of user-supplied GOG assets for the existing map prototype; this is not a complete game.**

Civ3Touch is not affiliated with or endorsed by Firaxis, 2K, GOG, FreeC3, or OpenCiv3/C7. Civilization III and related trademarks belong to their owners. No proprietary game data is included. Civ3Touch additions use GPL-3.0-only; see [LICENSE](LICENSE), [upstream attribution](UPSTREAM.md), and [licensing decision](docs/licensing.md).

## Build and try the prototype

Requirements: Rust 1.94.0 with the `aarch64-linux-android` target, JDK 21 (17+ supported by the build plugin), Android SDK platform 36/build-tools 35.0.0, NDK 27.2.12479018, Python 3.9+, and macOS or Linux. Gradle 8.14.3 is pinned by the included wrapper. Java/JNI and ARM64 are spike choices, not promises of final UI architecture or device support.

Set `JAVA_HOME`, `ANDROID_HOME`, and `ANDROID_NDK_HOME` for your installed tools. Ensure `cargo`, `rustup`, and `adb` are on PATH. Rust's pinned toolchain file installs its components/target through rustup. The spike builds Lua statically through mlua's vendored feature; system Lua is needed only for the separate unmodified upstream baseline.

```sh
cargo test --locked --workspace
cargo run --locked --bin smoke -- vendor/freec3/mods/base
android/gradlew -p android assembleDebug lintDebug
adb -s YOUR_DEVICE_SERIAL install -r android/app/build/outputs/apk/debug/app-debug.apk
python3 scripts/android-acceptance.py YOUR_DEVICE_SERIAL
python3 scripts/android-smoke.py YOUR_DEVICE_SERIAL
```

Launch → **Import Civilization III Complete** → select the copied English GOG installation root → wait for validation/import → **Play** → tap the Settler’s tile → tap a gold adjacent tile → **End Turn**. Copy the folder into a device folder such as `Documents/Civ3Complete`, including its GOG `.info` metadata. The picker cannot grant access to Android/data or the storage root on modern Android. The map follows the unit. The acceptance harness performs real touches and checks native state, rejected moves, reset, missing rules and recovery. The separate smoke harness retains Milestone 0 command/replay checks. Both existing harnesses require a debug APK and an ARM64 device/emulator; their explicit debug-only synthetic mode needs no game installation. Normal launches require import.

The sandbox uses a fixed generated 16×16 map and one player/Settler. End Turn skips unused movement. Rotation retains the session; app exit or process death discards it. Five original base terrain types and the Settler idle/run animations are supported. Other terrain/vegetation retain prototype markers; terrain transitions are not reproduced. Optional movement WAV and one ancient-era MP3 can be enabled with **Audio off/on**. Audio stops in the background. There is no save or opponent AI. See [Milestone 2 import contract and evidence](docs/milestone-2.md).

## Project layout and continuation

- `vendor/freec3/`: unchanged pinned upstream source; [revision and hashes](docs/freec3-baseline.json).
- `crates/civ3touch-core/`: platform-independent game session, smoke operation and local decoder probe; Android JNI adapter is target-gated.
- `android/`: Java map/input Activity, APK packaging, and Gradle wrapper.
- `scripts/`: native build, device smoke, inventory, and source-policy checks.
- `docs/`: [build instructions](docs/build.md), [architecture/integration](docs/android-integration.md), [GOG inventory](docs/gog-data.md), [test/CI evidence](docs/testing.md), and [milestone audit](docs/milestone-0-audit.md).
- `installer/`, `local-data/`, `.local/`, and build output: ignored local inputs and tools. See [asset policy](docs/asset-policy.md).

Canonical scope: [Milestone 2 Crew Brief](https://docs.google.com/document/d/1KpGp8DkI_fEKqae-VUopcJPklU1XAOrtN4EnfugxyjM/edit) and [roadmap](https://docs.google.com/document/d/149SobRRG_ZL3qq4fXuJ5KZG0R_l9vmf3NOmQ8W1TWlE/edit). Milestone 3 gameplay expansion requires a separate approved mission. Contribution rules are in [CONTRIBUTING.md](CONTRIBUTING.md).

## GitHub builds

After each successful push or merge to `main`, download the debug ARM64 APK and corresponding source from the [Actions runs](https://github.com/steelwater/civ3-touch/actions). See [CI and artifact policy](docs/github-actions.md) for checksums, retention, signing limitations, and merge protection.

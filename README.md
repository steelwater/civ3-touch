# Civ3Touch

An independent, open-source investigation of an Android client built around the FreeC3 simulation engine, using user-supplied GOG Civilization III Complete data. **Milestone 1 is a minimal interactive map prototype, not a complete game.**

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

Launch → **New Game** → tap the **S** Settler marker → tap a gold adjacent tile → **End Turn**. The map follows the unit. The acceptance harness performs real touches and checks native state, rejected moves, reset, missing rules and recovery. The separate smoke harness retains Milestone 0 command/replay checks. Both require a debug APK and an ARM64 device/emulator; no game installation is needed.

The sandbox uses a fixed generated 16×16 map and one player/Settler. End Turn skips unused movement. Rotation retains the session; app exit or process death discards it. There is no import, save, opponent AI or original artwork. See [Milestone 1 evidence and limits](docs/milestone-1.md).

## Project layout and continuation

- `vendor/freec3/`: unchanged pinned upstream source; [revision and hashes](docs/freec3-baseline.json).
- `crates/civ3touch-core/`: platform-independent game session, smoke operation and local decoder probe; Android JNI adapter is target-gated.
- `android/`: Java map/input Activity, APK packaging, and Gradle wrapper.
- `scripts/`: native build, device smoke, inventory, and source-policy checks.
- `docs/`: [build instructions](docs/build.md), [architecture/integration](docs/android-integration.md), [GOG inventory](docs/gog-data.md), [test/CI evidence](docs/testing.md), and [milestone audit](docs/milestone-0-audit.md).
- `installer/`, `local-data/`, `.local/`, and build output: ignored local inputs and tools. See [asset policy](docs/asset-policy.md).

Canonical scope: [Milestone 1 Crew Brief](https://docs.google.com/document/d/1zmHPomRr53cus_xb0WnbSXfWid9S9STP8YGHWO4kGG0/edit) and [roadmap](https://docs.google.com/document/d/149SobRRG_ZL3qq4fXuJ5KZG0R_l9vmf3NOmQ8W1TWlE/edit). Milestone 2 asset import requires a separate approved mission. Contribution rules are in [CONTRIBUTING.md](CONTRIBUTING.md).

## GitHub builds

After each successful push or merge to `main`, download the debug ARM64 APK and corresponding source from the [Actions runs](https://github.com/steelwater/civ3-touch/actions). See [CI and artifact policy](docs/github-actions.md) for checksums, retention, signing limitations, and merge protection.

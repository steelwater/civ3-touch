# Civ3Touch

An independent, open-source investigation of an Android client built around the FreeC3 simulation engine, using user-supplied GOG Civilization III Complete data. **The Milestone 7 brief records Milestones 1–6 as owner-accepted, with M5 limited to its resource foundation and M6 to research. [Milestone 7 Android quality of life](docs/milestone-7.md) awaits owner review and device acceptance; original saves, scenarios and complete Conquests rules remain unsupported.**

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

Launch → **Import Civilization III Complete** → select the copied English GOG installation root → wait for validation/import → **Play** → tap the Settler’s tile → tap a gold adjacent tile → **End Turn**. Copy the folder into a device folder such as `Documents/Civ3Complete`, including its GOG `.info` metadata. The picker cannot grant access to Android/data or the storage root on modern Android. Drag to pan, pinch to scale the map, and long press to inspect a tile or select stacked targets. Center returns to the selected unit. The acceptance harness performs real touches and checks native state, rejected moves, reset, missing rules and recovery. The separate smoke harness retains Milestone 0 command/replay checks. Both existing harnesses require a debug APK and an ARM64 device/emulator; their explicit debug-only synthetic mode needs no game installation. Normal launches require import.

The prototype uses a fixed generated 16×16 world with two civilizations and starting Settler, Worker and Warrior units. Select a unit to open its collapsible bottom action sheet for city founding, Worker improvements and combat. Use **Actions** for unit selection, touch-native city production/queues, research, read-only diplomacy information and save/load. Expand **Information** for economy and full turn feedback. End Turn runs the opponent's basic AI. Game rules remain in FreeC3.

**Save game** updates a manual slot. Actions → **Resume recovery save** restores the last completed action after a process restart; **Load saved game** restores the manual slot. Both are app-private and removed by uninstall/clear-data. Rotation and background/resume retain the live game. Saves are versioned Civ3Touch replays, not original Civ III saves.

Milestone 7 adds **Actions → Quick Save / Quick Load** with a separate confirmed quick slot. **Saves and backups** lists dated turn checkpoints, configures autosave cadence/retention, and imports/exports Civ3Touch JSON through the Android document picker. Recovery remains per completed action, and no save loads automatically. These files are not original Civ III `.sav` files.

**Game settings** adds persistent UI/font scaling, high-contrast light/dark themes, gesture choices and input help. Existing native save contracts remain unchanged. See [M7 controls, verification and limitations](docs/milestone-7.md); owner acceptance and hardware testing remain separate gates.

Five original base terrain types and the selected Settler's idle/run animations are supported. Other terrain, units and cities use prototype markers. Optional movement WAV and one ancient-era MP3 can be enabled with **Audio off/on** before play or through **Actions → Game settings**. See [Milestone 4 interaction, limits and playtest](docs/milestone-4.md) and the [preserved Milestone 2 import contract](docs/milestone-2.md).

## Project layout and continuation

- `vendor/freec3/`: pinned upstream source with [recorded resource patches](docs/freec3-resource-patches.json); [original revision and hashes](docs/freec3-baseline.json).
- `crates/civ3touch-core/`: platform-independent game session, smoke operation and local decoder probe; Android JNI adapter is target-gated.
- `android/`: Java map/input Activity, APK packaging, and Gradle wrapper.
- `scripts/`: native build, device smoke, inventory, and source-policy checks.
- `docs/`: [build instructions](docs/build.md), [architecture/integration](docs/android-integration.md), [GOG inventory](docs/gog-data.md), [test/CI evidence](docs/testing.md), and [milestone audit](docs/milestone-0-audit.md).
- `installer/`, `local-data/`, `.local/`, and build output: ignored local inputs and tools. See [asset policy](docs/asset-policy.md).

Canonical scope: [Milestone 7 Crew Brief](https://docs.google.com/document/d/1nyfnyX19tnue3dc0pYjsgpdWAQPrZWRM6zU0XVr2uqE/edit) and [roadmap](https://docs.google.com/document/d/149SobRRG_ZL3qq4fXuJ5KZG0R_l9vmf3NOmQ8W1TWlE/edit). The [M6 report and field mappings](docs/compatibility/README.md) cover read-only format research; the [M5 rules audit](docs/milestone-5.md) retains open gameplay-parity gaps. Contribution rules are in [CONTRIBUTING.md](CONTRIBUTING.md).

The [first resource slice](docs/milestone-5-resources.md) adds Wheat, Cattle, Gold and Horses to new prototype games, with worked-tile bonuses and technology-filtered resource labels. Long press a tile to inspect it. Existing M3-v2/M4 saves retain their original rules; new M5 saves require an M5-capable build. This remains partial resource support, without trade networks or luxury happiness.

## GitHub builds

After each successful push or merge to `main`, download the debug ARM64 APK and corresponding source from the [Actions runs](https://github.com/steelwater/civ3-touch/actions). See [CI and artifact policy](docs/github-actions.md) for checksums, retention, signing limitations, and merge protection.

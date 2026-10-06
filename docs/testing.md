# Civ3Touch verification and CI

## Milestone 3

Run the existing Rust format/Clippy/tests, synthetic inventory, vendor hashes, source policy and Android build/lint gates below. New core tests exercise economy, production queues, research, all three Worker improvements, basic AI/combat, full replay state comparisons, future outcomes and damaged saves.

On a disposable ARM64 emulator with the reviewed debug APK:

```sh
python3 scripts/android-import-acceptance.py YOUR_SERIAL local-data/gog/app
python3 scripts/android-loop-acceptance.py YOUR_SERIAL --imported
python3 scripts/android-acceptance.py YOUR_SERIAL --imported
python3 scripts/android-acceptance.py YOUR_SERIAL
python3 scripts/android-smoke.py YOUR_SERIAL
```

The loop harness performs actual touch/menu commands for movement, city founding, production and queueing, research, Worker roads, exploration and combat. It asserts engine snapshots from a debug-only app-private file; it never injects simulation state. It then verifies separate manual/recovery saves, force-stop/relaunch, background/resume and continued play. It overwrites this disposable app's manual/recovery slots. Omit `--imported` only for the explicit debug synthetic mode. Neither debug mechanism exists as a normal-launch import bypass.

Manual review remains required: inspect new city/unit/improvement markers, menu readability, mine/irrigation use, responsive phone/tablet/landscape and 1.5× text, rotation retention, and save error recovery. Restore emulator settings after checks. The [Milestone 3 record](milestone-3.md) provides the Captain's post-review playtest sequence and limits. Automated emulator evidence does not complete that acceptance gate.

Google's official testing-setup skill informs core regression tests, device journeys and lifecycle/layout checks. Existing Rust and adb/UI Automator infrastructure is retained; no new testing dependencies are needed. Play, R8, AGP migration and profiling skills are not applicable to this existing-stack gameplay integration.

## Milestone 2

Run the same Rust, inventory, source-policy, vendor-hash and Android build/lint gates below. Fifteen Rust tests now include synthetic GOG detection, required/optional-file handling, case resolution/collisions, symlink rejection, INI path restrictions, bounded malformed PCX/FLC failures, direction/ring-frame atlas behavior and native-facing reset after movement. CI needs no game data.

For developer-local real data and a disposable ARM64 API-36 test emulator:

```sh
cargo build --locked --bin validate_gog
cargo run --locked --bin validate_gog -- local-data/gog/app
adb -s YOUR_SERIAL install -r android/app/build/outputs/apk/debug/app-debug.apk
python3 scripts/android-import-acceptance.py YOUR_SERIAL local-data/gog/app
python3 scripts/android-acceptance.py YOUR_SERIAL --imported
python3 scripts/android-acceptance.py YOUR_SERIAL
python3 scripts/android-smoke.py YOUR_SERIAL
```

The import harness uses the actual system picker. It copies only profile files from the source into unique emulator `Documents/Civ3Touch-*` folders and tests valid, missing, unsupported and malformed variants, unchanged active data after failure, staging cleanup and process restart. **It replaces this app’s active import; use a disposable emulator, not an installation whose data you want to keep.** Input files stay untouched. Device test folders remain until the disposable session ends. The harness assumes an English phone-sized DocumentsUI with a Documents breadcrumb; it is not a portable test runner for every provider/locale. Do not upload game data or original-asset screenshots as test artifacts.

`--imported` exercises touch selection, rejected distant moves, one valid move, end turn, reset, missing-rule diagnosis and recovery with installed original assets. It also checks native direction and Android's parsed atlas column on initial launch, after movement and after New Game. Without it, the existing harness explicitly uses a debug-only synthetic launch extra. Normal app launches never bypass import.

Manual checks: cancel folder selection; verify clean launch gates Play; inspect original terrain and animated Settler; toggle audio, move and inspect debug `AUDIO_*` events for decoder readiness, music start/pause and successful sound stream; background/resume; rotate; inspect phone/tablet/landscape and 1.5× text. Verify retained game state and import progress after configuration changes. Restore emulator display/settings afterwards. Actual listening, physical-device/API-26 and alternative-provider testing must be reported separately.

Current results and scoped limitations: [Milestone 2 record](milestone-2.md).

## Milestone 1

Use the environment in `build.md`, then run:

```sh
cargo fmt -p civ3touch-core --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 scripts/test_inventory.py
python3 scripts/verify-upstream.py
python3 scripts/check-source-policy.py
android/gradlew -p android assembleDebug lintDebug
adb -s YOUR_SERIAL install -r android/app/build/outputs/apk/debug/app-debug.apk
python3 scripts/android-acceptance.py YOUR_SERIAL
python3 scripts/android-smoke.py YOUR_SERIAL
```

The acceptance harness launches the installed app, taps New Game, selects the actual drawn Settler, submits a rejected distant move, moves one engine-advertised tile, and advances to turn 2. It checks UI descriptions/status and bounded native debug evidence, then tests reset, missing rules and recovery. It does not call a test-only gameplay shortcut. The old smoke harness uses a debug-only `smokeTest` launch extra. Proprietary data is never needed.

Manual checks: verify selected/gold tile outlines and unit relocation, rotate during a game and confirm turn/position/selection survive, background/resume, retry after a missing-rules error, and inspect phone/tablet/landscape plus 1.5× text. Status text scrolls in a bounded panel. Device display settings must be restored after tests. Physical-device and accessibility completeness are separate limits, not inferred from an emulator pass.

Current results and known limitations: [Milestone 1 record](milestone-1.md). The existing `core-and-android` CI job runs the expanded Rust tests and Android build/lint on PRs. Device acceptance is local; CI uses synthetic data only.

Google's current [testing/testing-setup skill](https://github.com/android/skills/blob/main/testing/testing-setup/SKILL.md) informed the stack inspection, core behavior tests, real native/device acceptance, lifecycle and layout checks. This project uses platform Views/Canvas, Rust tests and an adb/UI Automator dump/input harness. No DI, Java mocking, database, coverage or screenshot framework is installed. Broad framework installation is excluded by the handbook's dependency/minimal-scope rules. Play, R8, performance and navigation skills are not material to this small existing-shell slice.

## Historical Milestone 0 evidence


The highest-risk seams are unchanged upstream behavior, Lua/native cross-compilation, JNI error handling, command/replay behavior, and metadata-only inspection of local game files.

## Commands and expected outcomes

From the repository root with the tool environment in `build.md`:

```sh
(cd vendor/freec3 && cargo build --locked --workspace)
(cd vendor/freec3 && cargo test --locked --workspace)
(cd vendor/freec3 && cargo clippy --locked --workspace -- -D warnings)
(cd vendor/freec3 && cargo run --locked -p fc3_headless -- --max-turns 2 --quiet)
cargo fmt -p civ3touch-core --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked --bin smoke -- vendor/freec3/mods/base
python3 scripts/test_inventory.py
python3 scripts/verify-upstream.py
python3 scripts/check-source-policy.py
python3 scripts/inventory-gog.py local-data/gog/app > docs/gog-inventory.json
cargo run --locked --bin validate_gog -- local-data/gog/app
android/gradlew -p android clean assembleDebug lintDebug
adb -s emulator-5554 install -r android/app/build/outputs/apk/debug/app-debug.apk
python3 scripts/android-smoke.py emulator-5554
```

`emulator-5554` is the observed local test serial, not a portable device identifier. The final clean Rust validation uses `CARGO_TARGET_DIR="$PWD/.local/validation-target"`; normal builds use `target/`. Host baseline commands need external Lua; the Civ3Touch commands use statically built vendored Lua.

## Observed evidence, 2026-10-02

- Unmodified upstream workspace: 468 tests passed, 3 FLIC tests ignored by upstream. Full workspace build and Clippy with warnings denied passed. Headless runner completed the configured short game.
- Civ3Touch: two Rust tests passed (rules/commands/replay and missing rules); own formatting and Clippy passed.
- Metadata inspector: three synthetic tests passed (empty installation, metadata without payload, escaping symlink). 115 upstream source checksums matched.
- GOG: nine required paths present; six terrain PCX and two Settler FLC files decoded. Full inventory contains 9,007 metadata rows.
- Android: ARM64 debug APK compiled and lint passed. API-36 emulator logged success, expected missing-rules failure, then successful recovery.
- UI: success and failure states inspected; phone, tablet-sized and landscape layouts and 1.5× text scaling checked on the emulator. This is a technical status screen, not a gameplay usability test.
- Installer SHA-256 rechecked; source-path policy and APK entry inventory inspected for accidental proprietary payloads.

The unchanged upstream formatter check (`cd vendor/freec3 && cargo fmt --all --check`) fails against Rust 1.94 formatting in seven source files. This is documented baseline drift; the pinned snapshot remains byte-identical. Do not apply formatting fixes to vendor files as part of this mission.

Android lint reports zero errors and four warnings: newer Gradle available, no x86_64 ChromeOS ABI, no modern data-extraction rule, and no application icon. Gradle also reports deprecated features ahead of Gradle 9. The spike is not being published; these do not establish production readiness. No lint baseline or suppression was added.

## CI

`.github/workflows/ci.yml` provides one `core-and-android` job: install pinned prerequisites; test upstream core/decoders/headless; check own formatting, Clippy and tests; run synthetic path tests and source/baseline checks; compile and lint the Android app. It uses no GOG data. Desktop GUI compilation is observed locally; the CI job deliberately targets the Android/core seam.

The original Milestone 0 audit preceded remote setup. The current public repository and required `core-and-android` protection are documented in [github-actions.md](github-actions.md). Required checks must pass on each new PR head; earlier milestone evidence does not satisfy that gate. Emulator verification remains a local/manual gate, not CI device coverage.

## Skills and remaining limits

Google's official `android/skills/testing/testing-setup` was read. Applied guidance: inspect the stack, test shared behavior directly, exercise the real native boundary on-device, and document responsive/failure checks. Its blanket DI/coverage/screenshot framework additions were not adopted because the brief and handbook require minimal dependencies. Security/performance/R8/Play/navigation skills were not material to this unsigned feasibility app; no store/release or navigation flow exists.

No physical device, API-26 runtime, x86_64, 16 KB-page device, visual PCX/FLIC rendering, complete game import, lifecycle game persistence, fuzzing, or original-save compatibility check was performed. Runtime evidence covers the selected ARM64 API-36 emulator and the stated local GOG samples only.

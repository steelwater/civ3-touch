# Milestone 0 verification and CI

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

There is no configured remote or PR. Consequently GitHub Actions execution and required-check protection have **not** been verified. Local equivalents passed on macOS; the Ubuntu workflow itself remains unrun until an approved Uplink establishes a remote. Emulator smoke is presently a local/manual gate, not a claim of CI device coverage.

## Skills and remaining limits

Google's official `android/skills/testing/testing-setup` was read. Applied guidance: inspect the stack, test shared behavior directly, exercise the real native boundary on-device, and document responsive/failure checks. Its blanket DI/coverage/screenshot framework additions were not adopted because the brief and handbook require minimal dependencies. Security/performance/R8/Play/navigation skills were not material to this unsigned feasibility app; no store/release or navigation flow exists.

No physical device, API-26 runtime, x86_64, 16 KB-page device, visual PCX/FLIC rendering, complete game import, lifecycle game persistence, fuzzing, or original-save compatibility check was performed. Runtime evidence covers the selected ARM64 API-36 emulator and the stated local GOG samples only.

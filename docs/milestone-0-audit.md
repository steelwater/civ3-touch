# Civ3Touch — Milestone 0 results and audit

Date: 2026-10-02 (Asia/Tokyo). Owner: Dan. **Technical finding: GO for planning Milestone 1.** The local feasibility proof is complete; public remote/CI execution and Captain acceptance remain separate gates.

## Mission and authority

Canonical [Milestone 0 Crew Brief](https://docs.google.com/document/d/1PmYEMGO8M3bInm-TJz_1Kq9a8DWjuNDFUr_58I6Z8ho/edit) and [roadmap](https://docs.google.com/document/d/149SobRRG_ZL3qq4fXuJ5KZG0R_l9vmf3NOmQ8W1TWlE/edit). Dan explicitly approved GPL-3.0 and the minimal Java/JNI spike on 2026-10-02. No engine change, Windows compatibility layer, gameplay expansion, remote publication, or release was performed.

## Outcome

FreeC3 executes from a native Android test app on an ARM64 API-36 emulator. The test loads upstream Lua rules from Android private storage, initializes a seeded world and units, rejects a wrong-player command, skips units, advances to turn 2, serializes/replays the command log, and verifies both player views. The Java screen reports PASS. Missing rules produce a visible failure and Java exception; a subsequent normal launch recovers.

Six GOG terrain PCX files and the Settler idle/run FLC files decode with the unchanged FreeC3 byte decoders. The installer and 9,007 extracted files remain ignored developer-local inputs. No known architectural blocker was found for this narrow GOG data path. This is not evidence of complete Conquests mechanics, original-save support, asset rendering, or a playable game.

## Baseline and repository foundation

- Upstream: https://github.com/andrewimm/FreeC3
- Exact revision: `90fc7eeda2d1914c9306b33a161686fa839b8455`.
- Mechanism: complete unmodified source snapshot at `vendor/freec3`, original license/README preserved; 115 per-file hashes in `docs/freec3-baseline.json`.
- Shared wrapper/data probe: `crates/civ3touch-core`; Android Java/JNI shell: `android`; build/check scripts: `scripts`.
- GPL-3.0-only additions, `LICENSE`, `UPSTREAM.md`, `CONTRIBUTING.md`, proprietary-data policy, copied dependency notices, and Android-target dependency license inventory prepared.
- `.github/workflows/ci.yml` builds/lints the Android configuration and runs core/synthetic tests without proprietary inputs. No remote exists; actual GitHub Actions and branch-protection checks are unrun.

## Architecture observations

Paths below refer to the exact upstream revision above.

| Area | Evidence and integration consequence |
| --- | --- |
| Simulation | `crates/fc3_core/src/engine/mod.rs`: `Engine::new_game`, `submit_command`, and `player_view` provide a natural explicit boundary. The engine holds `Rc<RefCell<World>>`; do not assume it can be shared across Android threads. |
| Commands/events | `crates/fc3_core/src/protocol.rs` defines the request/result/view types. Keep game-rule ownership in Rust. A small JNI adapter can call a scoped operation without introducing Android into the engine. |
| Lua | `crates/fc3_core/Cargo.toml` uses `mlua` 0.10 with `lua54` and `serialize`, without `vendored`. The host baseline needs Lua 5.4 development/linker support; Android needs Lua built for the selected target. |
| Filesystem | Contrary to a literal reading of the README's no-I/O description, `scripting/mod.rs::load_mod` canonicalizes and reads real paths. Relative mod discovery walks the working directory. Android should supply an absolute directory containing the upstream Lua files. Assets packaged inside an APK are not ordinary filesystem paths. |
| Serialization | Core data/protocol types use Serde. `GameLog` contains seed, configuration, and commands; replay creates a fresh engine and resubmits commands. This is not proof of original Civ III save compatibility or complete live Lua-state serialization. |
| PCX | `fc3_pcx` has no external crate dependencies and accepts byte slices. It decodes indexed PCX to RGBA and supports caller-selected transparent palette indices. Desktop atlas placement is separate. |
| FLIC | `fc3_flic` has no external crate dependencies and accepts byte slices. It reads FLC magic `0xAF12`, Civ III animation fields, palette and delta chunks, and outputs RGBA frames. The selected local GOG Settler idle/run files decoded successfully. |
| Desktop | `fc3_desktop` uses wgpu/winit/glyphon. These are presentation dependencies, not required by `fc3_core`. It takes `--resource-dir` or `RESOURCE_DIR`. Avoid pulling this client into the JNI spike. |
| Rules and assets | `mods/base` supplies upstream Lua rules. The engine can initialize a synthetic map without proprietary data. Desktop art paths include case-sensitive-looking names such as `Art/Units/Settler/settler.INI`; the GOG Settler INI uses lowercase `.ini`, requiring explicit resolution on Android. |

Likely extension points: an Android-only JNI adapter, explicit platform-independent asset resolution, existing byte-buffer decoders, and a caller-provided Lua directory. The approved JNI adapter and caller-provided Lua directory are implemented; a general GOG resolver is deferred. No engine rewrite is justified by the source inspection so far.

## Licensing decision

**Context:** the actual FreeC3 root LICENSE is GNU GPL version 3; README declares GPL-3.0. Individual inspected manifests do not provide a different grant. **Decision:** Dan approved GPL-3.0; Civ3Touch additions declare GPL-3.0-only while preserving the upstream text without inferring an explicit “or later” grant. **Alternatives:** a permissive-only combined derivative or engine switch were not adopted. **Consequences:** preserve notices, identify modifications, distribute covered source consistently, and satisfy corresponding-source/build/installation-information requirements applicable to any future binary distribution. **Owner/date:** Dan, 2026-10-02.

`docs/licensing.md` records the decision; `docs/dependency-licenses.json` and `third-party-notices/` record the resolved Rust graph and notices. The Gradle wrapper's Apache license/notice and embedded Lua copyright are preserved. No binary publication is authorized; release-specific compliance remains a later gate.

## GOG reference and compatibility

- Input: `installer/setup_civilization3_complete_2.0.0.7.exe`; 1,370,659,544 bytes.
- SHA-256 before/after: `4ad54ca308ea93af49b0db3a795736eda91aed61c8cd73b18196802d0f4d2395` (unchanged).
- Extractor: innoextract 1.9; package title Sid Meier's Civilization III Complete; Inno Setup 5.5.0 Unicode; GOG game/root ID 1471405734, English.
- `2.0.0.7` remains the observed filename identifier; no separate internal GOG build-number field was verified. Base `Text/version.txt` reports 1.29f, not asserted to be the Conquests version.
- Extracted layout: base Art/Text/Sounds plus civ3PTW, Conquests and support/GOG files. Metadata inventory: 9,007 files, 1,887,005,990 bytes.
- Required image probe: `Art/Terrain/{xggc,xtgc,xdgc,xdgp,xdpc,xpgc}.pcx` (all 1152×576); `Art/Units/Settler/settler.ini`, `settDefault.flc` (30×55, eight directions, 15 frames/direction), `settRun.flc` (40×63, eight directions, 10 frames/direction).
- Concrete mismatch: upstream expects `settler.INI`; GOG provides `settler.ini`. Future Android import must resolve actual case and reject collisions. The probe uses exact observed paths.
- Core initialization uses upstream Lua, not original BIQ files. Other art/audio is optional for later work; BIQ/scenario/save compatibility remains unknown/deferred. Windows executable runtime use is unnecessary for this proof.

See `docs/gog-data.md` and metadata-only `docs/gog-inventory.json`. No proprietary payloads were uploaded, committed, or packaged.

## OpenCiv3/C7 reference audit

Reference revision: [`6b9100d989f7634d6d57f947d73aac46032e59eb`](https://github.com/C7-Game/OpenCiv3/tree/6b9100d989f7634d6d57f947d73aac46032e59eb). Root LICENSE is MIT, copyright OpenCiv3 contributors. No code has been copied into Civ3Touch.

- `ConvertCiv3Media/ReadPcx.cs`: useful for comparing row padding, indexed pixels, and the trailing palette when validating FreeC3 against GOG samples.
- `ConvertCiv3Media/ReadFlic.cs`: useful for direction sequences, ring frames, sprite offsets, and differences between unit animations and leaderheads. It contains explicit uncertainties; it is a reference, not a compatibility guarantee.
- `QueryCiv3/Biq.cs`: demonstrates structured BIQ rules/map loading as a separate compatibility area. It does not establish that FreeC3 already loads Conquests rules.

## Integration decision

A minimal Java Activity invokes an Android-only JNI adapter around a platform-independent Rust smoke function. This avoids prematurely porting the desktop renderer or generating a broad binding API. The engine lives on one worker thread for one call; game rules remain in FreeC3. Packaged open-source Lua is copied to an explicit private filesystem directory. Rust errors/panics become Java failures. Persistent sessions and production lifecycle behavior are deferred.

Pinned tools: Rust 1.94.0; ARM64 Android API-26 linker from NDK 27.2.12479018; compile/target SDK 36; AGP 8.10.1; Gradle 8.14.3; local JDK 21.0.10. Host unmodified baseline used Lua 5.4.8; the cross-platform spike uses statically vendored Lua 5.4.7. The ARM64 library has 0x4000 ELF load-segment alignment; actual 16 KB-page-device testing remains unrun.

## Validation evidence and exact commands

Full tool setup is in `docs/build.md`; all commands below run from the repository root after setting the documented environment. Baseline commands additionally used `LUA_LIB=/tmp/civ3touch-audit/lua-5.4.8/src`, `LUA_LIB_NAME=lua`, `LUA_LINK=static` for this session's locally built library.

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

Final clean Rust tests, host smoke, decoder probe and Android build used `CARGO_TARGET_DIR="$PWD/.local/validation-target"` to avoid relying on prior Rust outputs. Android build output was cleaned. The resulting APK was installed and the device smoke repeated.

Results: 468 upstream tests pass, three upstream FLIC tests ignored; full baseline build/Clippy/headless execution pass; two Civ3Touch Rust tests and three synthetic inventory tests pass; own formatter/Clippy pass; all 115 upstream hashes match; eight real-data decodes pass; Android build/lint and success/failure/recovery checks pass. APK ZIP inspection found only 15 upstream Lua assets and the ARM64 core library, with no game payloads. Installer checksum remained unchanged. Phone/tablet/landscape status screens and 1.5× text scaling were inspected on the emulator; display settings were restored.

Local final debug APK SHA-256: `c016d5c8f4415534ece3d03abcb4c2433d9194e9bb81165d7cac7ffc4ce23183`. This is a development artifact, not an approved release or durable release archive.

## Deviations, limits and next action

- Upstream `cargo fmt --all --check` fails against Rust 1.94 formatting in seven existing files. The source snapshot remains unchanged; own formatting passes.
- Android lint has zero errors and four warnings: newer Gradle available, no x86_64/ChromeOS ABI, no modern data-extraction rule, and no icon. No checks were disabled or baselined away.
- Actual remote GitHub Actions, required-check protection, physical-device/API-26/16 KB-page-device tests, original BIQ/save compatibility, and rendered game assets are unverified. No remote/PR was provided or created; local CI-equivalent commands passed on macOS, but the Ubuntu workflow remains unrun.
- Google's official `testing/testing-setup` skill informed stack inspection, shared-core tests, actual native execution, and failure/layout checks. Broad DI, screenshot and coverage frameworks were excluded by the explicit minimal-scope/dependency rules. Performance, Play, R8 and navigation skills did not apply to this unsigned status-only spike.
- A first smoke attempt hit FreeC3's real rule that all units must be handled before ending a turn. The wrapper now issues SkipUnit commands; upstream behavior was preserved. A theme attribute incompatible with API 26 was removed after lint identified it.

**Go/no-go:** GO for a separately approved Milestone 1 brief on the demonstrated technical path. No known fundamental architecture, format, or licensing blocker remains for that narrow next step. This does not approve public release or certify complete game compatibility. Next operational step is Captain review and, when authorized, Uplink to a chosen GitHub repository and verification of the new CI job. No Milestone 1 implementation has begun.

Rollback: original installer unchanged; extracted data/toolchains/build outputs are ignored and local; the upstream snapshot is independently hash-verifiable. No existing source was replaced, no remote state changed, and no destructive cleanup occurred.

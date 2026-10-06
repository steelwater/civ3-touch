# Milestone 2 — GOG import and original assets

Date: 2026-10-06 (Asia/Tokyo). Owner: Dan. Canonical [Crew Brief](https://docs.google.com/document/d/1KpGp8DkI_fEKqae-VUopcJPklU1XAOrtN4EnfugxyjM/edit). Branch: `codex/milestone-2-gog-import`, based on `main` at `198ca12`. The brief authorizes implementation and focused Uplink, not merge, release or deployment.

## Implementation contract

A clean normal launch requires Import → system folder selection → source-file validation → private staging copy → GOG marker/media validation → atomic publication → Play. The existing generated world, Lua rules and FreeC3 game commands remain unchanged. No Windows program is copied or executed. No dependency was added and the vendor snapshot remains untouched.

`crates/civ3touch-core/src/assets.rs` is the reusable platform-independent profile, case-insensitive local resolver, validator and PCX/FLC loader. Android receives an allowlisted path/limit manifest and decoded ARGB images over JNI. It owns SAF permissions, private storage, Bitmap/Canvas presentation and MediaPlayer/SoundPool playback. Proprietary format knowledge does not live in Android UI or the simulation engine.

The reference is the locally inspected English GOG installer `setup_civilization3_complete_2.0.0.7.exe`, SHA-256 `4ad54ca308ea93af49b0db3a795736eda91aed61c8cd73b18196802d0f4d2395`. `2.0.0.7` is its filename identifier, not a separately verified internal build number. Detection checks GOG game/root IDs `1471405734`, language `english`, base version marker `1.29f`, Conquests readme signature/version `v1.22`, expected Settler INI references and supported media headers/decoding. This identifies the verified layout; it is not a cryptographic authenticity or ownership check. Steam/CD, other languages, mods and other layouts are not supported or inferred compatible.

## Required copied layout

Select the installation root (the folder containing `Art`, `Text`, `Conquests` and the GOG `.info` file). Case differences are accepted; duplicate case-insensitive names fail clearly. Do not select only `Art`, an installer EXE, the parent of the installation, or the `Conquests` subfolder.

| Path relative to root | Purpose |
| --- | --- |
| `goggame-1471405734.info` | GOG identity and language |
| `Text/version.txt` | Base 1.29f marker |
| `Conquests/readme.txt` | Conquests layout/version marker |
| `Art/Units/Settler/settler.ini` | Expected idle/run references |
| `Art/Terrain/xggc.pcx`, `xtgc.pcx`, `xdgc.pcx`, `xpgc.pcx` | Grassland, Coast, Tundra, Desert and Plains |
| `Art/Units/Settler/settDefault.flc`, `settRun.flc` | Eight-direction idle and movement animation |
| `Art/Units/Settler/SetRunFoot1.wav` (optional) | One movement sound |
| `Sounds/Build/ancient/AncECfull.mp3` (optional) | One ancient-era music track |

The profile is the executable source of truth for required paths and byte limits. `cargo run --locked --bin validate_gog -- --profile` emits it as JSON. `cargo run --locked --bin validate_gog -- local-data/gog/app` validates the local source and prints technical metadata only.

The minimal profile copies roughly 6 MB from the reference; it does not copy the entire 1.9 GB installation. Allow at least 32 MB free for bounded staging and replacement. All mandatory paths are located before copying. Required unreadable, missing, unsupported, ambiguous or malformed files prevent publication with a path-specific diagnosis. Optional audio may be absent or rejected without disabling Play; Android media playback failure is reported separately.

## Storage and lifecycle

Only user-selected SAF streams are read. No broad storage permission, retained folder grant, network permission, analytics or account is required. Source files are never altered. Profile paths determine private destinations; untrusted INI content cannot redirect reads. Local Rust resolution rejects symlinks. Imports are in `getNoBackupFilesDir()/imports`, and application backup is disabled.

Each attempt has a unique staging directory, per-file stream limits and a bounded total profile. Shared Rust validation and complete Bitmap creation must succeed before an fsynced pointer is atomically replaced in the same filesystem. Failed attempts preserve the active import; handled failures discard their staging. Abandoned staging/pointer files from a killed process are discarded on the next attempt. A process killed after a generation rename but before publication can leave an unused private `data-*` directory; it is never treated as an active import. A successful replacement removes the previous generation. Clearing app data/uninstalling removes imported copies; the user’s source remains available for reimport.

On launch the active data is decoded/revalidated. Corruption produces an import error and disables Play until reimport. Rotation retains the controller; its worker owns the native session and import. Process death requires restarting the prototype game, but a completed import persists. Audio starts only when enabled and a game is foreground, pauses in the background and releases on exit. Audio starts off each process session.

## Deliberate limits

This proves a narrow complete data path. Terrain uses uniform cells sampled from four original atlases, not full neighbor transitions. Ocean, Ice, Hill, Mountain and vegetation retain the prior colored/letter prototype presentation. Settler idle/run use all eight direction sequences and omit ring frames; clipped sprites are anchored at the tile’s feet rather than reproducing the full original animation canvas. Movement sound uses a selected footstep sample, not the original AMB sequencer. Music is one supported original track, not an era playlist. No other units, original rules/BIQ, scenarios, saves, combat or broader game-loop features are added.

The pinned PCX/FLIC decoders are behind header/size/structure checks; malformed decode panics become failures, not JNI unwinds. These bounded checks and synthetic tests are not a complete hostile-file fuzzing audit. Metadata markers do not authenticate file contents.

## Verification and continuation

Observed checks on the final implementation:

- Fourteen Rust tests pass (nine asset-boundary tests plus the five existing engine/smoke tests); own rustfmt and Clippy with warnings denied pass. Three synthetic inventory tests, source policy and all 115 vendor hashes pass.
- The developer-local validator decodes five terrain samples and eight-direction idle/run atlases from the reference data, with both optional audio paths recognized.
- Android `assembleDebug lintDebug` passes. Lint has zero errors and five warnings: four inherited toolchain/ABI/backup/icon advisories, plus conservative `getUsableSpace` preflight. The preflight deliberately measures currently free bytes without asking Android to evict other apps’ cache; stream failures still abort staging safely. No lint checks were suppressed.
- API-36 ARM64 read-only emulator: clean launch disables Play; actual folder selection, validation, private import and Play pass. The import harness passes missing-file, wrong-GOG-identity and malformed-PCX diagnoses, unchanged active record and staging cleanup after failure, and process-restart recovery.
- Touch acceptance with imported assets passes selection, rejected distant move, legal one-tile movement, end turn, reset, missing rules and recovery. The pre-existing synthetic touch and JNI smoke harnesses also pass.
- Imported terrain and Settler rendering were visually inspected. Game position/selection/movement survived background/resume, phone landscape, tablet dimensions and 1.5× text. Phone 1080×2400/420 dpi and tablet 1600×2560/240 dpi were used, then display settings restored. Debug evidence confirms MP3/WAV readiness, music started/paused/resumed and a successful footstep stream.
- APK inventory contains exactly the 15 upstream Lua asset files plus the native library, with no proprietary media/executables. Final local APK SHA-256: `f6c26af4b43085978055f49c4629df502e632fe100d9c546eddd7f3e23716ee2`. This is a local development APK, not a release artifact.
- Final diff reviewed. Remote `main` was unchanged at `198ca12` during Uplink preflight; protection requires an up-to-date `core-and-android` result. PR/head/CI completion is recorded in the Drive handoff after Uplink.

The first media check found a pause-before-start error; playback now pauses only a playing MediaPlayer, and final lifecycle checks pass. Two harness issues with DocumentsUI breadcrumb accessibility and stale picker tasks were corrected before the successful full import run.

Unrun checks: physical hardware, API 26/16 KB-page devices, subjective audio listening, non-English/alternative document providers, forced low-space/permission-revocation/power-loss injection and full accessibility/fuzzing. No evidence from those checks is claimed. No gameplay or compatibility expansion beyond the documented slice. Google’s current [Android testing-setup skill](https://github.com/android/skills/blob/main/testing/testing-setup/SKILL.md) informed shared tests, device journeys, failure states and layout/lifecycle checks. Existing Rust and adb/UI Automator tools were reused; no DI, mocking, screenshot or coverage dependency was installed. Play, R8 and profiling skills are not applicable to this import slice.

Rollback: changes remain on the feature branch; canonical `main`, source installation and installer remain unchanged. No vendor modifications, production deployment or release. Next gate is PR review and Captain playtesting; merge is separate.

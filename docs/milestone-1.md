# Milestone 1 — interactive Android vertical slice

Date: 2026-10-05 (Asia/Tokyo). Owner: Dan. Scope: [canonical Milestone 1 Crew Brief](https://docs.google.com/document/d/1zmHPomRr53cus_xb0WnbSXfWid9S9STP8YGHWO4kGG0/edit). Branch: `codex/milestone-1-vertical-slice`, based on `main` at `3a30a23`. The brief authorizes focused Uplink; merge, production deployment and release are separate gates.

## Outcome and implementation

The installed ARM64 debug APK completes New Game → rendered map/Settler → touch selection → one legal tile → End Turn. A gold ring marks selection and gold outlines/dots mark the engine's immediate move options. The camera follows the Settler. Terrain, fog and units come from the native player view; the artwork is simple original Canvas geometry.

The Rust `Session` wraps unchanged FreeC3. It asks for available moves and paths, accepts only paths with one step, submits the movement command, and returns engine events/errors plus the refreshed view. Invalid unit generations, out-of-bounds/distant/current-tile and exhausted movement requests do not enqueue later movement. End Turn explicitly skips unused unit movement through commands, then submits EndTurn.

The existing Java/JNI stack is extended with no new dependencies. A retained controller owns one worker thread and its thread-local native engine. Rotation/configuration changes preserve the session and selection, and background/resume preserves the game while the process lives. Exiting or process death discards it. Build/API/ABI choices remain unchanged; debug app metadata is `0.1-m1` / code 2, not a release tag.

## Prototype boundaries

- Fixed seed 42, generated 16×16 map, one player and one Settler. New Game restarts that sandbox. There is no opponent AI, victory flow, city founding, combat, save system, map pan/zoom or asset import.
- Single-player sandbox turn sequencing deliberately returns to that player's next numbered turn. This is not evidence of complete Civilization III mechanics.
- The view assumes the one starting unit remains alive; destruction and multi-unit selection are outside this mission.
- Status text is scrollable at large font scales. Canvas tiles have a summary accessibility description; full TalkBack tile navigation and keyboard gameplay are not implemented.
- No proprietary assets, installer or game payloads were added, packaged, uploaded or used for these tests. All 115 vendor files remain identical to the recorded baseline. No new durable tools/frameworks were installed.

## Verification

Commands and manual reproduction are in [testing.md](testing.md) and [build.md](build.md).

- Five Rust tests pass: existing smoke/missing-rules checks plus starting-unit/one-step/turn flow, rejected/stale/exhausted requests, and skip/reset behavior.
- Civ3Touch rustfmt and Clippy with warnings denied pass; three synthetic inventory tests, source policy and all vendor hashes pass.
- Android `assembleDebug lintDebug` passes with zero errors and the four existing warnings (Gradle update, ChromeOS ABI, backup-rule modernization, icon). Java/Gradle also report existing toolchain/API deprecation warnings. No checks were weakened or baselined away.
- API-36 ARM64 emulator: installed APK acceptance harness passes actual touch selection, rejected two-tile destination without mutation, one `UnitMoved` event to the requested tile, turn 2 with movement refreshed, New Game reset, missing-rules error and subsequent recovery. The separate JNI smoke success/failure/recovery harness passes.
- Manual/device checks: position, turn and selection survive landscape rotation, tablet-sized configuration, 1.5× font scaling and background/resume. Phone 1080×2400 at 420 dpi and tablet-sized 1600×2560 at 240 dpi were inspected; emulator display settings were restored. These are emulator configurations, not physical-device evidence.
- Final local debug APK SHA-256: `31f2e59e17f12bd964d952cf653e01eeda3fd476952b68a57ed824e6cae09bd7`. This is a local test artifact, not an approved release.
- APK ZIP inspected: 15 open-source Lua assets and the ARM64 core library; no proprietary payloads.
- Read-only remote preflight confirms `main` is unchanged and branch protection requires an up-to-date `core-and-android` check, including administrators. PR/CI results are recorded with the Uplink handoff after completion.

Google's official Android `testing/testing-setup` skill informed the checks; existing Rust and adb/UI Automator harnesses were reused under the handbook's minimal-dependency boundary. Play, R8, navigation and profiling skills were not applicable. Physical Android hardware, minimum API 26, 16 KB-page devices, full accessibility, complete gameplay and original-data compatibility remain unverified/out of scope.

The first harness attempt exposed truncated debug logging; evidence is now bounded to turn, unit, tile count, move options and results. ADB daemon interruptions were resolved with a foreground ADB session, and the complete acceptance harness was rerun successfully. Neither issue required vendor changes.

## Rollback and next step

All work is isolated on the feature branch. The canonical branch and proprietary local inputs are untouched. Review the focused PR and playtest the APK; merge and Milestone 2 remain Dan's decisions. No release, deployment, destructive cleanup or branch deletion is performed.

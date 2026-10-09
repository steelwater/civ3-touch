# Milestone 7 — Android quality of life

Date: 2026-10-10 (Asia/Tokyo). Owner: Dan. Status: implementation submitted for review; acceptance requires owner review and post-merge device playtesting. Authority: [M7 Crew Brief](https://docs.google.com/document/d/1nyfnyX19tnue3dc0pYjsgpdWAQPrZWRM6zU0XVr2uqE/edit). Branch: `codex/milestone-7-android-qol`, from clean `main` at `d99d77d`. M1–6 are owner-accepted according to this brief; M5 remains a resource foundation and M6 research-only. Dan confirmed the established remote `steelwater/civ3-touch` for the PR (the brief omits the hyphen).

## Baseline audit before modification

- `Session::save/load`: JSON envelope v1, exact Lua identity, M3-v2 and M5 resource rules contracts, maximum 16 MiB / 20,000 replay actions. Loading reconstructs a separate session, compares command results, full world, queues, player and turn; JNI replaces the current session only after success. No migration, original SAV or BIQ loading.
- `GameSaves`: app-private `no_backup/saves/manual.json` and `recovery.json`; debug synthetic sessions have separate `synthetic-saves`. AtomicFile, fsync, readback after commit. A post-commit verification error cannot roll back the old value: move verification before commit. No slot metadata, retention, cadence, Quick Save/Load, or SAF save transfer.
- `GameController`: single worker owns native state. Recovery is written after each successful native action before publishing the snapshot. Rotation retains controller, camera and selection. Resume only changes audio state. Process recreation starts at the welcome screen; recovery requires explicit action. No guarantee an in-flight action survives process death. Save failure warns while keeping the playable in-memory state.
- `MainActivity`: no broad storage/network permissions; artwork imports use SAF. It retains no document grants. Native state is never stored in an Android Bundle. Activity recreation returns open menus to the map.
- `TouchUi/MapView`: 48 dp minimum controls, scrollable menus, bounded collapsible panels, Android system font scaling, fixed light theme. Camera pan/pinch/selection retained in process. Map labels lack backing contrast. Imported Settler animation schedules redraws while window-visible, without a separate foreground/menu gate. No configurable gestures or verified external-device controls.
- Test stack: Rust regressions, plain JDK camera tests, Python/adb/UI Automator journeys. No DI, database, mocking or screenshot framework. Google's [testing-setup skill](https://github.com/android/skills/blob/main/testing/testing-setup/SKILL.md) informs boundary, lifecycle, configuration and screenshot checks; retain this stack and add no durable dependencies.

## LogicPass and increments

Outcome: recover a long session reliably and adjust readability without changing engine rules or existing saves. Highest risks: overwriting a good slot, accepting corrupt external data, duplicate actions during resume, unreadable large controls, and input gestures triggering unintended commands.

1. P0: retain old slot paths; add an explicit quick slot, atomic bounded turn autosaves with date/turn labels and cadence/retention preferences, and SAF native JSON import/export. Validate through existing core load; cancellation/failure never replaces the live game. Verify staged bytes before atomic publication. Do not rely on background callbacks for durability.
2. P1: persistent bounded UI/font scaling and high-contrast light/dark presentation, legible map labels, state symbols, scrollable settings/help and compact layouts. Retain camera/selection/live session through recreation.
3. P2: inspect actual input feasibility; bounded gesture options with fixed non-conflicting roles, explicit help/reset; gate animation on foreground and map visibility. External hardware and real-device power claims require evidence. Unsupported controller functionality is an escalation/defer gate, not an implementation claim.
4. Focused regression, screenshots using synthetic art only, final diff review, PR with latest-head CI, and canonical Drive handoff. No merge, deployment, release, engine/vendor edit, save migration, scenario/mod manager, dependency or cleanup.

## Failure and coverage matrix

| Boundary | Required evidence | Baseline / current status |
| --- | --- | --- |
| Native save identity/replay | frozen legacy and M5 round trips, next-turn equality, malformed/future/tampered rejection | 29 Rust tests pass, including legacy/M5 future-state regressions |
| Private slot writes | previous value survives oversized/failed/interrupted write; bounded retention | host, Android filesystem and UI staging-write failure checks pass |
| Quick/manual/recovery | distinct slots, explicit overwrite/load, restart, missing/corrupt diagnostics | quick/manual/recovery, missing/corrupt/future diagnostics and retained pre-M7 manual-save upgrade journey pass |
| Lifecycle | background/resume, configuration, process recreation, no duplicate turn | recreation, background/resume and explicit force-stop recovery pass; OS kill can lose an in-flight action |
| SAF | destination selection, round trip, cancellation, malformed/version failure | local DocumentsUI round trip, cancellations and malformed/future rejection pass; alternate providers untested |
| Storage failure | inaccessible URI, insufficient storage, old slot preserved | host stream/open failures covered; real storage exhaustion, revoked URI and physical power loss untested |
| Display | phone/tablet, both themes, UI/font extremes, rotation, touch targets | phone/tablet both orientations and app scale extremes pass; combined Android 150% / app UI130/font150 phone checks and visual review pass |
| Input | gesture conflicts/reset, keyboard/mouse/stylus/controller feasibility | phone/tablet gesture, keyboard focus/camera and mouse-wheel event tests pass; no external hardware evidence |
| Power/performance | foreground/background rendering, pacing/resume | API-36 emulator frame observation passes; real-device energy/frame pacing deferred |

Rollback: feature branch/revert for code; unchanged save contracts and old slot paths preserve compatibility. Uninstall/clear-data deletes private saves. External document providers control export durability; never promise atomic provider writes. Use a new export document and retain private slots.

## Implemented controls and storage

- **Actions → Quick Save / Quick Load**: separate `quick.json`, explicit confirmation, status feedback and validator errors. Cancelling leaves state and slots unchanged. Existing Save game / Load saved game still use `manual.json`; Resume recovery save still uses `recovery.json`.
- **Saves and backups** lists private slots with local date/time and saved turn, plus turn autosaves, import/export and settings. Metadata is read on the existing worker, not the UI thread. Labels describe stored JSON, not proof of replay validity; every load still uses the core validator.
- **Autosave settings**: default every completed turn and three rotating slots; choices off / every 1, 3 or 5 turns, with 3 / 5 / 10 slots. Cadence uses completed turn count (`turn - 1`), not wall time. Only successful turn advancement creates a turn checkpoint; save/load, UI changes and resume do not. Recovery remains per completed action regardless of cadence. The oldest slot among the configured range is replaced. Lowering retention does not delete older extra backups; at most ten autosave files exist. Each file is limited to 16 MiB; temporary staging can require another 16 MiB.
- Private slot paths and native bytes remain compatible with prior builds. New writes sync and verify `*.pending` before same-directory atomic replacement (API 26+). No non-atomic fallback is allowed. A process killed before publication leaves the old slot and an ignored, bounded staging file. The next write reuses staging. Pre-M7 AtomicFile backups remain readable. Physical sudden power-loss durability is not proven by process-kill tests.
- **Export native save…** uses ACTION_CREATE_DOCUMENT, defaults to `Civ3Touch-turn-N.json`, and writes the live snapshot to the selected provider, then checks UTF-8 readback equality. A provider that cannot read back the document yields a failure diagnostic instead of a verified-export claim. **Import native save…** uses ACTION_OPEN_DOCUMENT with explicit replacement confirmation, reads bounded UTF-8, and validates through unchanged `Session::load`. No artwork, proprietary content, URI or Android setting enters the save. There is no original `.sav` interoperability. No broad permission or persistent URI grant is requested. A provider failure may leave an incomplete external document; private saves remain. Prefer a fresh export and keep private copies until an imported round trip is verified.
- **Game settings → Display and readability**: UI scale 100/115/130%, font multiplier 100/125/150%, high-contrast light/dark, and reset. Android's system text scale still applies. Settings persist; changing them recreates presentation while retaining native state, selection and camera. Full-screen menus return to the map on recreation. On screens below 360 effective dp high, or phones with combined font scaling above 175%, information and selected-unit actions open as scrollable screens instead of consuming map height. At the latter extreme, New Game moves into Actions and the unit/Center buttons share the row evenly, avoiding word splitting. Map labels have black/white backing; other units use `!` and cities use Own/Other text in addition to colors. Movement destinations retain dots plus outlines, and selected markers have a double outline (the imported Settler retains its selection oval).
- **Gestures and input help**: choose default select/move tap or inspect-only tap; enable/disable drag and pinch independently; reset. Long press stays inspection-only, avoiding conflicting bindings. Disabled navigation gestures consume movement without issuing commands. Existing Actions/Center remain available. Keyboard map navigation uses arrows, +/− and C; ordinary buttons use Android focus/activation. Mouse wheel controls zoom. These are input-event paths, not a claim of tested physical hardware or full controller support.
- Imported Settler redraw callbacks now stop while backgrounded or a full-screen menu covers the map, and resume only in foreground map presentation. Audio retains its existing foreground gate. No background callback advances the simulation or is relied on to finish a save.

## Evidence ledger

Baseline: all 29 Civ3Touch Rust tests passed (22 unit tests plus seven resource/legacy regressions); baseline Android build/lint passed. The baseline device journey reached turn 37 with Worker/production feedback before an external shared-ADB reset interrupted it. A focused rerun on isolated ADB passed manual save/reload, matching future-turn state, background/resume and explicit process-death recovery; the interrupted full run is not a pass. Before/after hardware energy measurements are not available.

Implementation checks so far: package rustfmt, Clippy with warnings denied, camera tests, synthetic inventory (3), vendor-policy tests (4), compatibility probe tests (9), proprietary source policy and all 115 baseline/7 approved-patch hashes pass. New host filesystem tests pass UTF-8, byte limits, interruption staging, replacement and prior-value preservation on rejected/failed writes. Shell-only Android tests pass slot isolation, date/turn labels, legacy `.bak` restoration, real interrupted AtomicFile recovery, and retention. Build/lint passes with zero errors and the five pre-existing warnings (toolchain update, ChromeOS ABI, backup rules, importer storage advisory and app icon). SAF, missing/corrupt/future slot rejection, staging-write failure and pre-M7 manual-save continuation/Quick Save/relaunch/Quick Load/next-turn comparison pass. Combined Android/app font checks and visual review pass, including reachable settings after process recreation and explicit recovery at maximum text scale. Existing M5 resource inspection and the complete M4 touch/lifecycle journey pass: pan/pinch/long press, camera/selection through rotation, projected movement, city founding, production/queue controls, empty diplomacy, manual save/reload/future play, suspend/resume and process-death recovery. Artwork import passes valid/missing/unsupported/corrupt cases and process restoration; imported selection, rejected/legal movement, End Turn, facing/reset, missing-rule diagnosis/recovery and native smoke all pass. Latest-head remote CI is recorded in the PR and canonical handoff, separately from these local results. QA found and corrected overlapping map labels, insufficient phone landscape map height after density scaling, and word splitting at combined text extremes. Compact headers intentionally ellipsize; tapping them opens full details/actions.

Device evidence uses a dedicated disposable read-only `coloring_tablet_api_36` AVD: Android 16 / API 36, `sdk_gphone64_arm64`, `arm64-v8a`, isolated ADB port 5039 / serial `localhost:5685`. Other running project emulators were not used.

Rendering observation on the final APK: three-second frame deltas were foreground 23, menu-covered 0, background 0 and resumed 23; the launch command returned in 0.12 seconds and native state stayed unchanged. This measures this emulator’s scheduling only, not hardware energy, before/after improvement or a user-perceived latency benchmark.

New commands, using the existing environment and an isolated disposable emulator:

```sh
bash scripts/test-saves.sh
bash scripts/test-android-saves.sh YOUR_SERIAL
python3 scripts/android-qol-acceptance.py YOUR_SERIAL
python3 scripts/android-qol-layout.py YOUR_SERIAL
python3 scripts/android-qol-system-font.py YOUR_SERIAL
python3 scripts/android-save-documents.py YOUR_SERIAL
python3 scripts/android-qol-save-failures.py YOUR_SERIAL
```

The shell DEX is never included in the APK. Device journeys use synthetic slots and artwork; public screenshots must come only from `.local/m7-evidence`. The save journey deliberately replaces synthetic test quick-slot bytes for corruption checks and restores them. Never run it against a playthrough to retain. Existing import, touch, resource and core-loop regressions remain required where affected.

Android skill scope: `testing/testing-setup` applied using the established stack. `system/edge-to-edge` was considered but requires Compose; this app retains platform Views and its inset listeners. `profilers/android-profiler` was considered; no physical-device trace/energy evidence is available, and no profiler dependency was introduced. R8, Play and AGP migration are unrelated to this milestone.

## Approved P2 deferral — 2026-10-10

Dan approved emulator verification for gestures, keyboard/mouse events, accessibility/layout and lifecycle; deferred controller implementation and physical-device stylus, controller and power validation to owner testing when the APK is available. These are **deferred / not tested**, never completed features. Controller support remains a future implementation task. M8 mod/scenario selection, M9/M10 roadmap work, original-save fidelity and open M5 rules gaps remain unchanged; this milestone does not advance them. The immediate priority stays save reliability, lifecycle safety and existing gameplay compatibility.

## Owner playtest and remaining limits

On a physical Android device, install the reviewed debug APK as an update with the same signing key; do not uninstall or clear data to upgrade. Import your own GOG artwork if needed. Export a native save before longer testing, keep that document, then test the following with a disposable copy of your playthrough:

1. Load an existing save, play several turns, Quick Save, leave/restart the app, explicitly Quick Load and continue. Check city production, research, resources and turn count.
2. Select an autosave cadence/retention, inspect the dated checkpoints and load an older checkpoint deliberately. Export to your chosen document provider, import the copy and continue. Cancel both pickers once.
3. Try both themes and UI/font extremes in portrait/landscape, including Android text scaling. Check city/production/diplomacy screens, disabled controls, map markers, gesture reset and input focus.
4. Background, suspend/lock and resume during ordinary play. An in-flight command may not survive process death; recovery promises the last completed checkpoint, not an Android background-write guarantee.
5. Evaluate physical stylus precision and keyboard/mouse ergonomics. Controller implementation remains a future task. Measure sustained frame pacing, resume latency and battery use on actual hardware before claiming an efficiency improvement.

Untested until explicitly recorded otherwise: physical handset/tablet/stylus/controller input, controller implementation, real-device power/thermal behavior, TalkBack end-to-end use, API 26 runtime, 16 KB-page hardware, alternate/cloud document providers, revoked URI grants, true device storage exhaustion and sudden power loss. Host I/O failure injection and emulator staging-write denial are narrower evidence than a full-disk test. No hardware benchmark improvement is claimed. The app remains an ARM64 prototype with the existing rules/import limits.

M8 mod/scenario work, M9 broader distribution support and M10 desktop/display roadmap work remain open. M7 does not authorize them. The next acceptance step is First Officer review and Captain hardware playtesting, followed by an explicit merge decision. No merge, deployment, release/tag or Cleanup is part of this delivery.

## Synthetic visual evidence

These captures contain only the project’s Canvas markers and generated world, never imported Civilization III artwork. Visual review checked readable labels, scrollable production controls, system insets and map space. Large-font phone landscape uses the compact information/action screens.

| View | Configuration |
| --- | --- |
| [Save management](images/milestone-7/save-management.png) | Phone, default scale; separate dated private slots and document actions |
| [Phone map, light](images/milestone-7/phone-light-map.png) | 1080×2400, density 420, app UI/font 100/100 |
| [Phone city, dark](images/milestone-7/phone-dark-max-city.png) | 1080×2400, density 420, app UI/font 130/150 |
| [Phone landscape map](images/milestone-7/phone-landscape-max-map.png) | 2400×1080, density 420, app UI/font 130/150 |
| [Tablet map, dark](images/milestone-7/tablet-dark-max-map.png) | 1600×2560, density 240, app UI/font 130/150 |

## Review APK

Local owner-playtest artifact: `.local/milestone-7/civ3touch-m7-debug.apk`, copied from the final successful debug build without regeneration. SHA-256: `d5816292740de46692a0dc8757807f9cf3c54db4e4959ef21d379d1751c70bd2`. The APK contains no proprietary artwork; An ordinary compatible update retains app-private imports/saves. This is a local debug review artifact, not an approved production release or tag.

# Milestone 4 — touch interface

Date: 2026-10-08 (Asia/Tokyo). Owner: Dan. Authority: [Milestone 4 Crew Brief](https://docs.google.com/document/d/1N5vx-kjQc4sJ2hsAweb9p-dI2i4N0PCHL9bu3G3AIXE/edit). Branch: `codex/milestone-4-touch-interface`, from merged Milestone 3 `a5b2031`. Implementation and focused Uplink are authorized. Review, Captain playtesting and merge remain acceptance gates; the roadmap is not advanced by this implementation.

## Interaction contract

Android remains Java Views/Canvas with the existing JNI command/event boundary. No engine, rules, replay contract, storage, import, dependency or vendor changes are required.

- Tap the selected unit or its sheet header to expose its actions. Tap an engine-highlighted destination to move, including a friendly-occupied destination. Other own units can be selected by tapping or Actions → Choose unit. Own cities can be opened by tapping when that tap is not a selected legal move. Long press disambiguates stacked units and cities without issuing a command.
- Drag pans the camera. Android's gesture detector distinguishes touch slop, taps, cancellation and long presses. Once a second pointer participates, the entire gesture is excluded from tap commands, including the final single-pointer lift. Pinch scales only the map from 0.65× to 2×, around its focus, bounded to the world. Android font scaling continues to control text and ordinary controls.
- The camera is retained across rotation/backgrounding. Selecting a different unit, moving the selected unit or loading/starting a game recenters on the selection. Center explicitly recenters after exploration. Presentation coordinates never enter simulation or replay state.
- The bottom unit sheet lists only engine-advertised actions for the current unit, including attacks, Worker jobs and city founding where available. Collapse it to reclaim map space. Its bounded, scrollable height is smaller in landscape. No commands are invented for an exhausted unit.
- City screens put population, food, shields, commerce, buildings, current production and pending queue in a single scrollable view above large production/queue controls. Changing production retains the existing shield-reset rule; adding/clearing queue items uses the existing commands. Choices return to the map and refresh from the resulting snapshot. Return to map and system Back dismiss screens.
- Information expands to the existing turn/selection/completion feedback plus treasury and research. A bounded two-line summary signals completion feedback while collapsed; expand and scroll for all messages. Import/audio controls appear before play or under Actions → Game settings, keeping expanded information usable in landscape. Buttons have a minimum 48 dp target; all screen content scrolls for large text.

## Deliberate tradeoffs

The pinned `PlayerView` and available-command protocol contain no diplomatic relationship, dialogue, peace or treaty commands. Diplomacy therefore presents only opponent IDs already in the player's known units/cities and clearly explains the absence of negotiation. The empty state asks the player to explore. It neither reveals hidden players nor invents relationships, names or simulated conversations. Adding diplomacy mechanics remains Milestone 5 work.

No additional quick-command bar is added. The existing New Game / End Turn / Actions row handles global operations; the unit sheet handles contextual commands, and Center is a camera control. Actions also exposes a full-screen selected-unit list for constrained layouts. Full-screen menus and their scroll offsets return to the map on rotation; the live game, camera, selection and panel expansion survive. Viewport zoom is session presentation state and is not stored in game saves. Full TalkBack tile navigation and alternative input modes remain outside the brief.

## Verification and evidence

See [testing.md](testing.md) for reproducible commands. Evidence was collected on disposable ARM64 API-36 emulators, using the supported local English GOG installation for import/gameplay and separate synthetic saves for public screenshots.

- Rust: 22 tests, package formatting and Clippy with warnings denied passed. Three synthetic inventory tests passed; all 115 pinned vendor hashes match; source-policy check passed.
- Android: debug build and lint passed with zero errors and five existing warnings. No dependency or lint suppression was added. APK inspection found only the 15 open-source Lua rule assets, with no proprietary art/audio/installer payload.
- Layout: phone/tablet, portrait/landscape and 1.5× text journeys passed; screenshots inspected. Expanded information retains a usable landscape map (254 px at 420 dpi in the focused check). Back and rotation of open screens preserve engine state.
- Import: actual folder-picker import passed for supported data; missing, unsupported and corrupt inputs failed as expected, retained the prior active data and left no staging import. Restart revalidated app-private assets.

- Touch: real drag/pinch preserve the complete engine snapshot; long-press stack selection, panel collapse, camera retention on rotation, transformed legal-tile taps, sheet founding, city production/queue clearing and empty diplomacy passed. Manual save/load, deterministic continued play, background/resume and recovery after force-stop passed.

- Feedback: road, unit-production and research completion messages passed, including simultaneous research/production completion at turn 41; positive production overflow into queued Workers passed. The full feedback regression also passed manual/recovery replay and New Game → Save → Load with continued deterministic play. Imported gameplay passed movement costs, production/queues, research, Worker roads, exploration/fog, AI encounter and combat through turn 42. At the actual encounter, Diplomacy matched only the opponent IDs in the player view and left the complete snapshot unchanged. Imported manual/recovery replay, future-turn equality, background/resume and process-death recovery passed on an isolated emulator after the original device was interrupted by another app. The relocated in-game audio toggle passed both directions without changing simulation state. Imported touch movement/rejected moves, native/atlas facing, New Game reset, missing-rules diagnosis/recovery and native smoke probes passed. Synthetic screenshots are the only visual evidence permitted in the PR; proprietary artwork stays local.

Physical-device playtesting, API-26 and 16 KB-page runtime coverage, full TalkBack navigation, listening/audio quality, alternate document providers and OS power-loss behavior were not run. Emulator automation and screenshot inspection do not establish final game feel or milestone acceptance. UI Automator intermittently returned null roots or exited during window transitions; an ADB restart and missed injected taps also interrupted initial attempts. Direct taps at the observed control coordinates worked. A later failed recovery lookup returned another app’s UI hierarchy, proving cross-task device interference; the imported save was transferred locally to isolated `emulator-5680` for the remaining lifecycle checks. No proprietary images or test-state archives enter the PR. The harness uses bounded fresh-hierarchy retries, explicit New Game readiness and a short press with settling delays. State/feedback assertions are unchanged; no assertion uses stale UI output.

Verified local debug APK SHA-256: `003e7c6061d52a311ec5f0ce5384e4463e8b71cdf3d29f06cde6ccd790a5b233`. This is local test evidence, not a release artifact or approved release.

Synthetic review images: [unit sheet](images/milestone-4/unit-sheet.png), [city](images/milestone-4/city.png), [diplomacy](images/milestone-4/diplomacy.png), [landscape information at 1.5× text](images/milestone-4/phone-landscape-large-information.png).

## Captain playtest

1. Import the supported English GOG installation, then Play or load an existing Milestone 3 v2 save.
2. Tap a unit; drag in several directions; pinch in/out; verify no unintended movement. Center it, then tap a highlighted destination.
3. Collapse/expand the unit sheet. Long press a tile with stacked units or a city and choose a target. Use a Settler's Build City or a Worker's available improvement.
4. Enter a city, inspect the economy, choose production, add a queued item and clear the pending queue. Return to the map with both the explicit control and system Back.
5. Open Research, make a choice, advance turns and expand Information to read completion messages.
6. Open Diplomacy before and after encountering the opponent. Confirm it reports only known-map information and explains that negotiations are unavailable.
7. Save, advance a turn, reload and continue; background/resume, rotate, and force-stop/relaunch with Resume recovery save. Repeat key controls in landscape, tablet dimensions and larger text.

Rollback is the normal feature-branch/revert path. The core save contract and vendor baseline are unchanged. No merge, release, production deployment or cleanup is included.

# Milestone 3 — core playable loop

Date: 2026-10-06 (Asia/Tokyo). Owner: Dan. Authority: [Milestone 3 Crew Brief](https://docs.google.com/document/d/1iHSEP67hMxD_kxVMs5U2FBkREl8NVbO-5bgz5ep1tbU/edit). Branch: `codex/milestone-3-core-loop`, based on merged Milestone 2 `f1093da`. The brief authorizes focused Uplink, but not merge, deployment or release. **Milestone acceptance remains pending review and subsequent Captain Android playtesting.**

## Implementation and boundaries

The existing Rust session now starts a generated 16×16 world with two civilizations, each with a Settler, Worker and Warrior. Seed 42 remains the small reproducible prototype world. All movement costs, legal city sites, population, food, shields, commerce, research, buildings, worker progress, fog, combat and turn processing use the pinned FreeC3 core and its existing base Lua rules. Vendor files remain unchanged.

The adapter validates Android requests against currently available commands. Movement remains one immediate step. End Turn skips unused movement, runs FreeC3's SimpleAgent for the opponent, then returns to the human. AI decision randomness restarts each round from a clone of the simulation RNG; no separate unsaved AI state is required. AI command processing is bounded at 256 commands per round, with skip/end fallback on failure or budget exhaustion. Hidden AI movement/combat events are not exposed to Android. The human sees the refreshed fog-limited player view and own turn-start events.

City queues hold at most eight pending items and select the next item after a production-complete event. Pending items remain in order through saves. Invalidated queues are cleared with a visible explanation; captured/destroyed cities lose their pending queues. Choosing production immediately uses the native rule that changing items resets stored shields. Switching queued items uses that same rule, including its overflow limitation. With no pending queue, FreeC3 automatically selects its cheapest available production; this is explicitly not desktop Civ III parity.

Android retains the Java/Canvas/JNI architecture. Actions lists selectable units (including stacked units), legal unit actions, cities and production, research, manual save/load and recovery. The camera follows the selected unit, falling back to another surviving own unit or a city when it is consumed/destroyed. Own and discovered cities, enemy units, roads, mines and irrigation are rendered. Existing imported Settler art and terrain remain; other units and cities use clearly simple Canvas markers. No additional proprietary payloads, dependencies, accounts, network permissions or engine patches were added.

## Save architecture decision

**Context:** The brief requires platform-neutral save/load and Android lifecycle safety. Existing upstream world serialization skips the live RNG and does not include private engine turn state. The engine already supports command replay.

**Decision:** Dan approved option 1 in chat on 2026-10-06: versioned command-replay saves, app-private manual and recovery slots, atomic writes. Replaying validates each recorded command result and then compares the complete serialized world, city queues, current player and turn. Exact base Lua contents and an explicit engine/session contract ID must match. Save data never supplies an executable rules path. Loading constructs and verifies a separate session before replacing the live one.

**Alternatives considered:** Full engine snapshots would load faster but require additional engine persistence work and possibly separately authorized vendor patches. They were not selected.

**Consequences:** Loading cost grows with history. This prototype limits saves to 20,000 actions and 16 MiB; failures are explicit. Original Civ III saves and migration across future rule contracts are unsupported. The saved world is a replay verification record, not a directly deserialized engine replacement. Exact Lua content is included as open-source rules identity; proprietary imported artwork/audio is never embedded.

Android uses `getNoBackupFilesDir()/saves/{manual,recovery}.json` and `AtomicFile`, syncs writes, and verifies readback. Recovery is updated after every completed action before presenting the new state. A save failure preserves the prior committed slot and reports that the user should keep the app open and retry. A killed process can recover the last committed action; an interrupted in-flight action may need to be repeated. Explicit New Game or load replaces recovery, while the manual slot changes only through Save game. Clearing app data or uninstalling removes both slots. On relaunch, choose Actions → Resume recovery save or Load saved game. Imports must validate before either is available. Debug synthetic sessions use separate slots.

## Verification

Exact commands and harness instructions are in [testing.md](testing.md). Current local evidence: all 20 Rust tests, own rustfmt and Clippy with warnings denied pass; three synthetic inventory tests, source policy and all 115 vendor hashes pass. Android build/lint passes with zero errors and the five existing toolchain/ABI/backup/icon/free-space advisories. No checks were weakened. APK inventory contains only the 15 open-source base Lua assets and native library, not proprietary media.

The API-36 ARM64 disposable emulator passes the existing touch/facing/rejected-move/reset/missing-rules/recovery journey and the real SAF import journey, including missing/unsupported/corrupt inputs, preservation of the prior import and process-restart restoration. The expanded imported-assets touch journey passes: friendly-occupied one-step movement, city founding, chosen and queued production, a completed Worker road, fog exploration, opponent combat at turn 25, completed research at turn 42, separate manual save/quit/reload, continued play, background/resume and force-stop recovery. The save/reload check on the subsequent menu-validation build also compares the next turn after reload against the previously observed next turn and matches the full player view and queue.

Two early harness runs were corrected: its first exploration policy took a long southern route without reaching the opponent before the test budget; a later run continued exploring unnecessarily after combat. The final route uses only discovered terrain and stops exploring once combat is demonstrated. Review also corrected friendly-occupied highlighted tiles selecting another unit instead of submitting a move. No game-state injection or rule changes were used to satisfy the journey.

Final local debug APK SHA-256: `89d3c279e767709734192fa8f9101151f2afe6d0950aee55a9643c1646c5ee4b`. This is the tested development build, not an approved release artifact. Phone (1080×2400/420 dpi), tablet-sized (1600×2560/240 dpi), landscape and 1.5× text checks pass on the emulator; game state survives the configuration changes and display settings were restored. Missing manual-save loading reports an error without replacing the live game. The corrected City details dialog was visually inspected on phone and larger-text landscape/tablet. Latest-head CI is recorded in the delivery handoff. Visual inspection found Android dialog-title truncation; city economy/queue and research progress now use separate scrollable details/status dialogs.

Practical core tests cover legal/rejected movement, city growth and commerce, queued unit production, research, roads/mines/irrigation, opponent activity, exploration/combat, round-trip state and subsequent deterministic results, and malformed/incompatible/tampered saves. Tests use generated worlds and open-source rules. Android's debug snapshot file contains only the human-visible simulation state and a sequence number; the acceptance harness uses it for read-only assertions while all game actions go through visible touch controls.

## Playtest after review

1. Install the reviewed ARM64 debug build, import the supported English GOG folder, and tap Play.
2. Select the Settler, move to an adjacent highlighted tile, End Turn, then Actions → Build City.
3. Actions → Cities and production → city → Choose production. Select Warrior; add Worker to the queue. Open City details to inspect population, food, shields, commerce, buildings and the queue. Research status shows progress and completed technologies.
4. Actions → Research → select a technology. End turns until production and research complete.
5. Actions → Choose unit → Worker. Build Road. For Mine, move to a Hill; for Irrigation, use vegetation-free Grassland/Plains/Desert. Worker jobs take two/three turns in these rules.
6. Select a Warrior, explore into fog, encounter the other civilization and use an available Attack enemy action. Observe subsequent AI turns and combat results.
7. Save game; continue a turn; quit/relaunch; Load saved game and verify the earlier turn, units, cities, economy, queues, research and fog. Continue playing.
8. Background/resume and rotate during play. Force-stop/relaunch and use Resume recovery save. Check phone/tablet, landscape and larger text. Test empty/missing save handling without clearing a wanted manual save.

Unrun checks: physical Android hardware, API 26 runtime, 16 KB-page devices, forced low-space/power-loss injection during an atomic write, comprehensive accessibility, alternate document providers and subjective audio listening. Original-asset screenshots and game payloads are excluded from review artifacts. Captain post-review acceptance is pending.

Known compatibility limits: fixed small world; inherited simplified FreeC3 rules and basic AI; mines only on Hills; irrigation only on vegetation-free Grassland/Plains/Desert; no full Conquests parity, original binary saves, deep diplomacy, final touch redesign, multiplayer or store release. Original art for new unit types and cities, map pan/zoom, and comprehensive TalkBack tile navigation remain outside this implementation.

Rollback: source changes stay on the feature branch and can be reviewed/reverted through normal Git workflow. Vendor baseline, source GOG installation and installer are unchanged. Prior app versions cannot read the new save format; retain the reviewed build when retaining these local saves. No production release or remote cleanup is part of this mission.

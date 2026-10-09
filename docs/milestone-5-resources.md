# Milestone 5 — resource foundation, slice 1

Date: 2026-10-09 (Asia/Tokyo). Owner: Dan. Branch: `codex/milestone-5-rules-compatibility`. Scope: [M5 Crew Brief](https://docs.google.com/document/d/1vaHIo3TjDDqFBKw63hY0qtk8eRRrtJJRnE0aSJERGsA/edit), [initial audit](milestone-5.md), and Dan's explicit approval of narrow resource patches and versioned legacy-save preservation. This slice is partial M5-05/M5-06 work, not completion of those areas or Milestone 5.

## Implemented behavior and evidence

New Civ3Touch games enable the shared engine's resource mode. Wheat, Cattle, Gold and Horses receive stable IDs and deterministic placement on the existing 16×16 prototype map. Wheat/Cattle/Horses use clear Grassland/Plains, Gold uses clear Hills/Mountains, and Horses can also use clear Hills. Placement uses a separate seed-derived ChaCha8 stream, a one-in-eight candidate-tile frequency and uniform choice among eligible resources. These frequencies and placements are **prototype policy**, not a reproduction of the original map generator. No combat/start RNG draws are consumed by placement.

Reference evidence comes from the user-supplied `Conquests/Text/Civilopedia.txt`, SHA-256 `f88e8020bb69fc489e5fccc52ec371571510f99961b9d92889278800c68bd2c2`, in the locally identified GOG Conquests 1.22 installation. The relevant original documentation sections are `GCON_ResourcesN`, `GCON_ResourcesB`, `GCON_ResourcesS` and `GOOD_Wheat`, `GOOD_Cattle`, `GOOD_Gold`, `GOOD_Horses`. No original text or rules payload is packaged or committed. Independently expressed mechanics from those sections:

| Resource | Worked-tile food / shields / commerce bonus | Discovery requirement |
| --- | --- | --- |
| Wheat | +2 / 0 / 0 | None |
| Cattle | +2 / +1 / 0 | None |
| Gold | 0 / 0 / +4 | None |
| Horses | 0 / 0 / +1 | The Wheel |

Bonus resources affect the worked square, without requiring a road or trade connection. The shared engine adds bonuses when ranking/working non-center city tiles and in city tile-yield queries. Horses are exposed to a player only after The Wheel; hidden Horses do not contribute a bonus in this slice. Completing research refreshes that player's cached city yields and worked-tile selection. The source documents technology-based appearance; exact original-game ordering of yield availability still awaits runtime comparison.

Normal tile views filter resources by both exploration and player technology. Fringe tiles remain terrain-only; another player's research does not reveal Horses. Android parses the optional resource label, displays it on the map and in the existing long-press tile inspector. Android performs no resource calculations. Legacy views omit the field where no resource is exposed.

## Decision record: engine patch strategy

**Context:** the pinned engine had resource storage but no resource rules, and the project prohibited unapproved vendor edits. **Decision:** Dan explicitly approved narrowly scoped FreeC3 resource-foundation patches on 2026-10-09. The existing engine remains the simulation authority; no duplicate full engine, external service or dependency is added. **Alternatives considered:** wrapper-owned parallel state and keeping the milestone at audit; neither was selected. **Consequences:** preserve upstream license/notices, retain the original `docs/freec3-baseline.json`, and record every changed/new vendor file with its original/current SHA-256 and reason in `docs/freec3-resource-patches.json`.

The patch changes engine construction/log replay, worked-tile yields, research refresh, world/view resource fields, a new focused resource module and one headless-test tile literal. The 15 base Lua files stay byte-identical, preserving old save rule identities. `scripts/verify-upstream.py` checks all baseline files against their original hashes unless specifically recorded as patched, checks patched/new hashes, verifies the original-hash chain and rejects unrecorded vendor additions. Do not regenerate either manifest merely to make a failing check pass. Review the exact vendor diff first; unrelated edits require fresh approval. CI runs the existing upstream tests and the new integration tests.

## Decision record: save and ruleset policy

**Context:** saves are deterministic command replays validated against exact results/world state. Changing historical rules would invalidate or alter them. **Decision:** Dan approved retaining old rules for old saves, with explicit versioned contracts and no silent migrations. **Alternatives considered:** a duplicate full engine or a new-game-only cutover; neither was selected. **Consequences:** keep save envelope version `1` and dispatch by an exact ruleset identifier:

- `freec3-90fc7ee-civ3touch-m3-v2`: resource mode off, original base Lua and original city/turn behavior. Saving again retains this identifier. The new optional world flag is omitted when false, preserving the old serialized world shape exactly.
- `freec3-90fc7ee-civ3touch-m5-resources-v1`: resource mode on for new games. The resource flag, placements, command outcomes and future turns are replay-verified. Rule definitions and deterministic placement policy are tied to this identifier; changing them requires a new contract and compatibility assessment.

The shared core `GameLog` also records resource mode, defaulting absent fields to legacy mode. Its replay works independently of the Android/session adapter and has a dedicated round-trip test.

No migration is performed. Unknown envelope versions/contracts, changed Lua contents, altered resource state, mismatched outcomes or state fail before replacing the live session. Save data never selects executable paths or provides executable resource definitions. Existing limits (16 MiB and 20,000 actions), atomic Android writes, manual/recovery slots and lifecycle flow are unchanged. The previously unsupported M3-v1 contract stays unsupported. Return to Dan if later rules require disproportionately complex legacy handling.

## Regression evidence and reproduction

`crates/civ3touch-core/tests/resource_foundation.rs` checks frozen legacy saves/next-turn state from unpatched `350d15c`, exact re-save under the old contract, new-game placement and replay continuation, contract/resource tampering, independent RNG, allowed terrain, worked versus unworked yields, technology revelation, player isolation and research-driven yield refresh. The synthetic legacy fixtures were generated before engine edits by New Game → found city → research Pottery → five EndTurns; the next EndTurn was captured separately. They contain only open-source rules and synthetic game state, not original Civ III saves/assets. Do not regenerate them with the patched engine.

Commands: existing [testing.md](testing.md), plus `cargo test --locked --test resource_foundation`. Local initial results: 29 Rust tests pass, own rustfmt/Clippy pass, selected upstream core/decoder/headless tests pass, inventory/source-policy/gesture tests pass, Android `assembleDebug lintDebug` passes, and synthetic touch selection/move/rejected-move/facing/error-recovery and native smoke pass. Device work uses isolated read-only ARM64 API-36 `emulator-5682`. The final APK also passes `android-resource-acceptance.py`: native resource name appears in the long-press inspector; inspection and Back preserve the complete debug snapshot. Synthetic map/inspector screenshots were visually inspected. Touch navigation, city production/queues, empty diplomacy, manual/recovery replay and rotation passed; phone/tablet portrait/landscape and 1.5× text layouts passed. A first import-harness run passed valid import but could not find Game settings because the emulator was in landscape; the harness documents portrait assumptions. The portrait rerun passed valid, missing, unsupported and corrupt imports, preserved the active installation after rejection, and restored it after process restart. Latest imported-loop and required CI outcomes are recorded in the [canonical Drive handoff](https://drive.google.com/file/d/1Wv2KsQaWjVOx_6pzZtvBk1vYvywHUB-9/view); they were still running when this source record was prepared.

Final local debug APK SHA-256: `d8d3099dcb49942ab1673a3cae5ed69a4bd287fd8d4d8723f98a46e5e6799512`. Android lint has zero errors and the same five warnings. The APK contains exactly 15 open-source Lua assets and no original game payload. This is a local development build, not a release. Layout/touch checks preceded the final isolated GameLog serialization fix; the final APK was rebuilt/linted and its resource-inspector/native-smoke tests repeated. No presentation or session rule behavior changed in that fix.

Google's official [testing/testing-setup skill](https://github.com/android/skills/blob/main/testing/testing-setup/SKILL.md) was read and applied to shared-rule tests, real touch journeys and state restoration. Existing Rust/Java/ADB tools suffice; no DI/mock/coverage/screenshot framework was installed. R8, Play, AGP migration and profiling are not applicable to this slice.

## Known gaps and playtest

Original desktop runtime comparisons remain unrun. Civilopedia values are documentation evidence, not verified end-to-end parity or proof of the active BIQ configuration. Still open: the other resources, exact distribution/exhaustion/respawn rules, vegetation/floodplain cases, city-center resource treatment, government yield penalties, road/harbor/airport networks, strategic production prerequisites, trade, luxury happiness and AI resource strategy. City centers deliberately retain the existing base-yield calculation pending the separate city-economy rules slice. No resource is claimed to supply strategic units or happiness merely by being visible. No remaining matrix area is marked complete.

Captain playtest: start a new game; explore and long press a named resource tile; found a city beside a bonus resource and inspect yields; research The Wheel and check discovered Horses; save/reload and continue; then load a pre-M5 save and verify its old world and continued behavior. Repeat gestures, city production, research, combat, background/resume and recovery. The existing generated world and economy are still simplified.

Rollback: `main` is unchanged; the feature branch isolates the work. The prior build can still load legacy-contract saves but cannot load new M5-contract saves. Keep the matching build with M5 saves. No save deletion, merge, deployment or release is authorized by this slice.

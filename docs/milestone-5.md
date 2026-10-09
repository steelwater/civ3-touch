# Milestone 5 — rules compatibility audit

Date: 2026-10-09 (Asia/Tokyo). Owner: Dan; audit and proposed implementation owner: Civ3Touch Crew. Authority: [Crew Brief](https://docs.google.com/document/d/1vaHIo3TjDDqFBKw63hY0qtk8eRRrtJJRnE0aSJERGsA/edit), [roadmap](https://docs.google.com/document/d/149SobRRG_ZL3qq4fXuJ5KZG0R_l9vmf3NOmQ8W1TWlE/edit), and Dan's instruction to start on a new feature branch. Branch: `codex/milestone-5-rules-compatibility`, from clean `main` at `350d15c`. The roadmap explicitly marks Milestones 1–4 complete; their older repository acceptance notes are historical.

## Outcome and boundaries

**Subsequent approval and implementation:** Dan approved targeted resource-foundation engine patches and preserving legacy save behavior without duplicate engines. The accepted decisions, implemented slice and updated evidence are in [resource foundation](milestone-5-resources.md). The initial matrix remains an audit snapshot; M5-05/M5-06 now have the documented partial implementation, with runtime parity and remaining mechanics still open.

The matrix below records the required initial source audit, not completed Milestone 5 or a claim of Conquests parity. The subsequent resource slice is documented separately above. The brief permits focused implementation PRs; merge, deployment and acceptance remain separate gates.

LogicPass: identify the actual shared-core behavior, preserve the existing loop and replay contract, establish a reproducible reference, then implement the smallest verified dependency slice. The principal risks are inventing rules from FreeC3 placeholders, changing old replay outcomes, and introducing a second authority for simulation state. The first behavior change follows the approved architecture/save decision below and the original documentation evidence recorded in the resource slice.

## Observed reference identity

The local source is extracted GOG data, not evidence that the original desktop game has been run. No Windows executable was executed. Metadata-only inspection on this branch found:

| Item relative to `local-data/gog/app` | Bytes | SHA-256 |
| --- | ---: | --- |
| `goggame-1471405734.info` | 821 | `c2a356e9aed07152b33197879413cd8bb96938ea92cd8a46639afd87a5fb26df` |
| `Conquests/readme.txt` | 26016 | `e9c68a985af1b33fecd306e2e56ff45daccf85f42159f2856d1856546bee2f50` |
| `Conquests/Civ3Conquests.exe` | 3417464 | `838df6f8b3518d5f5f7ff50c7c7add715628ffabbd37221d0bcc7f080afc2746` |
| `__support/save/Conquests/conquests.biq` | 30501 | `1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c` |

The GOG metadata reports game/root ID `1471405734`, language `english`, no `buildId`, and metadata schema `version: 1`. The executable's fixed-version signature yields file/product version `1.22.0.0`; this is metadata evidence, not runtime verification. The previously recorded installer filename is `setup_civilization3_complete_2.0.0.7.exe`, SHA-256 `4ad54ca308ea93af49b0db3a795736eda91aed61c8cd73b18196802d0f4d2395`; the installer hash was not re-run in this audit.

Neither `Conquests/conquests.biq` nor root `conquests.biq` exists in the extracted tree. The support copy above is a candidate reference; its actual installation destination and use by a running standard game are **unverified**. Do not silently treat a scenario BIQ or this support copy as the active standard rules. Difficulty, civilization, map size, world settings, enabled victory conditions and optional game rules are not yet captured from a reference session. Exact active rules configuration remains an open gate.

## Evidence map

Paths prefixed `core/` below mean `vendor/freec3/crates/fc3_core/src/`; `lua/` means `vendor/freec3/mods/base/`. The pinned FreeC3 revision is `90fc7eeda2d1914c9306b33a161686fa839b8455`.

- E1: `core/world.rs::Player`, `core/city.rs::CityStore`, `core/protocol.rs`. Player state has gold, science and culture, but no government, diplomatic relationships, reputation or Golden Age state. Commands cover movement, combat, fortification, production, skip, unit actions, research and end turn.
- E2: `core/tile.rs::TileStore`, `core/mapgen.rs::generate_map`, `core/scripting/api_tile.rs`, `lua/yields.lua`, `lua/units.lua`. Tiles allocate optional resource IDs; map generation leaves them empty. Lua can read resource IDs but base yields never use them. Swordsman resource/technology requirements remain a TODO. Resource storage is not resource gameplay.
- E3: `core/engine/city.rs::reassign_city_tiles`, `calculate_tile_yield`, `process_city_food`, `process_city_production`; `core/engine/mod.rs::handle_end_turn`; `lua/growth.lua`, `production.lua`. Working tiles produce cached food/shields/commerce; the same commerce is credited to gold and science while researching, minus building maintenance from gold. No happiness allocation, corruption or waste calculation exists. Growth uses `10 + 2 * population`; this is an inherited approximation, not a verified rule.
- E4: `core/engine/city.rs::city_radius_static`, `city_border_tiles_static`, `core/engine/visibility.rs`, `core/scripting/api_city.rs`. Fixed workable radius/ownership/visibility exist. The player culture counter is exposed, but no base rule accumulates city culture or expands borders from thresholds.
- E5: `core/building.rs::BuildingDef`, `lua/buildings.lua`, `lua/growth.lua`. Palace and Granary are the base buildings; Granary has a food-retention hook. There is no world/player uniqueness category, wonder race, obsolescence model or wonder effect registry.
- E6: `core/unit.rs`, `core/unit_type.rs`, `core/engine/combat.rs`, `core/ai/mod.rs`, `lua/units.lua`, `lua/actions.lua`. Ordinary units, combat, city founding/capture and Worker jobs exist. No leader generation, army membership, barbarian camp lifecycle or pollution-cleaning action is defined.
- E7: `core/engine/mod.rs::is_game_over`, `player_with_most_units`, `core/engine/combat.rs::check_elimination`. Elimination requires no units and no cities. A sole living player wins; optional turn-limit winner uses unit count. Civ3Touch sets no turn limit. These are prototype completion rules.
- E8: `core/civilization.rs::CivDef`, `lua/civilizations.lua`, `lua/movement.lua`. Six named civilizations carry identity fields only. Unit movement traits exist; civilization traits do not.
- E9: `crates/civ3touch-core/src/session.rs`: 16×16 nonwrapping world, seed 42, two players, three starting units each, bounded opponent turns, queued production and command replay. `SAVE_VERSION = 1`; rules contract `freec3-90fc7ee-civ3touch-m3-v2`. Exact Lua text, command results, complete serialized world, queues, turn and current player are checked on load. A changed rules contract is rejected; there is no migration.

## Compatibility matrix and open gaps

The roadmap contains **20 bullets**, while the brief requests 21 areas. This audit splits “Wonders and Small Wonders” into two rows, preserving every named area. This is an explicit accounting assumption, not additional product scope.

Status vocabulary: **supported** requires verified target behavior; **partial** means some relevant implementation exists but is incomplete; **missing** means no implementation of the mechanic was found in the inspected core/base rules/session; **unverified** means the area is not yet sufficiently specified or evidenced. None is marked supported. Every row remains an open gap. Reference verification is **unverified for every row**; the transition and edge-case column is a proposed comparison/test agenda, not asserted original-game behavior. Numerical values and exact ordering must come from the reference before implementation. Crew owns implementation/tests; Dan owns unresolved rules decisions and acceptance.

Existing tests listed here characterize FreeC3/Civ3Touch, not original-game parity. `engine/tests/` is beneath `core/`. Proposed fixtures are not yet implemented or passing tests.

| ID / area | Status / code evidence | Dependencies | Reference transition and boundary cases to establish | Existing evidence / proposed regression |
| --- | --- | --- | --- | --- |
| M5-01 Governments | Missing; E1, E3 | Rules profile, economy | Government selection, revolution/anarchy and completion; unavailable tech, support costs and turn ordering | None; government switch across turn/reload, invalid request leaves state unchanged |
| M5-02 Culture and borders | Partial; E1, E4 | City culture, buildings, ownership | Per-turn culture to threshold to border claim; overlap, wrapping, capture, age effects | `city_tests::test_city_visibility_extends_from_border` only; threshold ±1 and contested border fixtures |
| M5-03 Happiness and civil disorder | Missing; E1, E3 | Government, luxury supply, citizen roles | Citizen mood balance to disorder and recovery; growth, starvation and war effects | None; happy/unhappy tie, disorder entry/recovery, production/research interaction |
| M5-04 Corruption and waste | Missing; E3 | Government, distance/rank, city economy | Gross yields to net yields; capital, remote city, caps/rounding and changed government | `city_tests::test_commerce_accumulates_into_player_gold` is prototype only; net-yield and rounding fixtures |
| M5-05 Resources | Partial; E2 | Definitions, terrain placement, rules identity | Resource appearance and tile yield; hidden/revealed resource, vegetation, city center | No resource gameplay test; deterministic placement/yield/reveal/reload fixtures |
| M5-06 Strategic and luxury resources | Missing; E2 | M5-05, technology, connection network | Connection gives production eligibility or luxury effect; duplicates, pillage, exhaustion, tech loss/access | Swordsman TODO; connect/disconnect, duplicate luxury, unavailable resource production tests |
| M5-07 Trade | Missing; E1–E3 | M5-06, M5-08/09 | Local/export/import supply changes through network and agreement; sea links, expiry, broken route | None; route break/reconnect and resource trade turn/reload integration |
| M5-08 Diplomacy | Partial (encounter display and combat only); E1, E6, E9 | Player relationships, command validation | Contact, peace/war and proposals; unknown opponent, unauthorized action, hidden data | M4 encounter UI checks only; relation transition/attack legality/replay fixtures |
| M5-09 Alliances and treaties | Missing; E1 | M5-08, turn duration | Signing, obligations, activation, expiry and termination; third parties and eliminated signatories | None; treaty duration and expiry save/reload integration |
| M5-10 Reputation-related rules | Missing; E1 | M5-07/09, event history | Breach to recorded consequence; responsibility for route loss and observer knowledge | None; breach/expiry/third-party cases with explicit reference outcomes |
| M5-11 Espionage | Missing; E1 | M5-08, technologies, treasury, seeded RNG | Mission availability, cost, success/failure and diplomatic consequence | None; insufficient gold, eligible target, seeded result and future replay |
| M5-12 Great Leaders | Missing; E6 | Combat/research events, RNG, wonders | Military/scientific leader eligibility, creation and consumption; limits and repeat triggers | None; deterministic eligible/ineligible trigger and consumption fixtures |
| M5-13 Armies | Missing; E6 | M5-12, unit membership, combat | Formation, loading, movement/combat benefits and destruction; capacity and mixed units | None; capacity/rejected load, member death and replay integration |
| M5-14 Wonders | Missing; E5 | Production, tech, global uniqueness, culture | Completion race to unique ownership/effects; simultaneous completion, capture and obsolescence | `building_tests::test_cannot_build_same_building_twice` is city-local only; global race/effect fixtures |
| M5-15 Small Wonders | Missing; E5 | Production, player uniqueness, prerequisites | Per-civilization eligibility/completion/effects; capture, loss and rebuilding | No Small Wonder test; two cities/players competing and capture fixtures |
| M5-16 Golden Ages | Missing; E1, E8 | Traits, combat/wonders, economy | Trigger to timed yield effect to expiry; repeat triggers and government interaction | None; trigger once, exact end boundary, reload midway |
| M5-17 Barbarians | Missing; E6 | Rules profile, placement/RNG, unit behavior | Camp creation, spawn, movement, combat, rewards/raids; fog, difficulty and city interaction | Ordinary SimpleAgent is not barbarian evidence; seeded camp/raid/reward fixtures |
| M5-18 Pollution | Missing; E3, E6 | Population/production, terrain effects, Worker actions | Risk to pollution placement to yield change to cleanup; repeat pollution and interrupted work | Worker tests cover roads/mines/irrigation only; seeded pollution and cleanup integration |
| M5-19 Victory conditions | Partial; E7 | Rules options, culture, land/population, diplomacy, projects | Enabled condition to winner/end state; thresholds, ties and disabled conditions | `combat_tests::test_elimination_last_unit_killed`, `city_tests::test_elimination_requires_no_units_and_no_cities`; each enabled victory trigger and reload boundary |
| M5-20 Civilization traits | Missing; E8 | Civ definitions, economy, tech, Worker rules | Trait assignment to its specific effects; combinations, civilization identity and Golden Age interaction | Civ registry/unit-trait tests are not civ-trait evidence; trait-specific fixtures after reference capture |
| M5-21 Remaining standard-game Conquests mechanics | Unverified; E1–E9 | Active standard rules inventory and rows above | Enumerate residual standard-game mechanics after overlap removal; assess specialists, enslavement, bombardment and other candidates against actual active rules | No parity evidence; one named fixture per confirmed residual mechanic; scenarios/mods remain excluded |

## Implementation sequence and resolved decision gate

1. Confirm the active standard Conquests rules profile and original-game comparison setup. Capture resource definitions/yields/reveal requirements first. The existing free-form Lua base is not authoritative reference data.
2. Implement resource definitions, deterministic synthetic fixtures, yield integration and technology visibility. Then connection networks and strategic/luxury availability. Verify ordinary, boundary, turn and replay cases before integrating trade.
3. Government/economy/happiness and culture; basic relationship state can proceed before trade/treaties, which depend on it. Each behavior needs an explicit reference and discrepancy record.
4. Treaty/reputation/espionage, civilization traits, leader/army/wonder/Golden Age systems, barbarians/pollution, then complete victory and the residual Conquests inventory according to demonstrated dependencies.

**Architecture decision, 2026-10-09:** Dan approved narrow resource-foundation patches to FreeC3, covered by regression tests and recorded separately from the original baseline. Wrapper-owned parallel simulation state and stopping at audit were considered but not selected. The implementation preserves the shared engine/Android boundary and avoids unrelated refactoring. See the [resource patch policy](milestone-5-resources.md).

**Save decision, 2026-10-09:** Dan approved keeping legacy rules for existing saves and a versioned M5 ruleset for new games. One engine uses targeted compatibility handling; no duplicate full engine or silent migration is permitted. If compatibility becomes disproportionately complex, return to Dan. The first slice keeps the old Lua files and legacy replay semantics, with frozen pre-patch fixture checks and new-game replay tests.

## Verification and continuation

See [Milestone 5 verification guide](milestone-5-verification.md). Current work is documentation and metadata inspection only. Observed baseline checks: `cargo test --locked --workspace` passed all 22 tests; `python3 scripts/verify-upstream.py` matched all 115 baseline files; `python3 scripts/check-source-policy.py` and `git diff --check` passed. The documentation diff and new matrix/guide were reviewed. These establish prototype health and source hygiene, not parity. No original desktop comparison, new rules fixture, Android run, focused implementation PR, CI run or acceptance is claimed here. Android build/lint/device checks were not repeated for documentation-only changes. No roadmap completion update is warranted.

Drive handoff: after Dan approved the upload, the [initial audit and verification guide](https://drive.google.com/file/d/1Wv2KsQaWjVOx_6pzZtvBk1vYvywHUB-9/view) were saved as Markdown in the existing Civ3Touch project folder. Drive readback verified the filename, Markdown MIME type, destination and content, including all 21 matrix rows. This resolves the earlier automatic approval rejection. No proprietary payload was uploaded.

Rollback: discard only the reviewed documentation edits if desired; `main`, vendor, local proprietary files and saves are untouched. The branch is the isolation path. Next action: complete focused resource-slice verification and Uplink with latest-head green CI, then Captain review/playtesting. Further resource coverage and original-game runtime comparisons remain open.

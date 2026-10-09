# Compatibility matrix and format-to-engine mapping

Source IDs R1–R7/L1/T1 are defined in [formats.md](formats.md). This matrix describes the inspected GOG candidate, not every Conquests file. “Parsed” means an actual local read, never merely that a reference struct exists. **No external rule or state field is applied to production by M6.** Native tests characterize the prototype, not original-game fidelity.

## Format-level matrix

| Format / feature | Parsed locally | Mapping status | Applied / tested | Evidence and limitation |
| --- | --- | --- | --- | --- |
| Original vanilla SAV | No | Candidate header offsets only | No / synthetic header T1 only | R1/R4; no real sample, version or state fidelity |
| Original Conquests SAV | No | Candidate embedded BIQ and state graph | No / synthetic header T1 only | R4; 32-player and other layout assumptions require sample verification |
| BIC 4.1 | Header | Vanilla identity only | No / synthetic header T1 | L1/R1; decompressed local file, body unverified |
| BIX 11.18 | Header | PTW identity only | No / synthetic header T1 | L1/R1; not a supported gameplay format |
| BIQ/BICX 12.8 | Header, section framing, selected GOOD/TECH fields | Proposed field mapping below | No / header T1, local observations | L1/R3; candidate active standard identity U |
| Scenario BIQ/BICX 12.7 | Header only | Version identified; overrides/map unmapped | No / synthetic header T1 | L1/R3; one bundled scenario, not a custom-mod validation suite |
| BICQ / future versions | No real sample | Unverified variant, diagnostics only | No / rejection classification T1 | R1; never default to Conquests compatibility |
| Standard Conquests rules | Selected bytes, not full rules semantics | Many missing engine fields | No / M5 native resource tests only | L1/R7; successful decoding is not gameplay parity |
| Scenario/mod-folder resolution | Reference source only | Proposed safe-root policy | No / existing fixed-profile tests only | R3/R5; no full scenario, search precedence or SAF mod test |
| Native M3-v2/M4 saves | Existing JSON replay load | Exact old contract | Existing behavior / frozen fixture and future-turn tests pass | `resource_foundation.rs`, `session.rs`; no migration |
| Native M5 saves | Existing JSON replay load | Exact M5 contract | Existing behavior / new-game and reveal/reload tests pass | Same tests; original SAV export/import remains absent |

## Field-level rules mapping

Column P: **H** header only; **F** framing/count only; **V** actual selected field value read locally; **R** reference-defined field, not decoded locally. Column M describes a potential mapping, **not working integration**. Column A/T separates existing applied prototype behavior from its evidence; all original-game runtime parity is **U**. Unsupported flags must be diagnosed in a future importer, not silently dropped. Entries group fields only where they share a mapping/limitation.

Core paths below are `vendor/freec3/crates/fc3_core/src/`; Lua is `vendor/freec3/mods/base/`.

| External section / field(s) | P / evidence | M: target or missing contract | A/T: existing prototype behavior; lossiness/unknowns |
| --- | --- | --- | --- |
| VER# major/minor, count, length | H / L1,R1 | Research metadata only | T1 accepts observed pairs; unknown pairs explicitly unverified |
| GOOD name | V for four / L1,R3 Good.cs | `resource.rs::ResourceDef.name` through explicit index map | Four fixed names; no import; same spelling alone is not identity |
| GOOD record index | V / L1 | Foreign index → approved native resource ID | Native Wheat=0, Cattle=1, Gold=2, Horses=3; original Horses=0, Cattle=19, Wheat=20, Gold=21 |
| GOOD FoodBonus | V for four / L1 | `ResourceDef.food` | M5 worked-tile tests; values agree with candidate, centers/government effects U |
| GOOD ShieldsBonus | V for four / L1 | `ResourceDef.shields` | M5 Cattle bonus tested; full original yield ordering U |
| GOOD CommerceBonus | V for four / L1 | `ResourceDef.commerce` | M5 worked/hidden resources tested; trade/happiness absent |
| GOOD Prerequisite | V for four / L1 | TECH index → stable tech ID → `reveal_tech` | Horses=4 resolves to The Wheel; others=-1; prototype revelation/reload tested, runtime timing U |
| GOOD Type | V for four / L1 | No resource category field | Candidate 0 for bonuses, 2 for Horses; category semantics from reference, strategic supply not applied |
| GOOD AppearanceRatio / DisappearanceProbability | V for four / L1 | No equivalent distribution/exhaustion model | Separate seeded prototype placement; do not substitute original numbers into it |
| GOOD Icon / CivilopediaEntry | R / R3 | Presentation metadata not `ResourceDef` | No resource icon import or Civilopedia UI; current labels only |
| TERR NumPossibleResources / resource bitset | F/R / R3 | Proposed terrain/resource eligibility map | Four resources use hardcoded terrain/vegetation eligibility; no imported bitset |
| TERR Name / terrain index | F/R / R3 | Explicit conversion into `tile.rs::Terrain` and `Vegetation` | Distinct fixed enums; external indexes/overlays must not be cast to native values |
| TERR Food / Shields / Commerce | F/R / R3 | `yields.lua` hooks, no dynamic terrain registry | Existing approximate tile economy; not Conquests values |
| TERR IrrigationBonus / MiningBonus / RoadBonus | F/R / R3 | Yield hooks + Worker actions | Existing road/mine/irrigation tests; restrictions and modifiers not full parity |
| TERR MovementCost / DefenseBonus | F/R / R3 | `movement.lua` / combat hooks | Prototype movement/combat tested, source scaling/order U |
| TERR AllowCities / Impassable / wheeled flags | F/R / R3 | Command validation / traits would need explicit rules | No generic flag import; reject unsupported behavior |
| TERR landmark, disease, pollution fields | F/R / R3 | Missing landmark/disease/pollution semantics | N; cannot discard flags and claim scenario compatibility |
| PRTO Name / record index | F/R / R3 Prto.cs | `UnitType.name`, foreign-index map to `UnitTypeId(u16)` | Name-based registry assigns new sequential IDs; no raw numeric reuse |
| PRTO Attack / Defense / Movement | F/R / R3 | `UnitType.attack/defense/movement` candidates | Existing movement/combat; exact domain, cost units and rule modifiers U |
| PRTO ShieldCost / PopulationCost | F/R / R3 | `UnitType.cost` for shields; no generic population cost field | Cost scale/production semantics need verification; not a direct assignment promise |
| PRTO Required / RequiredResource1–3 | F/R / R3 | Tech/resource prerequisites need rule enforcement | Strategic connectivity absent; current production does not implement these fields |
| PRTO UpgradeTo / AvailableTo | F/R / R3 | Upgrade graph and civ eligibility | `replaces/requires_civ` describe different concepts; no equivalence assumed |
| PRTO max HP | R / R3 Expr.cs | `UnitType.max_hp` needs experience/rules derivation | Do not invent a direct PRTO max-HP field; veteran/elite semantics U |
| PRTO bombard/range/rate/capacity/type | F/R / R3 | Missing or incompatible unit-system semantics | No faithful air/sea transport/bombard integration in Civ3Touch loop |
| PRTO flags/stealth targets/art reference | F/R / R3 | Traits/actions and presentation mapping, each independently audited | String trait storage is not implementation of each source flag; most U/N |
| TECH Name / index | V only record 4 name; others F/R | `TechDef.id/name` via stable mapping | Native `the_wheel` matched for M5; whole tree not imported |
| TECH Cost | F/R / R3 Tech.cs | `TechDef.cost` plus rate/difficulty rules | Research exists; raw BIQ cost is not assumed final beaker count |
| TECH Prerequisite1–4 | F/R / R3 | `TechDef.requires` with graph validation | Existing prerequisite lists; sentinels/cycles/era dependencies require validation |
| TECH Era / flags / Flavors | F/R / R3 | No full era/ability/trade/AI-flavor model | N/U; cannot infer abilities from completing a named technology |
| RACE Name / LeaderName / Adjective / Noun | F/R / R3 Race.cs | `CivDef.name/ruler_name/adjective/noun`, stable IDs | Identity fields exist; encoding and civ index mapping U |
| RACE FreeTech1–4 / city and leader lists | F/R / R3 | Initial state / naming beyond current definition | Fixed session startup; dynamic lists not imported |
| RACE trait flags / preferred governments / aggression | F/R / R3 | Missing civilization traits/government/AI semantics | N; six named base civilizations are not full Conquests civilizations |
| BLDG Name / Cost / MaintenanceCost | F/R / R3 Bldg.cs | `BuildingDef.name/cost/maintenance` candidates | Only Palace/Granary base rules; scaling/effects U |
| BLDG RequiredAdvance / resources / government / building | F/R / R3 | Tech requirements partly resemble `requires`; remaining typed conditions missing | No generic mapping; unsupported constraints must block application |
| BLDG Wonder / SmallWonder / RenderedObsoleteBy | F/R / R3 | Missing global/player uniqueness and obsolescence/effects | N; city-local duplicate prevention is insufficient |
| BLDG Culture / happiness / pollution / production / flags | F/R / R3 | Mostly missing city effect systems | N/U; Granary hook cannot represent arbitrary building behavior |
| GOVT Name / prerequisite / transition | F/R / R3 Govt.cs | No player government/revolution model | N; requires engine work before import |
| GOVT Corruption / TilePenalty / TradeBonus / ScienceRateCap | F/R / R3 | Missing government economy rules | N; current commerce credited to science/gold is simplified |
| GOVT support costs / worker rate / police / war weariness | F/R / R3 | Missing support/mood/government modifiers | N; no lossless application |
| RULE food, road rate, start units/treasury | F/R / R3 Rule.cs | Lua hooks / `WorldConfig` / fixed session setup | Existing approximations; changing any affects replay contract |
| RULE border thresholds / culture / GoldenAgeDuration | F/R / R3 | Missing culture expansion/Golden Age model | N; stored culture counter does not implement effects |
| RULE min/max research / army/barbarian/upgrade fields | F/R / R3 | Multiple missing semantics | N/U; not a single scalar import |
| WSIZ width/height/civs/TechRate/OptimalNumberOfCities | F/R / R3 Wsiz.cs | World config dimensions/count partly present; rates/rank missing | Fixed 16×16 two-civ session; arbitrary maps not accepted |
| WCHR / WMAP seed, wrapping, resources, geography | R only / R3 | `WorldConfig`, tile stores, coordinate conversion | Not in candidate standard framing; map generator/coordinates not equivalent |
| GAME default-rule/default-victory flags | F/R / R3 Game.cs | No scenario-default overlay contract | Need explicit base identity and overlay semantics; not “missing means zero” |
| GAME enabled victories / limits / alliances | F/R / R3 | Missing victory/treaty model | Existing elimination/optional unit-count finish is not source victory parity |
| GAME ScenarioSearchFolders | F/R / R3,R5 | Proposed bounded logical resolver only | No dynamic paths read; Android policy gate in formats report |
| CITY / UNIT / TILE / SLOC placed state | R only / R3 | Native stores with validated ownership/ID/coordinate mapping | No scenario loading; resources, fog, stacks, terrain overlays and placed improvements unresolved |
| DIFF / CTZN / CULT / EXPR / ESPN / ERAS / TFRM / FLAV | F/R / R3 | Difficulty, specialists, culture, experience, espionage, eras, Worker jobs, AI preferences | Framing recorded only; most require missing semantics or per-field research; unknown fields remain unsupported |

“V for four” means only Wheat, Cattle, Gold and Horses were inspected; **not** all 26 resource definitions. Their food/shields/commerce triples are respectively (2,0,0), (2,1,0), (0,0,4), (0,0,1), matching M5's independently documented values. The candidate's Horses appearance value 160 does not validate the prototype's one-in-eight distribution. No full proprietary rule table is reproduced.

## Mapping boundary

| Input stage | Proposed output | Required separation |
| --- | --- | --- |
| Bounded byte stream | Versioned parsed records with original indexes/flags and diagnostics | No live engine/Android mutation |
| Validated records + known base profile | Explicit foreign-to-native ID maps and unsupported-feature report | Preserve distinctions between absent, sentinel and unsupported |
| Approved supported subset | Engine configuration under an approved rules contract | No silent field loss; source provenance and replay tests |
| Unsupported state/semantics | Clear refusal or read-only report | Never claim playable fidelity from partial readability |

## Remaining M5 gaps

All [21 audited areas](../milestone-5.md#compatibility-matrix-and-open-gaps) stay open as parity work. M5-05/06 have only the accepted four-resource foundation; connection networks, strategic prerequisites, luxury happiness, exhaustion and complete distributions remain absent. Governments (01), happiness/disorder (03), corruption/waste (04), trade (07), treaties/reputation/espionage (09–11), leaders/armies (12–13), wonders/small wonders (14–15), Golden Ages (16), barbarians (17), pollution (18) and civilization traits (20) remain missing. Culture/borders (02), diplomacy (08), victory (19) and remaining Conquests mechanics (21) are partial/unverified. M6 reads do not close any of these rows.

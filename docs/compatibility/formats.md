# External formats, sources and scenario paths

See [mission and status](README.md). Evidence captured 2026-10-10; all proprietary inputs remained local and read-only.

## Source inventory and reliability

OpenCiv3 references are pinned to `6a8db067dc2c07ba28b5e648b4c6b0a0b771c769`, retrieved from its public HEAD during this audit. This differs from M0's older reference; it does not update the vendored FreeC3 engine. The following are primary implementation evidence, not a complete authoritative Civ III specification:

| ID | Source | What it establishes / limitation |
| --- | --- | --- |
| R1 | [QueryCiv3.cs](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/QueryCiv3.cs) | Signature/version offsets, integer reads; heuristic section scan and default Conquests classification are not validation |
| R2 | [Util.cs](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/Util.cs) | `00 04/05/06` compression heuristic; US text interpreted as Windows-1252, NUL terminated; whole-file buffering needs bounds |
| R3 | [Biq.cs](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/Biq.cs) and [section structs](https://github.com/C7-Game/OpenCiv3/tree/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/BiqSections) | Section shapes, dynamic references and unknown buffers; unsafe native copies are not an acceptable new boundary implementation |
| R4 | [Sav.cs](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/Sav.cs) and [SAV structs](https://github.com/C7-Game/OpenCiv3/tree/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/QueryCiv3/SavSections) | Embedded rules and state dependencies, repeated/dirty sections, Conquests assumptions; not verified on a local SAV |
| R5 | [C7/Util.cs](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/C7/Util.cs) | OpenCiv3's actual scenario/media lookup, case handling and fallbacks; original GOG search order not verified |
| R6 | [blast.h](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/Blast/reference-code/blast.h), [blast.c](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/Blast/reference-code/blast.c), [license/provenance](https://github.com/C7-Game/OpenCiv3/blob/6a8db067dc2c07ba28b5e648b4c6b0a0b771c769/Blast/readme.md) | Mark Adler's DCL reference algorithm; not ZIP implode. Local research executable only, no production dependency/source copy |
| R7 | [FreeC3 baseline](../freec3-baseline.json), [approved patches](../freec3-resource-patches.json), `vendor/freec3/crates/fc3_core/src/` | Actual target engine fields/rules; does not establish original-game behavior |
| L1 | [Four sample records](samples.json), [section framing](conquests-sections.json) | Direct metadata and bounded local observations; not redistributed file contents |
| T1 | `scripts/compatibility/test_inspect_header.py` | Entirely invented headers and malformed-input behavior; not real SAV coverage |

The public QueryCiv3 readme points to its author's `c3sat/queryciv3` Go precursor. That precursor and third-party forum specifications were not independently audited here; they are candidate cross-checks, not additional corroboration. No official complete binary specification, original checksum specification, or original-runtime comparison was established. OpenCiv3 implementation comments identify unknowns; preserve them as unknowns.

## Exact local reference

English GOG game/root ID `1471405734`; prior M5 executable metadata reports Conquests `1.22.0.0`, base marker `1.29f`. Installer filename `setup_civilization3_complete_2.0.0.7.exe` is not proof of an internal GOG build ID (none recorded). See [M5 provenance](../milestone-5.md). Installer/executable hashes were not recomputed for M6; four selected rules-file hashes were computed before and after analysis and remained equal.

A recursive inventory under `local-data/gog/app` found 28 BIQ, 32 BIC, 59 BIX and **zero SAV** files. Only the four below were decompressed; these counts do not imply the remaining files parse. GOG-supplied bundled scenarios are available; no separately owner-authored mod or SAV set was supplied.

| Format/sample relative to installation | Container | Decoded header/version | Actual examination / confidence |
| --- | --- | --- | --- |
| `civ3mod.bic` | DCL, `00 06` | `BIC ` / 4.1 | L: 17,368 → 111,308 bytes; header only, vanilla body unsupported |
| `civ3PTW/civ3X.bix` | DCL, `00 06` | `BICX` / 11.18 | L: 22,522 → 154,972 bytes; header only, PTW body unsupported |
| `__support/save/Conquests/conquests.biq` | DCL, `00 06` | `BICX` / 12.8 | L: 30,501 → 209,222 bytes; framing and selected GOOD/TECH fields inspected; active standard status U |
| `Conquests/Conquests/1 Mesopotamia.biq` | DCL, `00 06` | `BICX` / 12.7 | L: 27,873 → 272,612 bytes; header only, scenario contents not semantically parsed |
| `BICQ` variant | Not locally observed | R1 recognizes 12 as Conquests | R only; probe labels unverified, no guessed compatibility |
| Original `.sav` | R2 suggests same detection path | R1 uses `CIV3`, marker at 4, major at 6, minor at 10 | R/T only; no verified original version pair or comparison of editions |

All four rules headers have `VER#` at offset 4, record count 1 at 8, length 720 at 12, and little-endian major/minor at 24/28. R3 reads description at 32 (640 bytes), title at 672 (64), sections at 736. The public probe deliberately emits no title/description, user text or paths. Reserved bytes at 16/20 are not interpreted. No cross-edition body compatibility follows from shared offsets.

## Container and parser constraints

A two-byte DCL-looking prefix is only a candidate: it can be malformed or unrelated. The research probe does not decompress it. The separate local reference trial capped decoded output at 16 MiB and each process at 10 seconds; each selected input was under 31 KB. `blast` completed successfully, but the old reference API did not establish exact trailing-input consumption. Do not call this full integrity validation. No embedded checksum was verified; SHA-256 records reproducibility, not authenticity or original-game acceptance.

R3's BIQ layout uses four-byte section tags/counts, many length-prefixed records, variable arrays and special cases. The local Conquests walk reached exactly byte 209,222 after handling FLAV separately. Framing evidence covers BLDG, CTZN, CULT, DIFF, ERAS, ESPN, EXPR, GOOD, GOVT, RULE, PRTO, RACE, TECH, TFRM, TERR, WSIZ, FLAV and GAME; counts/lengths are in the metadata manifest. The scenario/map families CITY, CLNY, CONT, LEAD, SLOC, TILE, UNIT, WCHR and WMAP are present in R3's parser, **not verified in this standard candidate**. Section presence is not a parsed feature, and omitted scenario sections must not be invented.

Future readers need explicit little-endian decoding, checked offset arithmetic, bounded counts and output, consistent lengths, validated cross-references and exact completion. Preserve `-1` sentinels as absence where the specific field proves that convention, never unsigned indexes. Reject unknown versions/required sections until their framing is known; do not scan for printable tags and treat incidental strings as sections. Windows-1252 is reference evidence for English text only; language/encoding and malformed-byte policy remain a gate. Never execute text as Lua or use it as a filesystem path by default.

## SAV feasibility and missing state

R4 reads an embedded BIQ length at offset 38 and starts it at 562, overlays sections on separately supplied rules, then processes GAME, WRLD, TILE, CONT, LEAD, CITY, UNIT and other state. Its Conquests-specific 32-leader expectation and world tile count `width * height / 2` cannot be generalized to every save. City records can contain discarded/dirty entries. Other sections describe dates, replay/history, diplomacy, resource availability, wonders, queues and UI/advisor data. Reference fixed offsets are hypotheses for a future version-gated probe, not a safe parser contract today.

- Header/version inspection: a reasonable first SAV experiment, blocked on real samples for high confidence. The synthetic probe merely returns raw header integers and `unverified_sav`.
- Read-only map/state extraction: feasible in principle after rules resolution and section validation; requires source-to-native coordinates, IDs, ownership, fog and unit/city relationship checks. Partial extraction must label omitted state and cannot be offered as faithful continuation.
- Full import/round-trip: unsupported. Government, happiness, corruption, treaties, wonder effects, leaders/armies, pollution, victory options and further M5 gaps lack equivalent semantics. Original RNG, AI state and turn sequencing are not proven equivalent; a native journal does not exist in the source SAV. Unknown fields/checksums and optional rules prevent a fidelity claim even if visible map fields can be read.

Required owner fixtures: vanilla and Conquests saves with exact original version, matching active rules hash, settings, source checksum, paired before/after one action and one turn, and expected visible outcomes. Keep all originals private and immutable. Use working copies only when a tool writes. No original desktop executable was run in this audit.

## Scenarios, mods and Android paths

R3's GAME has a scenario-search-folder string and default-rule/victory flags plus player/alliance overrides; PRTO/RACE and PediaIcons/INI references lead to artwork. R5 splits scenario search folders on semicolons, tries each relative path followed by `Conquests/Conquests/`, `Conquests/Scenarios/`, and `civ3PTW/Scenarios/` variants. It then tries Conquests, PTW and base assets, with its own standalone/fallback art rules. It normalizes backslashes and parent segments, uses case-insensitive component lookup and can choose the first matching entry. This describes OpenCiv3; GOG precedence and per-asset behavior are U.

Android's existing importer has no arbitrary filesystem-root access or mod-path graph. SAF provides selected document trees/streams; private copies contain only allowlisted media. A desktop relative path can escape the selected tree or reach data that was never granted/copied. A future scenario resolver must therefore be designed explicitly, not reused from desktop path concatenation.

| Input/problem | Current evidence | Proposed bounded behavior; not implemented |
| --- | --- | --- |
| `..\\Conquests\\Scenario`, slash variants | R5 uses parent traversal in legitimate paths | Resolve against a virtual installation manifest constrained to approved roots; reject any escape, or report unsupported legacy path until approved mapping exists |
| Absolute, drive-letter, UNC, URI, NUL/control characters | Arbitrary paths absent from current profile | Reject before opening; never gain access from text in BIQ/INI |
| Case collisions and Unicode names | Existing ASCII-insensitive profile rejects ambiguity | Deterministic case/encoding policy, exact ambiguity diagnostic; never first-match silently |
| Semicolon list, missing mod folder | R5 precedence, R3 field | Bounded list/depth; show requested and attempted logical locations; missing required data blocks activation |
| Missing custom art/sound/rules | Current optional audio is narrowly defined | No silent standard-rules substitution; approve optional-asset fallbacks separately |
| Symlink, cyclic references, malformed indexes | Current local profile rejects symlinks | Reject symlink escapes; visited-node/depth/file/byte limits; resolve references before publishing import |
| Scenario changes between imports | Current importer stages then publishes | Separate source identity and validated manifest, no mutable references into source files; preserve active import on failure |

No resolver is implemented by this milestone, so the probe cannot follow an embedded path at all. Production Android scenario support and a mod manager remain M8. Minimal virtual-root policy versus broader staged-tree import is an approval gate, not a decision made here.

# Milestone 5 — reproducible rules verification

Use with the [audit/matrix](milestone-5.md) and [existing testing commands](testing.md). This is a verification plan. Proposed fixtures and desktop comparisons have not yet been executed. Keep proprietary rules, executables, saves and original-asset screenshots local; CI uses hand-authored synthetic states and public behavioral assertions only.

## Establish the reference

From the repository root, reproduce the metadata hashes without launching or modifying the source installation:

```sh
python3 - <<'PY'
from pathlib import Path
import hashlib
root = Path('local-data/gog/app')
for name in (
    'goggame-1471405734.info',
    'Conquests/readme.txt',
    'Conquests/Civ3Conquests.exe',
    '__support/save/Conquests/conquests.biq',
):
    data = (root / name).read_bytes()
    print(name, len(data), hashlib.sha256(data).hexdigest())
PY
cargo run --locked --bin validate_gog -- local-data/gog/app
```

The import validator checks the M2 media/layout contract, not standard rules parity. Compare hashes with the audit before reusing observations. The metadata extraction used a `VS_FIXEDFILEINFO` signature (`0xFEEF04BD`) in the executable and decoded the two high/low-word pairs for file and product versions; both yielded `1.22.0.0`. This does not prove which BIQ a desktop session loads.

On a user-owned native desktop installation, record executable version/hash, the actual active standard-game rules path/hash, language, civilization, difficulty, map size/settings, optional rules and all enabled victory conditions. Confirm the candidate support BIQ's relationship to the active file. Do not copy files into a new location and assume they are active. No Windows emulation, desktop client development or full scenario reader is part of this milestone. If the desktop reference is unavailable, keep the relevant row unverified and request observations; never substitute a reimplementation's behavior as authority.

## Per-slice evidence and fixture contract

For each named matrix ID, keep a compact comparison record containing:

1. Reference identity/configuration and evidence date; exact starting state and commands, before/after values, turn phase and original-game observed result. Identify manual/rules-text claims separately from runtime observations.
2. A minimal synthetic map and fixed seed. Explicitly set ownership, population, terrain, resource/technology state, buildings, relationships and enabled options needed for the assertion. No original binary map/save is committed.
3. Expected state transitions with numerical values and ordering derived from the reference. Record assumptions and unresolved edge cases before coding; do not use the new implementation to generate its own expected values.
4. An ordinary success case, rejection/boundary case, turn transition and relevant interaction. A rejected command must leave meaningful state and the future deterministic sequence unchanged.
5. Save immediately before and after the transition. Reload using the same contract; compare complete simulation state, queues, turn/current player, command results and the next seeded outcomes. For randomized rules, compare the subsequent random event as well as the immediate result.
6. Source/test names, command output, observed pass/fail and remaining discrepancy. Only mark supported after the stated scope actually matches the reference. Keep incomplete subrules open even if one fixture passes.

The first resource fixtures should isolate unconnected/connected supply, resource on/off the worked tile, unrevealed/revealed technology state, duplicate luxury, route break/reconnect, production eligibility and save/reload. Trade requires relationship/treaty prerequisites; do not claim it from a local connection test. Government/happiness fixtures must include disorder recovery; culture fixtures must include threshold and ownership conflict; treaty fixtures must include expiration versus breach; leader/army, wonder, Golden Age, pollution and victory fixtures must test both triggers and non-triggers. Exact parameters await reference capture.

## Android and CI regression boundary

After a behavior slice, use the current official Google Android testing guidance as required by the handbook, then run the established format/Clippy/Rust tests, synthetic inventory/source-policy/vendor checks and Android build/lint. Any approved vendor patch must have an equally strict explicit provenance check; do not merely update hashes to conceal changes.

On an isolated disposable ARM64 device/emulator, retain this journey:

Import user-supplied GOG data → New Game → move/found city → production/research → diplomacy/combat → end turns → manual save/reload → continue with the same result → background/resume → process-death recovery. Run the existing imported loop and touch harnesses in [testing.md](testing.md), including import failure retention and pan/pinch non-mutation. New diplomacy commands require real transition assertions in addition to the existing read-only encounter screen. Run relevant empty/failure and phone/tablet/landscape/large-text checks when affected.

Do not overwrite wanted device saves. Do not reuse older emulator or CI passes as evidence for changed rules. Each focused PR must identify its contract and tested commit, reference evidence, unresolved gaps, local checks, Captain playtest steps and latest-head required CI result. Human review, merge approval and Captain Android acceptance remain separate gates.

## Initial audit verification record

The audit makes no runtime changes. Its verification consists of source inspection, metadata hashes, the existing core test suite, vendor/source-policy checks and documentation diff review. Android build/lint/device journeys and original-game runtime comparisons are not run for this documentation-only change. No new rules tests exist yet. Detailed observed results are recorded in `milestone-5.md` when checks complete.

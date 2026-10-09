# Milestone 6 — save and data compatibility research

Date: 2026-10-10 (Asia/Tokyo). Owner: Dan. Status: **in progress; findings await review and owner acceptance**. Authority: [Milestone 6 Crew Brief](https://docs.google.com/document/d/182_1nacqFU2GKN-ufsVv-alTY72daWejMzMqFpZnR7A/edit). Branch: `codex/milestone-6-compatibility-research`, starting at accepted M5 merge `ab06228`. The brief records Milestones 1–5 as owner-accepted; M5 acceptance covers the resource foundation, not complete rules parity. Earlier pending-acceptance notes are historical.

## Findings

Read-only rules metadata is feasible. Four local GOG files decompressed with the reference PKWare DCL decoder and exposed distinct versioned headers. The candidate standard Conquests rules contain readable section framing and resource fields, but their use by a running original game remains unverified. Reading these bytes does not make the current engine capable of applying their rules.

Original SAV header inspection is plausible from reference code, but **no original SAV was available** in the supplied installation. No original save metadata, world, turn, player, RNG, checksum or subsequent gameplay was verified. A full original-save import is a no-go for implementation now: the engine lacks significant state and semantics, and Civ3Touch's replay journal cannot be reconstructed by renaming a SAV or injecting a partial world. This is a Crew recommendation, not a new Captain decision.

Keep the reliable distinct Civ3Touch JSON format and both existing rules contracts. There is no export feature, migration, production parser, Android UI change, engine patch, new dependency or binary round-trip in this milestone. The only committed executable addition is an isolated Python header probe with synthetic tests, run by existing CI.

## Reading order and evidence

- [Formats and scenarios](formats.md): source inventory, versions, containers, SAV feasibility, Android path conflicts.
- [Field mapping and compatibility matrix](mapping.md): parsed versus mapped versus applied/tested status, including M5 gaps.
- [Verification and reproduction](verification.md): exact commands, public synthetic fixtures, private-data procedure, blocked checks and handoff.
- [Sample metadata](samples.json): four relative paths, byte counts, source and decompressed SHA-256, signatures and versions. No file contents.
- [Candidate section framing](conquests-sections.json): offsets, counts and record lengths only; not a complete semantic parse.

Evidence labels throughout: **L** = direct local observation; **R** = fixed-revision source inspection, not an original-game specification; **T** = synthetic or existing prototype test; **U** = unverified; **N** = absent from the audited application. Mapping targets are proposals, not implemented imports.

## Audit and discrepancies

`crates/civ3touch-core/src/session.rs::save/load` writes envelope version 1, an exact rules identity, request/result journal and world/queues/turn/player verification. Loading constructs a separate session, replays at most 20,000 actions, compares complete results and state, and rejects saves over 16 MiB. Contracts remain `freec3-90fc7ee-civ3touch-m3-v2` and `freec3-90fc7ee-civ3touch-m5-resources-v1`. Unknown contracts, including M3-v1, remain unsupported. Frozen legacy fixtures and future-turn checks pass. Android `GameSaves.java` retains app-private manual/recovery JSON slots, AtomicFile and readback. No file provided to the research probe reaches those paths.

The pinned FreeC3 Rust source has no original SAV/BIQ/BIC/BIX reader. It loads base Lua definitions; the approved resource patch supplies four static resource definitions. World serialization includes seed/call counters but skips the live RNG; engine/session replay and contract checks remain essential. A parsed original world alone is not a resumable native save.

The current asset importer is a small allowlisted art/audio profile, not a rules/scenario importer. `assets.rs` resolves known paths case-insensitively and rejects collisions/symlinks; the Settler INI cannot redirect reads. Android SAF streams copy only profile files into private staging. Neither arbitrary scenario search folders nor external rule indexes are accepted today.

Discrepancies that matter:

1. The Conquests `.biq` sample has `BICX`, also used by PTW; identify signature **and version**, not filename or signature alone.
2. `__support/save/Conquests/conquests.biq` is a candidate standard-rules file, not proven active installation data. Root `Conquests/conquests.biq` is absent. Do not replace this uncertainty with an asserted standard profile.
3. A general record-length walk fails at `FLAV`; the reference uses a group/count and fixed records there. The corrected local walk reaches EOF, but unknown bytes and section semantics remain unvalidated.
4. GOOD indexes differ from native resource IDs. Horses has original index 0; native index 0 is Wheat. Numeric copying would silently corrupt interpretation.
5. OpenCiv3's header helper defaults unknown classifications to Conquests and scans printable bytes for section names. Neither approach is a safe validation contract for a new reader.

## Proposed implementation order and decision gates

Estimates are rough engineering effort after fixtures are available, including focused tests; not delivery commitments. Each production slice needs a separate brief and Captain approval.

| Phase | Smallest outcome | Effort / risk | Gate and proof before proceeding |
| --- | --- | --- | --- |
| A: metadata | Current research-only header probe and diagnostics | Delivered research / low | Review four observed variants; classify compressed input only as candidate; owner accepts findings |
| B1: bounded decoder | Isolated DCL reader with input/output/work budgets and exact stream consumption | 2–4 days / medium | Approve implementation/dependency choice; synthetic valid/truncated/bomb streams, trailing bytes, known local hashes; no Android integration |
| B2: selected BIQ reader | Conquests 12.8 framing plus GOOD and TECH references into read-only data records | 3–5 days / medium | Confirm active standard file; signed lengths/counts, duplicate/unknown sections, sentinels, bad indexes and exact EOF tests; reject unsupported required fields |
| C: selected rules integration | Explicit ID map and chosen resource fields applied under a new approved contract | 4–8 days / high | Original-game comparison for selected semantics; no dropping unsupported mechanics; frozen old saves and continued-turn checks; legacy-cost review |
| D: scenarios | Bounded scenario manifest and approved search roots, then selected map/state support | 1–3 weeks discovery / high | Milestone 8 authority, Android SAF/path policy and coordinate conversion tests; complete missing-asset report before activation |
| E: optional SAV | Start with versioned read-only metadata/world inspection, never immediate conversion | 1–2 weeks research; full import not estimable / very high | Owner-supplied vanilla/Conquests samples and same-position next-turn reference, complete missing-state inventory, RNG/turn-order fidelity; explicit go/no-go |

Smallest recommended next slice: **B1 then B2, read-only and host-only**, using synthetic framing and locally supplied candidate rules. Do not combine this with production rules loading. Stop on unknown versions or unsupported required sections; return a version/offset/field-specific diagnostic, not partial success presented as compatibility. Existing research spike can be reverted independently; production code, vendor provenance, source game files and existing saves are unchanged.

## Decisions for Dan after review

Approve or revise the next bounded read-only BIQ slice; confirm the candidate standard BIQ against a real GOG Conquests 1.22 installation; provide representative locally retained SAVs only if SAV research should advance. Keep full playable scenarios/mod management at M8 and original SAV conversion behind demonstrated fidelity. No new save format decision is required for this research.

README cross-reference is updated on this branch. Proposed canonical roadmap entry: “Milestone 6 in progress — compatibility research and isolated metadata probe; findings pending owner acceptance; no original-save or scenario-loading support.” The roadmap is not marked complete or edited by this research. Review, Captain testing, merge and release remain separate gates.

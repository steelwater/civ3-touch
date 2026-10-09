# Civ3Touch

The user-supplied Personal Projects handbook governs this project. Current scope is the [Milestone 5 Crew Brief](https://docs.google.com/document/d/1vaHIo3TjDDqFBKw63hY0qtk8eRRrtJJRnE0aSJERGsA/edit): audit and matrix first, then small reviewed rules-compatibility slices and focused PRs. The canonical roadmap marks Milestones 1–4 complete. Preserve their working Android flows and command-replay saves. See `docs/milestone-5.md` for the audit, evidence gaps and unresolved engine/save decisions; `docs/milestone-4.md` for the touch contract; and `docs/milestone-3.md` for simulation/save boundaries. Flag unresolved rule interpretations and architecture choices for Dan before implementation. Milestone acceptance requires review and Captain Android playtesting. No merge or deployment without approval. The [Milestone 0 brief](https://docs.google.com/document/d/1PmYEMGO8M3bInm-TJz_1Kq9a8DWjuNDFUr_58I6Z8ho/edit) remains historical context.

Read `README.md`, `docs/milestone-0-audit.md`, `docs/milestone-1.md`, `docs/milestone-2.md`, `docs/milestone-3.md`, `docs/milestone-4.md`, `docs/milestone-5.md`, and `docs/testing.md` before changing behavior. Keep Android presentation separate from the Rust engine. Do not switch engines or introduce Windows emulation.

Never commit/upload proprietary installer or game payloads. Follow `docs/asset-policy.md`; use synthetic tests in CI. Keep `vendor/freec3` byte-identical to `docs/freec3-baseline.json` unless a specific upstream update/patch is authorized. Format only the Civ3Touch package, not the vendor workspace.

Testing strategy and exact commands: [docs/testing.md](docs/testing.md). No remote/PR/release is authorized merely by the existence of a CI workflow.

Dan approved narrow M5 resource-foundation patches and preservation of legacy save behavior on 2026-10-09. See `docs/milestone-5-resources.md` for the scope and save policy, and `docs/freec3-resource-patches.json` for patch provenance. Keep the original baseline manifest intact; unrelated vendor edits still require approval. Use one engine with targeted contract handling; never silently migrate saves.

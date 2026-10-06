# Civ3Touch

The user-supplied Personal Projects handbook governs this project. Current scope is the [Milestone 2 Crew Brief](https://docs.google.com/document/d/1KpGp8DkI_fEKqae-VUopcJPklU1XAOrtN4EnfugxyjM/edit), including focused Uplink. Milestone 3 gameplay expansion requires a separate mission. The [Milestone 0 brief](https://docs.google.com/document/d/1PmYEMGO8M3bInm-TJz_1Kq9a8DWjuNDFUr_58I6Z8ho/edit) remains historical context.

Read `README.md`, `docs/milestone-0-audit.md`, `docs/milestone-1.md`, `docs/milestone-2.md`, and `docs/testing.md` before changing behavior. Keep Android presentation separate from the Rust engine. Do not switch engines or introduce Windows emulation.

Never commit/upload proprietary installer or game payloads. Follow `docs/asset-policy.md`; use synthetic tests in CI. Keep `vendor/freec3` byte-identical to `docs/freec3-baseline.json` unless a specific upstream update/patch is authorized. Format only the Civ3Touch package, not the vendor workspace.

Testing strategy and exact commands: [docs/testing.md](docs/testing.md). No remote/PR/release is authorized merely by the existence of a CI workflow.

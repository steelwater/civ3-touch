# Contributing

Civ3Touch is an independent GPL-3.0-only project. Submit only code you have the right to contribute under that license. Preserve upstream notices. Identify third-party code, its exact source revision, and its license before incorporating it.

Read the [README](README.md), [asset policy](docs/asset-policy.md), and [test instructions](docs/testing.md). Keep simulation and data handling independent of Android presentation. Work on the approved milestone; do not silently change engines, add a compatibility layer, or expand platform support.

Never attach proprietary installers, game data, screenshots of proprietary assets, sounds, saves, or extracted payloads to commits, issues, CI artifacts, or review documents. Use synthetic test data; keep real-data tests developer-local. Metadata-only path inventories are allowed.

Run the focused checks in `docs/testing.md` and inspect the diff and proposed source set before committing. Treat `vendor/freec3` as immutable until an explicitly documented upstream update or patch is approved. Do not run a formatter across the vendor snapshot.

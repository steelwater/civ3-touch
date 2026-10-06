# GOG reference installation and minimum probe manifest

Milestone 2 now defines the executable import profile and storage/rendering contract in [milestone-2.md](milestone-2.md). The original six-image probe below is historical: the active slice requires four atlases, Settler idle/run, GOG/version markers and optional selected audio.

Reference input: `setup_civilization3_complete_2.0.0.7.exe`, 1,370,659,544 bytes; SHA-256 `4ad54ca308ea93af49b0db3a795736eda91aed61c8cd73b18196802d0f4d2395`.

`innoextract` 1.9 identifies “Sid Meier's Civilization III Complete”, Inno Setup 5.5.0 (Unicode). Extracted GOG metadata reports game/root ID `1471405734`, English. `2.0.0.7` is the package filename identifier; no separate internal GOG build-number field was verified. The base `Text/version.txt` reports `1.29f`; do not mislabel that as the Conquests version.

## Repeatable inspection

```sh
innoextract --list installer/setup_civilization3_complete_2.0.0.7.exe
innoextract --extract --include app --output-dir local-data/gog installer/setup_civilization3_complete_2.0.0.7.exe
python3 scripts/inventory-gog.py local-data/gog/app > docs/gog-inventory.json
cargo run --locked --bin validate_gog -- local-data/gog/app
```

The installer is read-only input. `app/` includes base `Art/`, `Text/`, `Sounds/`, `civ3PTW/`, `Conquests/`, support/install utilities and GOG metadata. The metadata-only inventory records 9,007 files / 1,887,005,990 bytes, with paths, sizes and scope-specific classifications. It does not copy payloads or certify every scenario/file as supported.

## Minimum probe

| Classification | Exact relative paths | Observed result |
| --- | --- | --- |
| Required for terrain decoder probe | `Art/Terrain/xggc.pcx`, `xtgc.pcx`, `xdgc.pcx`, `xdgp.pcx`, `xdpc.pcx`, `xpgc.pcx` under the same directory | All six decode as 1152×576 RGBA |
| Required to locate chosen unit art | `Art/Units/Settler/settler.ini` | INI references `settDefault.flc` and `settRun.flc` |
| Required for animation probe | `Art/Units/Settler/settDefault.flc` | 30×55, eight directions, 15 frames per direction |
| Required for animation probe | `Art/Units/Settler/settRun.flc` | 40×63, eight directions, 10 frames per direction |
| Required for core/map initialization | FreeC3 `mods/base/*.lua`, shipped as open-source source | No original game file required for the synthetic seeded map |
| Optional for later prototype | Other base terrain overlays, city art, further animations, audio | Inventoried, not decoded or rendered by this spike |
| Unknown/deferred | Original BIQ/BIC/BIX rules, scenarios, other expansion content | Original Conquests rules are not imported; FreeC3's Lua rules are the current engine input |
| Runtime-unneeded for spike | Windows EXEs/DLLs, help/manual files | Never executed or packaged |

The original FreeC3 Settler path ends `settler.INI`, while this package uses `settler.ini`. Windows/macOS case folding can conceal that mismatch; Android import must resolve actual case or build a verified case-folded index that rejects collisions. This spike uses the observed exact filenames and does not implement the future importer.

The byte decoders successfully consumed the selected GOG files. This establishes a narrow native data path; it does not prove sprite rendering, every animation chunk, every palette convention, full Conquests rules or save compatibility. The inventory rejects missing exact probe paths, case collisions, and file symlinks escaping the root. Synthetic tests cover missing files, metadata-only output, and escaping links. A future importer must additionally enforce resource limits and validate untrusted files before decoding.

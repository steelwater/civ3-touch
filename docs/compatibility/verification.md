# Milestone 6 verification and reproducibility

Date: 2026-10-10. Scope: research docs, metadata-only manifests, isolated host Python probe and its CI invocation. No Android, Rust, Lua, vendor, save fixture or proprietary input changes.

## Checks and results

| Check | Observed result |
| --- | --- |
| Baseline `cargo test --locked --workspace` before edits | PASS: 22 unit + 7 resource integration tests; legacy save/re-save/future-turn checks included |
| `cargo fmt -p civ3touch-core --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| `bash scripts/test-touch.sh` | PASS: existing camera/gesture unit checks |
| `python3 scripts/test_inventory.py` | PASS: 3 synthetic inventory tests |
| `python3 scripts/verify-upstream.py` | PASS: 115 baseline files and 7 recorded resource patches |
| `python3 scripts/test_vendor_policy.py` | PASS: 4 policy tests |
| `python3 scripts/check-source-policy.py` | PASS before final staging; repeated at handoff |
| `python3 scripts/compatibility/test_inspect_header.py` | PASS: 9 tests; all 736 truncated rules-prefix lengths, unknown versions, bad framing, oversized sparse file, SAV header, compression-candidate honesty, symlink/FIFO rejection, source-byte preservation and inert embedded path |
| `android/gradlew -p android --no-daemon assembleDebug lintDebug` | PASS: 46 tasks, 2 executed/44 up-to-date; existing output reused, not a clean rebuild; no production source changed |
| Android synthetic save/recovery regression | Result recorded in PR/Drive handoff after isolated device run |
| Latest-head required `core-and-android` CI | Result recorded in PR/Drive handoff; local passes are not remote CI |

Commands assume `source .local/env.sh` on this workstation, or equivalent documented environment in [build.md](../build.md). The first baseline attempt had Cargo absent from PATH; sourcing the existing environment fixed it. Sandbox initially blocked the Gradle cache and ADB listener; approved escalation allowed the normal tools. An initial synthetic test exposed an unhashable bytearray header key; normalizing the four signature bytes fixed it, and the full probe suite then passed. The exploratory length walk stopped at FLAV until its distinct source-defined grouping was handled; no production parser or assertions were weakened.

Google's current [testing/testing-setup](https://github.com/android/skills/blob/main/testing/testing-setup/SKILL.md), last-updated 2026-09-23 when read, informed existing-stack review and state-restoration testing. Stack: Java Views/Canvas/JNI, Rust tests, plain Java camera tests and Python/ADB UI Automator journeys. Existing tools suffice; no DI, mocking, screenshot or coverage dependency was installed. R8, Play, AGP migration, profiling and navigation redesign are not material to a host-only metadata probe. Broad responsive/layout/import/audio journeys were not repeated because no production behavior or rendering changed. Physical Android, API-26, 16 KB-page, accessibility and original-runtime comparison remain unrun.

## Public reproducible probe

From repository root:

```sh
python3 scripts/compatibility/test_inspect_header.py
python3 scripts/compatibility/inspect_header.py YOUR_LOCAL_FILE
```

macOS/Linux, Python 3.9+, standard library only. The probe opens an explicit regular file read-only, rejects a final symlink, reads at most 736 bytes and rejects files above 32 MiB. It follows no content-derived paths. Parent components of the explicit caller path are not a sandbox; the future Android scenario policy is separate. It emits only length/signature/version/classification and fixed diagnostic text. No title, description, file body or private absolute path is emitted.

Exit 2 means malformed/truncated/oversized/unreadable input. Exit 0 means classification completed, **not** a validated file or successful game load. `compressed_candidate`, `unverified_sav`, `unverified_version` and `metadata_only` all leave body integrity and gameplay unsupported. A two-byte compression prefix remains only a candidate; the probe makes no claim that it can detect corruption inside compressed data. Unknown counts/layouts fail; unknown version pairs report unverified and do not parse sections. Synthetic fixtures are constructed in test code, never copied from GOG files. The probe is outside Cargo/Gradle/Android packaging and cannot replace a live session.

## Private source reproduction

The exact source hashes, sizes and decoded hashes are in [samples.json](samples.json). Do not commit or upload decompressed outputs, original rules, game saves, artwork, or source-derived complete data tables. Existing [asset policy](../asset-policy.md) applies. A metadata-only installation inventory can be repeated with:

```sh
python3 - <<'PY'
from pathlib import Path
from collections import Counter
root = Path('local-data/gog/app')
print(Counter(p.suffix.lower() for p in root.rglob('*')
              if p.is_file() and p.suffix.lower() in {'.sav', '.bic', '.bix', '.biq'}))
PY
```

The local DCL experiment used unmodified `Blast/reference-code/blast.c` and `blast.h` at OpenCiv3 revision `6a8db067dc2c07ba28b5e648b4c6b0a0b771c769` in ignored `.local/milestone-6/reference/`. It compiled with the existing `cc`, not a new installed dependency. The driver was exactly:

```c
#include <stdio.h>
#include "blast.h"
static unsigned char buf[4096];
static unsigned input(void *p, unsigned char **b) { *b=buf; return (unsigned)fread(buf,1,sizeof(buf),(FILE*)p); }
static int output(void *p,unsigned char *b,unsigned n) { size_t *total=p; if (*total+n>16*1024*1024) return 1; *total+=n; return fwrite(b,1,n,stdout)!=n; }
int main(int argc,char **argv) { if(argc!=2)return 2; FILE *f=fopen(argv[1],"rb"); if(!f)return 2; size_t total=0; int r=blast(input,f,output,&total); fclose(f); return r?3:0; }
```

Fetch the two public reference files from the pinned links in [formats.md](formats.md), preserve their license headers, save the driver as `probe.c` beside them, then:

```sh
cc -Wall -Wextra .local/milestone-6/reference/probe.c \
  .local/milestone-6/reference/blast.c -o .local/milestone-6/reference/probe
```

Two signed/unsigned comparison warnings originate in the unchanged 2003 reference C source. For each of the four metadata-manifest paths, read its bytes to compute SHA-256, run the executable with Python `subprocess.run([probe, source], check=True, capture_output=True, timeout=10)`, verify the decoded length/hash, then hash the source again. Bound source size to the recorded small inputs; this reference trial is not a hostile-file service. `stdout` contains proprietary decoded bytes and must stay in local memory or an ignored local file. Do not print it into public logs. The audit stored only the candidate Conquests decoded working copy under ignored `.local/milestone-6/`; the original remained untouched.

For the exact known candidate hash, framing reproduction starts at 736: read an ASCII tag and little-endian signed count; bound the count to 0..10,000; for ordinary records read signed payload length, require `0 <= length <= remaining - 4`, then advance by `4 + length`. FLAV is exceptional: require one group, read a count of seven, then seven 292-byte records with relationship count seven at record+260. Verify every range before access and require final offset 209,222. This is a candidate-specific experiment, not a general format algorithm. The [framing manifest](conquests-sections.json) records its output. GOOD records use name at +4/24 bytes and eight signed integers at +60: category, appearance, disappearance, icon, prerequisite, food, shields, commerce. Inspect only the four M5 names; resolve prerequisite index 4 against TECH name at +4/32 bytes. All other semantic fields remain reference-only.

## Unavailable evidence and next checks

- No original SAV files: vanilla/Conquests saves, header versions, checksum/length semantics, world extraction and next-turn fidelity remain untested. Do not download unknown-provenance saves to fill this gap.
- Candidate standard BIQ not confirmed active in a running GOG installation. Conquests v1.22 metadata does not independently prove which BIQ was loaded.
- No original desktop runtime, original round-trip, playable scenario, user-authored mod or Android arbitrary-path resolver trial. These are future approved slices.
- DCL output bounds were applied to selected real-data trials, not a production decompression fuzz suite. The committed probe never decompresses. Corpus/fuzzing, input/work budgets and trailing-data handling belong to B1.
- Required remote CI runs on the final PR head; final evidence belongs in the PR and canonical Drive handoff to avoid a commit/CI status loop. Review and owner acceptance stay pending even after green CI.

Rollback: ordinary revert of the two research scripts, CI line and documentation; no native save format changes or data migration to undo. Local proprietary source hashes match before/after; vendor baseline plus recorded patches is unchanged. No merge, release, deployment or cleanup is performed.

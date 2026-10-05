#!/usr/bin/env bash
# Package the exact CI-built APK with its corresponding source and provenance.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${GITHUB_SHA:?Run in GitHub Actions}"
: "${GITHUB_REPOSITORY:?Run in GitHub Actions}"
: "${GITHUB_RUN_ID:?Run in GitHub Actions}"
output="$PWD/.local/ci-artifact"
source_dir="$output/source"
mkdir -p "$source_dir/.cargo"
cp android/app/build/outputs/apk/debug/app-debug.apk "$output/civ3-touch-debug-arm64.apk"
git archive "$GITHUB_SHA" | tar -x -C "$source_dir"
# Include the exact Rust dependency sources, including bundled Lua. Use a
# relative replacement path so the extracted source bundle builds elsewhere.
(cd "$source_dir" && cargo vendor --locked --versioned-dirs dependencies > .cargo/config.toml)
tar -czf "$output/civ3-touch-corresponding-source.tar.gz" -C "$output" source
cat > "$output/BUILD.txt" <<INFO
Civ3Touch development build (GPL-3.0-only additions)
Repository: https://github.com/$GITHUB_REPOSITORY
Commit: $GITHUB_SHA
Workflow run: https://github.com/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID
Built UTC: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Variant: debug; ABI: arm64-v8a; minimum API: 26
This is a Milestone 1 interactive map prototype, not a complete game or production release.
The APK contains FreeC3 Lua rules, no proprietary Civilization III data.

Corresponding source: civ3-touch-corresponding-source.tar.gz
Extract it, read README.md and docs/build.md, and run the documented build.
Rust dependency sources and Cargo offline source-replacement config are included.
JDK, Android SDK/NDK and Gradle remain external build prerequisites.
Licenses and attribution are included in LICENSE, UPSTREAM.md and third-party-notices/.
Each CI runner uses its own debug signing key; later APKs may require uninstalling
an older development APK first. Do not treat this as a stable signing identity.
Actions artifacts expire after 30 days. Approved versioned releases are separate.
INFO
(cd "$output" && sha256sum civ3-touch-debug-arm64.apk civ3-touch-corresponding-source.tar.gz > SHA256SUMS)

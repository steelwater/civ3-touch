# GitHub CI and development APKs

Dan authorized creation of the public `steelwater/civ3-touch` repository and APK builds whenever changes merge into `main` on 2026-10-02. This extends the Milestone 0 repository foundation without adding gameplay or authorizing a production release.

`CI and Android APK` runs on pull requests targeting `main`, every push to `main` (including merges), and manual dispatch. The `core-and-android` job checks the unchanged upstream core, Civ3Touch formatting/Clippy/tests, synthetic inventory tests, source policy and upstream hashes, then builds and lints the Android debug APK. Build/test failures block artifact upload.

For successful `main` runs, the Actions artifact `civ3-touch-debug-arm64-<commit SHA>` contains:

- The exact built ARM64 debug APK.
- Corresponding project source from that commit, all locked Rust dependency sources (including Lua), and portable Cargo source replacement configuration.
- SHA-256 checksums for APK and source archive.
- Build provenance, license/build pointers, and scope/signing limitations.

Download it from the completed run's **Artifacts** section. Retention is 30 days. This is a development artifact, not a GitHub Release; approved versioned release artifacts still belong in GitHub Releases under the handbook's separate workflow. No production signing credentials are introduced. Ephemeral CI debug keys mean later builds may require uninstalling the previous test app.

Main-branch protection should require an up-to-date passing `core-and-android` check and a pull request; prevent force pushes/deletion and apply to administrators. The solo-owner workflow uses zero mandatory approving reviews, so Dan can review and merge his own PR after the check passes. There is no automatic merge or deployment.

No proprietary installer, extracted game files, secrets or local tools are included. GitHub uses only public source and synthetic tests. Device smoke testing remains a Captain/local validation step.

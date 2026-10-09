# Reproducible local builds

Use the repository root for commands unless stated otherwise. Do not install dependencies into source control. Network access is needed for first-time Rust/Gradle dependency resolution. SDK license acceptance belongs to the developer; this workstation reused its already accepted Android SDK licenses.

## Tools

- Rust 1.94.0, rustfmt, Clippy, target `aarch64-linux-android` (pinned in `rust-toolchain.toml`).
- JDK 21; local validation used Android Studio's bundled OpenJDK 21.0.10.
- Android SDK `platforms;android-36`, `build-tools;35.0.0`, NDK `27.2.12479018`.
- Gradle wrapper 8.14.3 with official distribution checksum; AGP 8.10.1.
- Python 3.9+ for metadata/check scripts. `innoextract` 1.9 for optional developer-local GOG inspection.

A local Rust installation may use `CARGO_HOME="$PWD/.local/cargo"` and `RUSTUP_HOME="$PWD/.local/rustup"`; put `$CARGO_HOME/bin` on PATH before using Cargo. The session's ignored `.local/env.sh` records this workstation's tool locations; it is not a portable source file.

On this workstation, the SDK is under `$HOME/Library/Android/sdk`, the project-local NDK is `.local/android-sdk/ndk/27.2.12479018`, and Java is `/Applications/Android Studio.app/Contents/jbr/Contents/Home`. On other machines, set the three environment variables to the actual installed paths. No machine path is built into the source.

## FreeC3 baseline and approved resource patches

The full source snapshot is already present: revision `90fc7eeda2d1914c9306b33a161686fa839b8455`, plus the approved M5 resource patches. Verify original-file provenance and exact patched content with `python3 scripts/verify-upstream.py`; see [patch policy](milestone-5-resources.md). The upstream test command below tests the current patched source.

The baseline's mlua configuration expects external Lua 5.4. Either use the platform's development package (`liblua5.4-dev` plus pkg-config on Ubuntu), or build a local copy:

```sh
mkdir -p .local
curl -fsSL https://www.lua.org/ftp/lua-5.4.8.tar.gz -o .local/lua-5.4.8.tar.gz
tar -xzf .local/lua-5.4.8.tar.gz -C .local
make -C .local/lua-5.4.8 macosx  # use linux on Linux
export LUA_LIB="$PWD/.local/lua-5.4.8/src"
export LUA_LIB_NAME=lua
export LUA_LINK=static
(cd vendor/freec3 && cargo build --locked --workspace)
(cd vendor/freec3 && cargo test --locked --workspace)
(cd vendor/freec3 && cargo clippy --locked --workspace -- -D warnings)
(cd vendor/freec3 && cargo run --locked -p fc3_headless -- --max-turns 2 --quiet)
```

The original baseline has formatting differences with Rust 1.94's formatter. `cargo fmt --all --check` in that workspace reports them; do not modify the pinned snapshot to hide this finding. Civ3Touch's own formatting check is `cargo fmt -p civ3touch-core --check`.

## Civ3Touch and Android

```sh
cargo test --locked --workspace
cargo run --locked --bin smoke -- vendor/freec3/mods/base
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt -p civ3touch-core --check
python3 scripts/test_inventory.py
python3 scripts/verify-upstream.py
python3 scripts/check-source-policy.py
android/gradlew -p android assembleDebug lintDebug
```

Gradle runs `scripts/build-android-core.sh`, using the NDK's API-26 ARM64 clang linker and static Lua 5.4.7 from locked `lua-src`. It copies the `.so` to generated JNI output, and packages only upstream open-source Lua from `vendor/freec3/mods/base`. The desktop renderer is not linked. The native linker requests 16 KB page alignment; this is not a claim of completed 16 KB-device testing.

Fresh build proof without deleting prior Rust outputs: set `CARGO_TARGET_DIR="$PWD/.local/validation-target"`, then run the same commands. Gradle `clean assembleDebug lintDebug` rebuilds generated APK output.

Run `adb devices -l`, choose an ARM64 serial, install the debug APK, then run `python3 scripts/android-acceptance.py SERIAL` for the Milestone 1 touch path. Also run `python3 scripts/android-smoke.py SERIAL`. That harness launches three scoped app instances: success, missing-rules failure, and recovery. It does not clear device logs or uninstall other apps.

# macOS-Only Platform Support Cleanup

## Status

Approved — ready for implementation planning.

## Context

Klyntbot is intended to be a macOS-only application. Linux and Windows artifacts, CI jobs, and source branches were added during early release/CI experimentation. This design removes that cross-platform scaffolding so the repository, CI pipeline, and release process reflect macOS-only support.

## Goals

- Remove Linux and Windows CI/release jobs.
- Remove Linux-specific Docker CI environment.
- Remove Windows icon asset.
- Delete Linux-only Rust crates/modules and platform-specific dead code.
- Simplify desktop UI utilities to macOS-only behavior.
- Update release docs to remove Linux/Windows plans.
- Keep the macOS build, test, and release path fully working.

## Non-goals

- Re-architecting the sandbox or notification systems.
- Adding new macOS features.
- Changing version computation or release naming conventions.

## Design

### 1. CI / release / build infrastructure

| Change | Location | Action |
|---|---|---|
| Delete Docker CI | `docker/Dockerfile.ci`, `.dockerignore`, `scripts/run-docker-ci.sh` | Remove entirely. |
| Release artifact jobs | `.github/workflows/release-build-artifacts.yml` | Delete `build-linux-x86_64` and `build-windows-x86_64`; keep `build-macos-arm64`. |
| Quality gates | `.github/workflows/ci.yml` | Move `rust-quality` and `desktop-ui-quality` to `macos-latest`; drop Linux Tauri system dependencies. |
| Release orchestration | `.github/workflows/release.yml`, `.github/workflows/release-stable.yml` | Switch orchestration jobs to `macos-latest` for consistency. |
| Updater manifest | `scripts/generate-updater-manifest.py` | Remove `linux-x86_64` and `windows-x86_64` from `PLATFORM_MAP`; keep darwin entries. |

### 2. Tauri / desktop crate configuration

| Change | Location | Action |
|---|---|---|
| Icon list | `crates/desktop/tauri.conf.json` | Remove `icons/icon.ico` from the `bundle.icon` array. |
| Icon asset | `crates/desktop/icons/icon.ico` | Delete file. |

### 3. Rust source cleanup

| Change | Location | Action |
|---|---|---|
| Delete helper crate | `crates/klynt-sandbox-helper/` | Remove entire crate. |
| Delete Linux modules | `crates/klynt-sandbox/src/linux.rs`, `bwrap.rs`, `helper_proto.rs` | Remove files. |
| Sandbox lib | `crates/klynt-sandbox/src/lib.rs` | Remove all `#[cfg(target_os = "linux")]` branches and `LinuxSandboxRunner` re-export. |
| Process hardening | `crates/klynt-process-hardening/src/lib.rs` | Remove Linux/Windows/BSD stubs and constants; keep macOS hardening only. |
| Common notify | `crates/common/src/notify.rs` | Remove Linux `notify-send` and Windows PowerShell toast implementations and tests; keep macOS AppleScript path. |
| Desktop notify | `crates/desktop/src/notify.rs` | Remove Linux fallback branch. |
| Exe policy | `crates/klynt-execpolicy/src/executable_name.rs` | Remove Windows executable suffix branch. |
| PTY | `crates/klynt-pty/src/lib.rs` | Remove Linux-only `prctl(PR_SET_PDEATHSIG)` block. |
| Linux tests | `crates/klynt-sandbox/tests/linux_smoke.rs`, `helper_locator.rs`, `bwrap_args.rs` | Remove files. |
| Sandbox Cargo.toml | `crates/klynt-sandbox/Cargo.toml` | Update description; remove `[target.'cfg(target_os = "linux")'.dependencies]`. |
| Workspace deps | `Cargo.toml` | Remove `landlock` and `nix` after confirming no other crate uses them. |

### 4. Desktop UI cleanup

| Change | Location | Action |
|---|---|---|
| Platform paths | `desktop-ui/src/utils/platformPaths.ts` | Collapse to macOS vs. unknown; remove Windows/Linux branches. |
| Shortcuts | `desktop-ui/src/utils/shortcuts.ts` | Simplify to macOS-only labels/behavior. |
| Shortcut tests | `desktop-ui/src/utils/shortcuts.test.ts` | Remove non-macOS tests. |

### 5. Documentation

| Change | Location | Action |
|---|---|---|
| Release plan | `docs/plan-release-cicd.md` | Remove Phase 3 Linux/Windows job plans. |
| Spec plan | `docs/superpowers/plans/2026-06-16-release-versioning-cicd.md` | Remove Linux/Windows CI job specs and platform map entries. |

### 6. Verification

After all changes, run on macOS:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cd crates/desktop && cargo tauri build --target aarch64-apple-darwin
```

## Risks

- Some deleted Linux code may be referenced by shared modules or tests under generic `cfg(unix)` gates. Each removal must be followed by a workspace compile/test pass.
- `crates/klynt-sandbox` may become macOS-only; ensure the public API still satisfies callers.
- Removing `landlock`/`nix` from workspace dependencies is safe only if no remaining crate references them.

## Success criteria

- No `ubuntu-latest`, `windows-latest`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `AppImage`, or `NSIS` references remain in CI/scripts/config.
- `cargo check`, `cargo clippy`, `cargo nextest`, and `cargo tauri build` pass on macOS.
- `scripts/generate-updater-manifest.py` produces only darwin entries.

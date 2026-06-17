# Release CI/CD Plan for Klyntbot

## Goal
Add automated release versioning and build/publish pipeline for the Klynt desktop app, modeled on Tolaria's setup.

## Current State
- Existing CI: `.github/workflows/ci.yml` runs `rust-quality`, `desktop-ui-quality`, and `desktop-build-check` on push/PR to `main` and `dev`.
- Desktop app: Tauri v2 crate in `crates/desktop`, React/Vite frontend in `desktop-ui`, package manager is Bun.
- Tauri updater is already configured in `crates/desktop/tauri.conf.json` with a public key and endpoint pointing at GitHub releases.
- Signing secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_KEY_PASSWORD` are already used in CI.
- No release workflows, no version automation, and no cross-platform artifact publishing.

## Proposed Architecture
Adopt Tolaria's calendar-semver, two-channel model:

| Channel | Trigger | Tag format | Version string | Release type |
|---|---|---|---|---|
| Alpha | Every push to `dev` | `alpha-vYYYY.M.D-alpha.NNNN` | `YYYY.M.D-alpha.N` | GitHub prerelease |
| Stable | Push of git tag | `stable-vYYYY.M.D` or legacy `vYYYY-MM-DD` | `YYYY.M.D` | GitHub release |

- Version is computed in GitHub Actions, then written into:
  - `crates/desktop/tauri.conf.json` (`version`)
  - `crates/desktop/Cargo.toml` (`version`)
- A reusable workflow `.github/workflows/release-build-artifacts.yml` builds cross-platform Tauri bundles.
- Release workflows publish GitHub releases and attach a `latest.json` Tauri updater manifest.

## Phases

### Phase 1: Minimal Alpha Release (macOS first)
1. Create `.github/workflows/release-build-artifacts.yml` with a macOS job that builds:
   - `aarch64-apple-darwin` updater `.app.tar.gz` + `.sig`
   - `x86_64-apple-darwin` updater `.app.tar.gz` + `.sig`
   - Optional `.dmg` controlled by an input flag
2. Create `.github/workflows/release.yml` (alpha) that:
   - Computes calendar-semver alpha version from `dev` pushes
   - Calls `release-build-artifacts.yml`
   - Publishes a GitHub prerelease
   - Generates and attaches `latest.json`
3. Ensure the CMake deployment-target wrapper used in `ci.yml` is also applied to release macOS builds.

### Phase 2: Stable Releases
4. Create `.github/workflows/release-stable.yml` that:
   - Parses version from `stable-v*` or `v20*` tags
   - Calls `release-build-artifacts.yml` with `.dmg` uploads enabled
   - Publishes a GitHub release and `latest.json`

### Phase 3: Cross-Platform Builds
5. Add a Linux job to `release-build-artifacts.yml` building `deb`, `rpm`, and `AppImage` for `x86_64-unknown-linux-gnu`.
6. Add a Windows job to `release-build-artifacts.yml` building NSIS for `x86_64-pc-windows-msvc`.
7. Update alpha and stable release workflows to build a multi-platform `latest.json` (darwin-aarch64, darwin-x86_64, linux-x86_64, windows-x86_64).

### Phase 4: GitHub Pages & Release Pages (optional)
8. Add a workflow or job to deploy `latest.json` and release download/history pages to GitHub Pages.
9. Optionally port/adapt Tolaria's `scripts/build-release-download-page.ts` and `scripts/build-release-history-page.ts`.

## Risks & Decisions
- **Signing/notarization**: Apple certificate secrets may not be configured yet. The first implementation can build unsigned bundles but still produce Tauri updater signatures with the existing private key.
- **Runner cost/time**: Cross-platform release builds are slow. Start with macOS only and gate Linux/Windows behind future work.
- **MLX on macOS release runners**: Already fixed in CI via a CMake wrapper; the same wrapper must be used in release builds.
- **Alpha branch**: Tolaria uses `main`; this plan assumes Klyntbot's active branch `dev` is the alpha channel. Switch to `main` later if the branching model changes.
- **Windows NSIS tooling**: Requires prefetching NSIS 3.11 and `nsis_tauri_utils.dll`; can reuse Tolaria's PowerShell script.

## Acceptance Criteria for Phase 1
- [ ] Pushing to `dev` triggers a successful alpha release workflow.
- [ ] The release is published as a GitHub prerelease with a `alpha-vYYYY.M.D-alpha.NNNN` tag.
- [ ] The release contains macOS updater artifacts and a `latest.json` updater manifest.
- [ ] The `latest.json` version matches the computed alpha version.

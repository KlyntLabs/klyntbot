# Release Process

Klyntbot uses calendar-semver versioning and two release channels: **alpha** and **stable**.

## Versioning

- **Stable:** `YYYY.M.D` (e.g., `2026.6.16`)
- **Alpha:** `YYYY.M.D-alpha.N` (e.g., `2026.6.16-alpha.1`)

Repo files (`Cargo.toml`, `crates/desktop/Cargo.toml`, `crates/desktop/tauri.conf.json`, `desktop-ui/package.json`) stay at placeholder `0.1.0`. The real release version is computed in CI and injected at build time.

## Alpha releases

Alpha releases are created automatically on every push to `main`.

## Stable releases

Create and push a tag:

```bash
git tag stable-v2026.6.16
git push origin stable-v2026.6.16
```

Or use the legacy date tag format:

```bash
git tag v2026-06-16
git push origin v2026-06-16
```

## Release notes

Add a file at `release-notes/<tag>.md` before pushing the stable tag. If the file is missing, CI will generate notes from the git log.

## Local version preview

```bash
python3 scripts/compute-release-version.py stable --tag stable-v2026.6.16
python3 scripts/compute-release-version.py alpha
```

## Required secrets

- `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_KEY_PASSWORD`
- `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY` / `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`
- `WINDOWS_CODE_SIGNING_CERTIFICATE` / `WINDOWS_CODE_SIGNING_CERTIFICATE_PASSWORD` (optional)
- `GITHUB_TOKEN` (provided automatically)

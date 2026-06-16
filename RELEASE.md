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

## Required CI secrets

Configure these at **Settings → Secrets and variables → Actions** in the GitHub repository.

### Tauri updater signing (required)

Tauri requires an Ed25519 key pair to sign update bundles. The public key is embedded in `crates/desktop/tauri.conf.json`; the private key is the repository secret.

1. Generate a key:
   ```bash
   cargo install tauri-cli --locked
   cargo tauri signer generate --password "" -w ~/.tauri/myapp.key
   ```
2. Add the secret value:
   - `TAURI_SIGNING_PRIVATE_KEY` — contents of the generated `.key` file
   - `TAURI_KEY_PASSWORD` — the password used when generating the key (can be empty, but the secret must still exist)

### macOS code signing and notarization (required for signed macOS builds)

- `APPLE_CERTIFICATE` — Base64-encoded `.p12` Developer ID Application certificate
- `APPLE_CERTIFICATE_PASSWORD` — password for the `.p12`
- `APPLE_SIGNING_IDENTITY` — Common Name of the certificate, e.g., `Developer ID Application: Your Name (TEAM_ID)`
- `APPLE_ID` — Apple ID email used for notarization
- `APPLE_PASSWORD` — app-specific password for that Apple ID
- `APPLE_TEAM_ID` — Apple Developer Team ID

To base64-encode the certificate:
```bash
base64 -i certificate.p12 -o certificate.p12.b64
```
Paste the contents of `certificate.p12.b64` into the `APPLE_CERTIFICATE` secret.

### Windows code signing (optional)

- `WINDOWS_CODE_SIGNING_CERTIFICATE` — Base64-encoded `.pfx` code-signing certificate
- `WINDOWS_CODE_SIGNING_CERTIFICATE_PASSWORD` — password for the `.pfx`

If omitted, Windows builds will still run but installers will be unsigned.

### GitHub token

- `GITHUB_TOKEN` — provided automatically by GitHub Actions; do not create manually.

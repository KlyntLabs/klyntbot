# macOS Code Signing & Notarization for Klynt

Klynt is a macOS-only Tauri v2 application. This document explains how signing works in CI and how to switch from ad-hoc signing to a proper Apple Developer ID distribution signature.

## Current setup

The release workflow (`.github/workflows/release-build-artifacts.yml`) builds a macOS `.app` bundle, `.dmg`, and Tauri updater `.tar.gz` artifacts. It supports two signing modes:

1. **Apple Developer ID signing + notarization** — required for public distribution.
2. **Ad-hoc signing** — fallback when no Apple certificate secret is present. The app is signed with the pseudo-identity `-`. Users must right-click → Open or run `xattr -cr` on the `.app`/`.dmg` because Gatekeeper will reject it.

## Required GitHub secrets

| Secret | Purpose | Required for ad-hoc? |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Minisign private key used to sign updater `.tar.gz` files | Yes |
| `TAURI_KEY_PASSWORD` | Password for the minisign private key | Yes |
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` certificate exported from Keychain Access | No |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` export | No (unless cert is password-protected) |
| `APPLE_SIGNING_IDENTITY` | Certificate identity, e.g. `Developer ID Application: Klynt Labs Inc (XXXXXXXXXX)` | No |
| `APPLE_ID` | Apple ID email for notarization | No |
| `APPLE_PASSWORD` | App-specific password for the Apple ID | No |
| `APPLE_TEAM_ID` | Apple Developer Team ID | No |

## Generating / exporting an Apple Developer ID certificate

1. Enroll in the [Apple Developer Program](https://developer.apple.com/programs/) (paid account).
2. In Xcode or the Developer portal, create a **Developer ID Application** certificate.
3. Download the `.cer` file and import it into Keychain Access on a Mac.
4. In Keychain Access, select the imported certificate **and its private key**, then choose **File → Export Items** and save as `.p12`.
5. Encode the `.p12` file as base64:
   ```bash
   base64 -i KlyntDeveloperID.p12 -o KlyntDeveloperID.p12.b64
   ```
6. Set the GitHub secrets:
   ```bash
   gh secret set APPLE_CERTIFICATE --repo KlyntLabs/klyntbot < KlyntDeveloperID.p12.b64
   gh secret set APPLE_CERTIFICATE_PASSWORD --repo KlyntLabs/klyntbot
   gh secret set APPLE_SIGNING_IDENTITY --repo KlyntLabs/klyntbot
   gh secret set APPLE_ID --repo KlyntLabs/klyntbot
   gh secret set APPLE_PASSWORD --repo KlyntLabs/klyntbot
   gh secret set APPLE_TEAM_ID --repo KlyntLabs/klyntbot
   ```

Once these secrets are set, the release workflow will automatically use real signing and notarization instead of ad-hoc signing.

## Local ad-hoc build

To build a release DMG locally without an Apple certificate:

```bash
export APPLE_SIGNING_IDENTITY="-"
cd crates/desktop
cargo tauri build --target aarch64-apple-darwin
```

The resulting `.dmg` will be ad-hoc signed and will trigger Gatekeeper warnings when moved to another Mac.

## References

- [Tauri v2 macOS signing docs](https://v2.tauri.app/distribute/sign/macos/)
- [Apple: Notarizing macOS software](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

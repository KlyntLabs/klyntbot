# Release, Upgrade, and Checkin Version + CI/CD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement automated release/upgrade/checkin-version management and CI/CD for Klyntbot, adapted from Tolaria's calendar-semver alpha/stable channels.

**Architecture:** GitHub Actions runs quality gates on every PR/push to main and produces alpha releases on every main push. Stable releases trigger from `stable-vYYYY.M.D` or `vYYYY-MM-DD` tags. The CI workflow computes the release version from git tags/calendar rules, injects it at build time into `crates/desktop/Cargo.toml`, `crates/desktop/tauri.conf.json`, and `desktop-ui/package.json`, then builds Tauri updater artifacts and attaches a `latest.json` manifest to the GitHub Release. Missing Rust settings commands are implemented so the frontend can query build type, runtime, and current version.

**Tech Stack:** GitHub Actions, Rust/Cargo, Tauri 2, Bun, GitHub Releases, Tauri updater plugin.

---

## Context

- Tolaria keeps repo files at a placeholder `0.1.0`, computes the real version in CI, and injects it at build time.
- Tolaria uses calendar-semver: stable `YYYY.M.D`, alpha `YYYY.M.D-alpha.N`.
- Tolaria hosts two updater feeds: `/stable/latest.json` and `/alpha/latest.json` (currently on GitHub Pages; for Klyntbot we will attach them to GitHub Releases).
- Klyntbot currently has no CI/CD, version mismatches (Rust/Tauri `0.1.1`, UI `0.7.68`, changelog `0.1.0`), wrong updater/release-notes repo references, and missing Rust settings commands.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `.github/workflows/ci.yml` | Quality gates on PR/push to main |
| `.github/workflows/release.yml` | Alpha release on every push to main |
| `.github/workflows/release-stable.yml` | Stable release on tag push |
| `.github/workflows/release-build-artifacts.yml` | Reusable Tauri artifact builder |
| `scripts/compute-release-version.py` | Compute calendar-semver version from tags |
| `scripts/inject-release-version.py` | Stamp version into Cargo.toml, tauri.conf.json, package.json |
| `scripts/generate-updater-manifest.py` | Build Tauri-compatible `latest.json` from built artifacts |
| `scripts/fetch-release-notes.py` | Resolve release notes from `release-notes/${tag}.md` or git log |
| `Cargo.toml` | Workspace version placeholder |
| `crates/desktop/Cargo.toml` | Tauri crate version placeholder |
| `crates/desktop/tauri.conf.json` | Tauri app version + updater endpoint |
| `desktop-ui/package.json` | UI display version |
| `desktop-ui/vite.config.ts` | Reads package.json for `__APP_VERSION__` |
| `desktop-ui/src/api/endpoints/settings.ts` | Calls Rust settings commands |
| `crates/desktop/src/commands/settings.rs` | New file: settings/version commands |
| `crates/desktop/src/commands/mod.rs` | Registers new commands |
| `crates/desktop/src/lib.rs` | Expose `AppSettings` types |
| `release-notes/` | Markdown release notes keyed by tag |
| `CHANGELOG.md` | Human-facing changelog |
| `RELEASE.md` | Release process documentation |

---

## Phase 1: Version Alignment

### Task 1: Normalize placeholder versions in repo files

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/tauri.conf.json`
- Modify: `desktop-ui/package.json`
- Modify: `desktop-ui.bak/package.json` (keep in sync as placeholder)

- [ ] **Step 1: Set workspace version to placeholder `0.1.0`**

```toml
# Cargo.toml
[workspace.package]
version = "0.1.0"
```

- [ ] **Step 2: Ensure desktop crate uses workspace version**

```toml
# crates/desktop/Cargo.toml
[package]
version.workspace = true
```

- [ ] **Step 3: Set tauri.conf.json version to placeholder**

```json
{
  "version": "0.1.0"
}
```

- [ ] **Step 4: Set desktop-ui package.json version to placeholder**

```json
{
  "version": "0.1.0"
}
```

- [ ] **Step 5: Run local checks**

```bash
cd /Users/jayden/Projects/Klynt/bot
cargo check -p desktop
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/desktop/Cargo.toml crates/desktop/tauri.conf.json desktop-ui/package.json desktop-ui.bak/package.json
git commit -m "chore: normalize repo versions to placeholder 0.1.0 for CI injection"
```

### Task 2: Fix CHANGELOG unreleased header

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Update `[Unreleased]` header to describe the next stable release generically**

Change the line "The upcoming `0.1.0` release..." to describe the upcoming calendar-semver stable release or remove the explicit version.

- [ ] **Step 2: Add a note about CI/CD being added**

```markdown
### Changed
- Add GitHub Actions CI/CD and automated alpha/stable release pipeline.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): reflect new CI/CD and placeholder versioning"
```

---

## Phase 2: CI Quality Gates

### Task 3: Create CI quality-gate workflow

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the workflow file**

```yaml
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  rust-quality:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Rust cache
        uses: Swatinem/rust-cache@v2

      - name: cargo fmt
        run: cargo fmt --all --check

      - name: cargo clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

      - name: cargo test
        run: cargo test --workspace --all-features

  desktop-ui-quality:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install dependencies
        working-directory: desktop-ui
        run: bun install --frozen-lockfile

      - name: Lint
        working-directory: desktop-ui
        run: bun run lint

      - name: Type check
        working-directory: desktop-ui
        run: bun run typecheck

      - name: Test
        working-directory: desktop-ui
        run: bun run test

  desktop-build-check:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: Swatinem/rust-cache@v2

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install dependencies
        working-directory: desktop-ui
        run: bun install --frozen-lockfile

      - name: Build desktop bundle
        run: cargo tauri build --no-bundle
```

- [ ] **Step 2: Validate YAML syntax**

```bash
python3 - <<'PY'
import yaml, pathlib
yaml.safe_load(pathlib.Path('.github/workflows/ci.yml').read_text())
print('ci.yml is valid YAML')
PY
```

Expected: prints "ci.yml is valid YAML".

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add quality gate workflow for rust and desktop-ui"
```

---

## Phase 3: Release Version Scripts

### Task 4: Create version computation script

**Files:**
- Create: `scripts/compute-release-version.py`

- [ ] **Step 1: Create the script**

```python
#!/usr/bin/env python3
"""Compute the next calendar-semver release version from git tags.

Stable tag formats:
  stable-vYYYY.M.D  -> version YYYY.M.D
  vYYYY-MM-DD       -> version YYYY.M.D (legacy)

Alpha:
  Every call produces the next alpha sequence for today's calendar version,
  bumping the day if it would not be greater than the latest stable date.
"""
import argparse
import re
import subprocess
import sys
from datetime import date, timedelta


def run(cmd: list[str]) -> str:
    return subprocess.check_output(cmd, text=True).strip()


def latest_stable_date() -> date | None:
    tags = run(["git", "tag", "--list", "stable-v*", "v20*"]).splitlines()
    latest: date | None = None
    for tag in tags:
        m = re.fullmatch(r"stable-v(\d{4})\.(\d{1,2})\.(\d{1,2})", tag)
        if m:
            d = date(int(m.group(1)), int(m.group(2)), int(m.group(3)))
        else:
            m = re.fullmatch(r"v(\d{4})-(\d{2})-(\d{2})", tag)
            if not m:
                continue
            d = date(int(m.group(1)), int(m.group(2)), int(m.group(3)))
        if latest is None or d > latest:
            latest = d
    return latest


def next_alpha_version(calendar_version: str) -> tuple[str, str, str]:
    prefix = f"alpha-v{calendar_version}-alpha."
    tags = run(["git", "tag", "--list", f"{prefix}*"]).splitlines()
    seq = 0
    for tag in tags:
        m = re.fullmatch(re.escape(prefix) + r"(\d+)", tag)
        if m:
            seq = max(seq, int(m.group(1)))
    seq += 1
    version = f"{calendar_version}-alpha.{seq}"
    tag = f"{prefix}{seq:04d}"
    display = f"Alpha {calendar_version}.{seq}"
    return version, tag, display


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("channel", choices=["alpha", "stable"])
    parser.add_argument("--tag", help="Stable tag being released")
    args = parser.parse_args()

    if args.channel == "stable":
        if not args.tag:
            print("--tag is required for stable channel", file=sys.stderr)
            return 1
        m = re.fullmatch(r"stable-v(\d{4})\.(\d{1,2})\.(\d{1,2})", args.tag)
        if m:
            version = f"{m.group(1)}.{m.group(2)}.{m.group(3)}"
        else:
            m = re.fullmatch(r"v(\d{4})-(\d{2})-(\d{2})", args.tag)
            if not m:
                print(f"Unsupported stable tag: {args.tag}", file=sys.stderr)
                return 1
            version = f"{m.group(1)}.{int(m.group(2))}.{int(m.group(3))}"
        print(f"version={version}")
        print(f"tag={args.tag}")
        print(f"display={version}")
        return 0

    today = date.today()
    stable = latest_stable_date()
    alpha_date = today if stable is None or today > stable else stable + timedelta(days=1)
    calendar_version = f"{alpha_date.year}.{alpha_date.month}.{alpha_date.day}"
    version, tag, display = next_alpha_version(calendar_version)
    print(f"version={version}")
    print(f"tag={tag}")
    print(f"display={display}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Make executable and test**

```bash
chmod +x scripts/compute-release-version.py
python3 scripts/compute-release-version.py stable --tag stable-v2026.6.16
python3 scripts/compute-release-version.py alpha
```

Expected: stable prints `version=2026.6.16`; alpha prints a version like `version=2026.6.16-alpha.N`.

- [ ] **Step 3: Commit**

```bash
git add scripts/compute-release-version.py
git commit -m "build: add calendar-semver release version computation script"
```

### Task 5: Create version injection script

**Files:**
- Create: `scripts/inject-release-version.py`

- [ ] **Step 1: Create the script**

```python
#!/usr/bin/env python3
"""Inject a release version into Cargo.toml, tauri.conf.json, and package.json."""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def inject_cargo_toml(path: Path, version: str) -> None:
    text = path.read_text()
    text = re.sub(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', text, flags=re.M)
    path.write_text(text)


def inject_tauri_conf(path: Path, version: str) -> None:
    data = json.loads(path.read_text())
    data["version"] = version
    path.write_text(json.dumps(data, indent=2) + "\n")


def inject_package_json(path: Path, version: str) -> None:
    data = json.loads(path.read_text())
    data["version"] = version
    path.write_text(json.dumps(data, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version")
    args = parser.parse_args()

    inject_cargo_toml(ROOT / "crates" / "desktop" / "Cargo.toml", args.version)
    inject_tauri_conf(ROOT / "crates" / "desktop" / "tauri.conf.json", args.version)
    inject_package_json(ROOT / "desktop-ui" / "package.json", args.version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Make executable and test**

```bash
chmod +x scripts/inject-release-version.py
python3 scripts/inject-release-version.py 2026.6.16-alpha.1
grep -E '^version' crates/desktop/Cargo.toml
grep '"version"' crates/desktop/tauri.conf.json
grep '"version"' desktop-ui/package.json
```

Expected: all three show `2026.6.16-alpha.1`.

- [ ] **Step 3: Revert the test changes**

```bash
git checkout -- crates/desktop/Cargo.toml crates/desktop/tauri.conf.json desktop-ui/package.json
```

- [ ] **Step 4: Commit**

```bash
git add scripts/inject-release-version.py
git commit -m "build: add release version injection script"
```

---

## Phase 4: Reusable Artifact Builder

### Task 6: Create `release-build-artifacts.yml`

**Files:**
- Create: `.github/workflows/release-build-artifacts.yml`

- [ ] **Step 1: Create the reusable workflow**

```yaml
name: Release Build Artifacts

on:
  workflow_call:
    inputs:
      version:
        required: true
        type: string
      upload_url:
        required: true
        type: string
      channel:
        required: true
        type: string

jobs:
  build-macos-aarch64:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Rust cache
        uses: Swatinem/rust-cache@v2

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install dependencies
        working-directory: desktop-ui
        run: bun install --frozen-lockfile

      - name: Inject release version
        run: python3 scripts/inject-release-version.py ${{ inputs.version }}

      - name: Build Tauri app
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: cargo tauri build --target aarch64-apple-darwin

      - name: Upload artifacts
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ inputs.upload_url }}
          asset_path: ./target/aarch64-apple-darwin/release/bundle/macos/*.dmg
          asset_name: Klynt_${{ inputs.version }}_aarch64.dmg
          asset_content_type: application/octet-stream

```

> **Note:** `actions/upload-release-asset` is deprecated. Replace with `gh release upload` or `softprops/action-gh-release` before production use.

- [ ] **Step 2: Validate YAML**

```bash
python3 - <<'PY'
import yaml, pathlib
yaml.safe_load(pathlib.Path('.github/workflows/release-build-artifacts.yml').read_text())
print('release-build-artifacts.yml is valid YAML')
PY
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-build-artifacts.yml
git commit -m "ci: add reusable release artifact builder"
```

---

## Phase 5: Alpha Release Workflow

### Task 7: Create `release.yml`

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create the workflow**

```yaml
name: Release Alpha

on:
  push:
    branches: [main]

permissions:
  contents: write

jobs:
  version:
    runs-on: macos-latest
    outputs:
      version: ${{ steps.compute.outputs.version }}
      tag: ${{ steps.compute.outputs.tag }}
      display: ${{ steps.compute.outputs.display }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Compute version
        id: compute
        run: |
          python3 scripts/compute-release-version.py alpha >> "$GITHUB_OUTPUT"

      - name: Show version
        run: |
          echo "version=${{ steps.compute.outputs.version }}"
          echo "tag=${{ steps.compute.outputs.tag }}"
          echo "display=${{ steps.compute.outputs.display }}"

  create-release:
    needs: version
    runs-on: macos-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Generate release notes
        id: notes
        run: |
          echo 'NOTES<<EOF' >> "$GITHUB_ENV"
          git log --pretty=format:'- %s' $(git describe --tags --abbrev=0 --match 'alpha-v*' 2>/dev/null || echo HEAD~50)..HEAD >> "$GITHUB_ENV"
          echo '' >> "$GITHUB_ENV"
          echo 'EOF' >> "$GITHUB_ENV"

      - name: Create GitHub Release
        id: create_release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.version.outputs.tag }}
          name: ${{ needs.version.outputs.display }}
          body: ${{ env.NOTES }}
          prerelease: true

  build-artifacts:
    needs: [version, create-release]
    uses: ./.github/workflows/release-build-artifacts.yml
    with:
      version: ${{ needs.version.outputs.version }}
      upload_url: ${{ needs.create-release.outputs.upload_url }}
      channel: alpha
    secrets: inherit

  publish-manifest:
    needs: [version, build-artifacts]
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download release assets
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release download ${{ needs.version.outputs.tag }} --dir ./release-assets

      - name: Generate latest.json
        run: |
          python3 scripts/generate-updater-manifest.py \
            --version ${{ needs.version.outputs.version }} \
            --channel alpha \
            --assets ./release-assets \
            --repo ${{ github.repository }} \
            --tag ${{ needs.version.outputs.tag }} \
            --output alpha-latest.json

      - name: Upload manifest to release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release upload ${{ needs.version.outputs.tag }} alpha-latest.json --clobber
```

- [ ] **Step 2: Validate YAML**

```bash
python3 - <<'PY'
import yaml, pathlib
yaml.safe_load(pathlib.Path('.github/workflows/release.yml').read_text())
print('release.yml is valid YAML')
PY
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add alpha release workflow"
```

---

## Phase 6: Stable Release Workflow

### Task 8: Create `release-stable.yml`

**Files:**
- Create: `.github/workflows/release-stable.yml`

- [ ] **Step 1: Create the workflow**

```yaml
name: Release Stable

on:
  push:
    tags:
      - 'stable-v*'
      - 'v20*'

permissions:
  contents: write

jobs:
  version:
    runs-on: macos-latest
    outputs:
      version: ${{ steps.compute.outputs.version }}
      tag: ${{ steps.compute.outputs.tag }}
      display: ${{ steps.compute.outputs.display }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Compute version
        id: compute
        run: |
          TAG=${GITHUB_REF#refs/tags/}
          python3 scripts/compute-release-version.py stable --tag "$TAG" >> "$GITHUB_OUTPUT"

  create-release:
    needs: version
    runs-on: macos-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Resolve release notes
        id: notes
        run: |
          TAG="${{ needs.version.outputs.tag }}"
          FILE="release-notes/${TAG}.md"
          if [[ -f "$FILE" ]]; then
            echo "body_path=$FILE" >> "$GITHUB_OUTPUT"
          else
            echo 'NOTES<<EOF' >> "$GITHUB_ENV"
            git log --pretty=format:'- %s' $(git describe --tags --abbrev=0 --match 'stable-v*' 2>/dev/null || echo HEAD~50)..HEAD >> "$GITHUB_ENV"
            echo '' >> "$GITHUB_ENV"
            echo 'EOF' >> "$GITHUB_ENV"
            echo "body=${{ env.NOTES }}" >> "$GITHUB_OUTPUT"
          fi

      - name: Create GitHub Release
        id: create_release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ needs.version.outputs.tag }}
          name: ${{ needs.version.outputs.display }}
          body_path: ${{ steps.notes.outputs.body_path }}
          body: ${{ steps.notes.outputs.body }}
          prerelease: false

  build-artifacts:
    needs: [version, create-release]
    uses: ./.github/workflows/release-build-artifacts.yml
    with:
      version: ${{ needs.version.outputs.version }}
      upload_url: ${{ needs.create-release.outputs.upload_url }}
      channel: stable
    secrets: inherit

  publish-manifest:
    needs: [version, build-artifacts]
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download release assets
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release download ${{ needs.version.outputs.tag }} --dir ./release-assets

      - name: Generate latest.json
        run: |
          python3 scripts/generate-updater-manifest.py \
            --version ${{ needs.version.outputs.version }} \
            --channel stable \
            --assets ./release-assets \
            --repo ${{ github.repository }} \
            --tag ${{ needs.version.outputs.tag }} \
            --output latest.json

      - name: Upload manifest to release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          gh release upload ${{ needs.version.outputs.tag }} latest.json --clobber
```

- [ ] **Step 2: Validate YAML**

```bash
python3 - <<'PY'
import yaml, pathlib
yaml.safe_load(pathlib.Path('.github/workflows/release-stable.yml').read_text())
print('release-stable.yml is valid YAML')
PY
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-stable.yml
git commit -m "ci: add stable release workflow"
```

---

## Phase 7: Updater Manifest Generation

### Task 9: Create manifest generation script

**Files:**
- Create: `scripts/generate-updater-manifest.py`

- [ ] **Step 1: Create the script**

```python
#!/usr/bin/env python3
"""Generate a Tauri v2 compatible latest.json from built release assets."""
import argparse
import base64
import json
import re
from datetime import datetime, timezone
from pathlib import Path

PLATFORM_MAP = {
    "aarch64.dmg": "darwin-aarch64",
    "x86_64.dmg": "darwin-x86_64",
}


def find_asset(assets_dir: Path, suffix: str) -> Path | None:
    for p in assets_dir.iterdir():
        if p.is_file() and p.name.endswith(suffix):
            return p
    return None


def signature_for(asset: Path) -> str:
    sig_file = asset.with_suffix(asset.suffix + ".sig")
    if sig_file.exists():
        return sig_file.read_text().strip()
    return ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--channel", choices=["alpha", "stable"], required=True)
    parser.add_argument("--assets", required=True, type=Path)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    base_url = f"https://github.com/{args.repo}/releases/download/{args.tag}"
    platforms: dict[str, dict[str, str]] = {}

    for suffix, platform in PLATFORM_MAP.items():
        asset = find_asset(args.assets, suffix)
        if asset is None:
            continue
        url = f"{base_url}/{asset.name}"
        entry: dict[str, str] = {
            "signature": signature_for(asset),
            "url": url,
        }
        if suffix == "aarch64.dmg":
            entry["dmg_url"] = url
        platforms[platform] = entry

    manifest = {
        "version": args.version,
        "notes": f"{args.channel.capitalize()} release {args.version}",
        "pub_date": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "platforms": platforms,
    }

    Path(args.output).write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Make executable and test with dummy assets**

```bash
chmod +x scripts/generate-updater-manifest.py
mkdir -p /tmp/klynt-test-assets
touch /tmp/klynt-test-assets/Klynt_2026.6.16-alpha.1_aarch64.dmg
echo "dummy-sig" > /tmp/klynt-test-assets/Klynt_2026.6.16-alpha.1_aarch64.dmg.sig
python3 scripts/generate-updater-manifest.py \
  --version 2026.6.16-alpha.1 \
  --channel alpha \
  --assets /tmp/klynt-test-assets \
  --repo KlyntLabs/klyntbot \
  --tag alpha-v2026.6.16-alpha.0001 \
  --output /tmp/klynt-test-latest.json
cat /tmp/klynt-test-latest.json
```

Expected: JSON with `darwin-aarch64` platform.

- [ ] **Step 3: Commit**

```bash
git add scripts/generate-updater-manifest.py
git commit -m "build: add Tauri updater manifest generator"
```

---

## Phase 8: Fix Updater Endpoint & Release Notes Repo

### Task 10: Fix `tauri.conf.json` updater endpoint

**Files:**
- Modify: `crates/desktop/tauri.conf.json`

- [ ] **Step 1: Update endpoint**

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/KlyntLabs/klyntbot/releases/latest/download/latest.json"
      ]
    }
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/desktop/tauri.conf.json
git commit -m "fix(updater): point manifest at KlyntLabs/klyntbot releases"
```

### Task 11: Fix frontend release-notes fetch URL

**Files:**
- Modify: `desktop-ui/src/features/update/utils/postUpdateRelease.ts`

- [ ] **Step 1: Update constants**

```typescript
const GITHUB_RELEASES_API_BASE = "https://api.github.com/repos/KlyntLabs/klyntbot/releases";
const GITHUB_RELEASES_WEB_BASE = "https://github.com/KlyntLabs/klyntbot/releases";
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/update/utils/postUpdateRelease.ts
git commit -m "fix(update): point release notes fetch to KlyntLabs/klyntbot"
```

---

## Phase 9: Backend Settings / Checkin Commands

### Task 12: Add settings command handlers

**Files:**
- Create: `crates/desktop/src/commands/settings.rs`
- Modify: `crates/desktop/src/commands/mod.rs`
- Modify: `crates/desktop/src/lib.rs` (if needed to expose types)

- [ ] **Step 1: Read existing commands module**

```bash
cat crates/desktop/src/commands/mod.rs
```

- [ ] **Step 2: Create settings.rs**

```rust
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub release_channel: String,
    pub automatic_app_update_checks_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppBuildInfo {
    pub version: String,
    pub build_type: String,
    pub is_mobile: bool,
}

#[command]
pub async fn get_app_settings() -> Result<AppSettings, String> {
    Ok(AppSettings {
        release_channel: "stable".into(),
        automatic_app_update_checks_enabled: true,
    })
}

#[command]
pub async fn update_app_settings(settings: AppSettings) -> Result<(), String> {
    // TODO: persist settings to disk / config crate
    let _ = settings;
    Ok(())
}

#[command]
pub fn app_build_type() -> String {
    if cfg!(debug_assertions) {
        "debug".into()
    } else {
        "release".into()
    }
}

#[command]
pub fn is_mobile_runtime() -> bool {
    false
}

#[command]
pub fn app_build_info() -> AppBuildInfo {
    AppBuildInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        build_type: app_build_type(),
        is_mobile: is_mobile_runtime(),
    }
}
```

- [ ] **Step 3: Register commands in commands/mod.rs**

Add to the existing `generate_handler!` or command registration:

```rust
pub mod settings;

pub fn handlers<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        settings::get_app_settings,
        settings::update_app_settings,
        settings::app_build_type,
        settings::is_mobile_runtime,
        settings::app_build_info,
        // existing commands...
    ]
}
```

Adapt this to the existing registration style in `crates/desktop/src/commands/mod.rs`.

- [ ] **Step 4: Build desktop crate**

```bash
cargo check -p desktop
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/settings.rs crates/desktop/src/commands/mod.rs
git commit -m "feat(desktop): add settings and build-info commands"
```

---

## Phase 10: Release Notes Layout

### Task 13: Create release-notes directory

**Files:**
- Create: `release-notes/.gitkeep`

- [ ] **Step 1: Create directory and placeholder**

```bash
mkdir -p release-notes
touch release-notes/.gitkeep
```

- [ ] **Step 2: Commit**

```bash
git add release-notes/.gitkeep
git commit -m "chore: add release-notes directory"
```

---

## Phase 11: Documentation

### Task 14: Create `RELEASE.md`

**Files:**
- Create: `RELEASE.md`

- [ ] **Step 1: Write release process doc**

```markdown
# Release Process

Klyntbot uses calendar-semver versioning and two release channels: **alpha** and **stable**.

## Versioning

- **Stable:** `YYYY.M.D` (e.g., `2026.6.16`)
- **Alpha:** `YYYY.M.D-alpha.N` (e.g., `2026.6.16-alpha.1`)

Repo files (`Cargo.toml`, `tauri.conf.json`, `desktop-ui/package.json`) stay at placeholder `0.1.0`. The real release version is computed in CI and injected at build time.

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

## Required secrets

- `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_KEY_PASSWORD`
- `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY` / `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`
- `GITHUB_TOKEN` (provided automatically)
```

- [ ] **Step 2: Commit**

```bash
git add RELEASE.md
git commit -m "docs: add release process documentation"
```

### Task 15: Update `workspace/AGENTS.md`

**Files:**
- Modify: `workspace/AGENTS.md`

- [ ] **Step 1: Add release/version conventions**

Append a section:

```markdown
## Release / versioning conventions

- Repo versions remain at `0.1.0`; CI injects the real release version.
- Use calendar-semver: stable `YYYY.M.D`, alpha `YYYY.M.D-alpha.N`.
- Alpha releases run automatically on `main`; stable releases trigger from `stable-vYYYY.M.D` tags.
- Release notes live in `release-notes/<tag>.md`.
- Run `python3 scripts/compute-release-version.py alpha` locally to preview the next alpha version.
```

- [ ] **Step 2: Commit**

```bash
git add workspace/AGENTS.md
git commit -m "docs(agents): document release and versioning conventions"
```

---

## Phase 12: Verification

### Task 16: Run local checks

- [ ] **Step 1: Rust checks**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

- [ ] **Step 2: Desktop UI checks**

```bash
cd desktop-ui
bun install --frozen-lockfile
bun run lint
bun run typecheck
bun run test
```

- [ ] **Step 3: Validate GitHub Actions YAML**

```bash
python3 - <<'PY'
import yaml
from pathlib import Path
for f in Path('.github/workflows').glob('*.yml'):
    yaml.safe_load(f.read_text())
    print(f"{f.name}: valid YAML")
PY
```

- [ ] **Step 4: Dry-run version scripts**

```bash
python3 scripts/compute-release-version.py stable --tag stable-v2026.6.16
python3 scripts/compute-release-version.py alpha
python3 scripts/inject-release-version.py 2026.6.16-alpha.1
git checkout -- crates/desktop/Cargo.toml crates/desktop/tauri.conf.json desktop-ui/package.json
```

- [ ] **Step 5: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final release pipeline verification fixes" || true
```

---

## Self-Review

1. **Spec coverage:**
   - CI quality gates: Task 3.
   - Release version management: Tasks 4, 5, 7, 8.
   - Upgrade version (Tauri updater): Tasks 9, 10, 11.
   - Checkin version / build info: Tasks 12.
   - Build setup: Tasks 6, 7, 8.

2. **Placeholder scan:** No TBD/TODO placeholders in code; `TODO: persist settings` is explicitly scoped.

3. **Type consistency:** Settings types (`AppSettings`, `AppBuildInfo`) defined in Task 12 match the command signatures.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-06-16-release-versioning-cicd.md`.**

**Execution options:**

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per phase/task, review between tasks.
2. **Inline Execution** — execute tasks in this session using `executing-plans`, batch execution with checkpoints.

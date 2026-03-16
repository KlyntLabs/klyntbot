# macOS Distribution Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Distribute Klynt as a macOS `.dmg` with auto-updates and a unified binary that doubles as the MCP server.

**Architecture:** The desktop Tauri binary gains a `mcp serve --stdio` CLI subcommand (branching before Tauri runtime init). Tauri's updater plugin checks a GitHub Releases endpoint for updates and shows a dialog. A GitHub Actions workflow builds, signs updater artifacts, and publishes releases to `KlyntLabs/klynt-bot`.

**Tech Stack:** Tauri 2, tauri-plugin-updater, clap, rmcp (stdio transport), GitHub Actions, tauri-action

**Spec:** `docs/superpowers/specs/2026-03-15-macos-distribution-design.md`

---

## Chunk 1: Unified Binary

### Task 1: Add clap CLI parsing to desktop binary

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Add clap dependency**

In `crates/desktop/Cargo.toml`, add to `[dependencies]`:

```toml
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Define CLI struct and subcommands**

In `crates/desktop/src/main.rs`, add before the `main` function:

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "Klynt", about = "Klynt personal AI agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the MCP server
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Serve MCP over stdin/stdout
    Serve {
        /// Use stdio transport
        #[arg(long)]
        stdio: bool,
    },
}
```

- [ ] **Step 3: Branch main() before Tauri runtime**

Replace the existing `main()` function opening. The current `main()` starts with `tauri::Builder::default()` immediately. Change it to parse CLI args first and branch:

```rust
fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Mcp { command }) => {
            match command {
                McpCommands::Serve { stdio } => {
                    if stdio {
                        run_mcp_stdio();
                    } else {
                        eprintln!("Only --stdio transport is currently supported");
                        std::process::exit(1);
                    }
                }
            }
        }
        None => {
            run_desktop_app();
        }
    }
}
```

Move all existing `main()` body into a new `fn run_desktop_app()`.

- [ ] **Step 4: Guard windows_subsystem attribute**

The existing attribute at the top of `main.rs` suppresses stdout/stderr on Windows release builds. Since we're macOS-only, it has no runtime effect — but to be safe and explicit for the MCP stdio path, leave it as-is (it's a no-op on macOS).

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/main.rs
git commit -m "feat(desktop): add clap CLI with mcp subcommand skeleton"
```

### Task 2: Implement MCP stdio server in desktop binary

**Files:**
- Modify: `crates/desktop/src/main.rs`

Reference: `crates/klyntbot-server/src/main.rs` (lines 30-101) for the working MCP stdio implementation.

- [ ] **Step 1: Implement `run_mcp_stdio()`**

Add the function to `main.rs`. This closely mirrors `klyntbot-server/src/main.rs:30-87`:

```rust
fn run_mcp_stdio() {
    use std::sync::Arc;
    use klyntbot_server::handler::KlyntbotServerHandler;
    use rmcp::service::ServiceExt;
    use tracing_subscriber::EnvFilter;

    // Init tracing to stderr (stdout is reserved for MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        // Load config (same API as klyntbot-server/src/main.rs:30)
        let config = config::load_with_env_overrides()
            .await
            .expect("config load failed");

        // Init AppCore in Server mode — AppMode lives in `common` crate
        let (app, events) =
            app_core::AppCore::init(common::AppMode::Server, Some(config.clone()))
                .await
                .expect("init failed");
        let app = Arc::new(app);

        // Drain unused EventChannels — mirrors klyntbot-server/src/main.rs:45-60
        // intervention_rx is mpsc (returns None when closed),
        // pipeline_rx is broadcast (returns Err when closed)
        tokio::spawn(async move {
            let mut intervention_rx = events.intervention_rx;
            let mut pipeline_rx = events.pipeline_rx;
            let mut intervention_closed = false;
            let mut pipeline_closed = false;
            while !intervention_closed || !pipeline_closed {
                tokio::select! {
                    msg = intervention_rx.recv(), if !intervention_closed => {
                        if msg.is_none() { intervention_closed = true; }
                    }
                    result = pipeline_rx.recv(), if !pipeline_closed => {
                        if result.is_err() { pipeline_closed = true; }
                    }
                }
            }
        });

        // Build MCP handler — KlyntbotServerHandler::new is sync, takes Vec<String>
        let whitelist = config.mcp.server.exposed_tools.clone();
        let handler = KlyntbotServerHandler::new(app.clone(), whitelist);

        // Serve over stdio
        tracing::info!("Starting MCP server (stdio)");
        let transport = rmcp::transport::io::stdio();
        let service = handler.serve(transport).await.expect("Failed to serve MCP");

        tokio::select! {
            result = service.waiting() => {
                if let Err(e) = result { eprintln!("Server error: {e}"); }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down...");
            }
        }

        app.shutdown().await;
    });
}
```

- [ ] **Step 2: Add missing imports and dependencies**

Add these explicit dependencies to `crates/desktop/Cargo.toml` (Rust requires explicit dependency declarations — transitive deps are not directly usable):

```toml
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
rmcp = { workspace = true, features = ["server", "transport-io"] }
```

- [ ] **Step 3: Test MCP stdio manually**

```bash
cargo build -p desktop
echo '{"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{}},"id":1}' | ./target/debug/desktop mcp serve --stdio
```

Expected: JSON response with MCP server capabilities (tools list).

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/
git commit -m "feat(desktop): implement MCP stdio server in unified binary"
```

---

## Chunk 2: Tauri Updater Plugin

### Task 3: Add tauri-plugin-updater dependency

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/tauri.conf.json`
- Modify: `crates/desktop/capabilities/default.json`
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Generate Ed25519 signing keypair**

Run interactively (this only needs to be done once, the keys are saved):

```bash
cargo tauri signer generate -w ~/.tauri/klynt.key
```

This generates:
- `~/.tauri/klynt.key` (private key — keep secret, add to GitHub secrets as `TAURI_SIGNING_PRIVATE_KEY`)
- `~/.tauri/klynt.key.pub` (public key — goes into `tauri.conf.json`)

Note the passphrase — it goes into GitHub secrets as `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

- [ ] **Step 2: Add Cargo dependency**

In `crates/desktop/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-updater = "2"
```

- [ ] **Step 3: Add npm dependency**

```bash
cd desktop-ui && bun add @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

- [ ] **Step 4: Configure tauri.conf.json**

Add `createUpdaterArtifacts` to `bundle` and add `plugins.updater` section. In `crates/desktop/tauri.conf.json`:

Add to `"bundle"`:
```json
"createUpdaterArtifacts": true
```

Add top-level `"plugins"` key (sibling of `"bundle"`):
```json
"plugins": {
  "updater": {
    "endpoints": [
      "https://github.com/KlyntLabs/klynt-bot/releases/latest/download/latest.json"
    ],
    "pubkey": "<PASTE CONTENTS OF ~/.tauri/klynt.key.pub>"
  }
}
```

- [ ] **Step 5: Add updater capability**

In `crates/desktop/capabilities/default.json`, add to the `permissions` array:

```json
"updater:default"
```

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/tauri.conf.json crates/desktop/capabilities/default.json desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "feat(desktop): add tauri-plugin-updater dependency and config"
```

### Task 4: Register updater plugin and add update check on startup

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/main.rs`
- Create: `desktop-ui/src/shared/lib/updater.ts`
- Modify: `desktop-ui/src/App.tsx` (or root component)

- [ ] **Step 1: Add process plugin dependency (needed for relaunch)**

In `crates/desktop/Cargo.toml`, add:

```toml
tauri-plugin-process = "2"
```

- [ ] **Step 2: Register plugins in Tauri builder**

In `run_desktop_app()`, add both plugins alongside existing `.plugin()` calls:

```rust
.plugin(tauri_plugin_updater::Builder::new().build())
.plugin(tauri_plugin_process::init())
```

- [ ] **Step 3: Add process capability**

In `crates/desktop/capabilities/default.json`, add to `permissions`:

```json
"process:default"
```

- [ ] **Step 4: Create frontend update checker**

In Tauri 2, the standard pattern is to check for updates from the frontend JS. Create `desktop-ui/src/shared/lib/updater.ts`:

```typescript
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export async function checkForUpdates(): Promise<void> {
  try {
    const update = await check();
    if (update) {
      const confirmed = window.confirm(
        `Version ${update.version} is available. Update now?`
      );
      if (confirmed) {
        await update.downloadAndInstall();
        await relaunch();
      }
    }
  } catch (e) {
    console.warn('Update check failed:', e);
  }
}
```

- [ ] **Step 5: Call update check on app mount**

In the root component (e.g., `App.tsx`), add a `useEffect` that checks for updates after a short delay:

```typescript
import { useEffect } from 'react';
import { checkForUpdates } from './shared/lib/updater';

// Inside the component:
useEffect(() => {
  const timer = setTimeout(() => {
    checkForUpdates();
  }, 3000);
  return () => clearTimeout(timer);
}, []);
```

- [ ] **Step 6: Build and verify plugin registration**

```bash
cargo build -p desktop
```

Expected: Compiles without errors. Both updater and process plugins are registered.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/main.rs crates/desktop/capabilities/default.json desktop-ui/src/
git commit -m "feat(desktop): add update check with user dialog on startup"
```

---

## Chunk 3: GitHub Actions CI/CD

### Task 5: Create the release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create workflow directory**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: Write the release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: 'Release'

on:
  push:
    tags:
      - 'v*'

jobs:
  build-and-release:
    permissions:
      contents: write
    runs-on: macos-latest
    steps:
      - name: Checkout source
        uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2

      - name: Install Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: latest

      - name: Install frontend dependencies
        run: cd desktop-ui && bun install

      - name: Install cargo-nextest
        run: cargo install cargo-nextest --locked

      - name: Run tests
        run: cargo nextest run --workspace

      - name: Run clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: v__VERSION__
          releaseName: 'Klynt v__VERSION__'
          releaseBody: 'Download the .dmg to install Klynt.'
          releaseDraft: false
          prerelease: false
          args: --target aarch64-apple-darwin
          projectPath: crates/desktop
          tauriScript: cargo tauri

      - name: Upload to release repo
        env:
          GH_TOKEN: ${{ secrets.RELEASE_REPO_TOKEN }}
        run: |
          TAG_NAME="${GITHUB_REF#refs/tags/}"
          VERSION="${TAG_NAME#v}"

          # Find the built artifacts
          DMG=$(find target/aarch64-apple-darwin/release/bundle/dmg -name "*.dmg" | head -1)
          UPDATER_TAR=$(find target/aarch64-apple-darwin/release/bundle/macos -name "*.tar.gz" | head -1)
          UPDATER_SIG="${UPDATER_TAR}.sig"
          LATEST_JSON="latest.json"

          # Create latest.json for the updater
          # Note: Tauri auto-signs the .tar.gz when TAURI_SIGNING_PRIVATE_KEY is set.
          # The .sig file contains the signature string we embed in latest.json.
          SIGNATURE=$(cat "$UPDATER_SIG")
          TAR_BASENAME=$(basename "$UPDATER_TAR")
          DOWNLOAD_URL="https://github.com/KlyntLabs/klynt-bot/releases/download/${TAG_NAME}/${TAR_BASENAME}"

          cat > "$LATEST_JSON" <<ENDJSON
          {
            "version": "${VERSION}",
            "notes": "Klynt ${VERSION}",
            "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
            "platforms": {
              "darwin-aarch64": {
                "url": "${DOWNLOAD_URL}",
                "signature": "${SIGNATURE}"
              }
            }
          }
          ENDJSON

          # Create release on the public repo
          # File upload format: local_path (GitHub uses the filename as the asset name)
          gh release create "$TAG_NAME" \
            --repo KlyntLabs/klynt-bot \
            --title "Klynt ${VERSION}" \
            --notes "Download the .dmg to install Klynt." \
            "$DMG" "$UPDATER_TAR" "$LATEST_JSON"
```

- [ ] **Step 3: Verify workflow syntax**

```bash
# If you have actionlint installed:
actionlint .github/workflows/release.yml
```

Or just verify it's valid YAML:
```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub Actions release workflow for macOS distribution"
```

### Task 6: Configure GitHub secrets

This is a manual step — not code. Document for the developer.

- [ ] **Step 1: Add secrets to `KlyntLabs/klyntbot`**

Go to https://github.com/KlyntLabs/klyntbot/settings/secrets/actions and add:

| Secret | Value |
|--------|-------|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of `~/.tauri/klynt.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Passphrase entered during key generation |
| `RELEASE_REPO_TOKEN` | GitHub PAT with `repo` scope (needs write access to `KlyntLabs/klynt-bot`) |

- [ ] **Step 2: Create PAT for cross-repo releases**

Go to https://github.com/settings/tokens → "Generate new token (classic)":
- Scopes: `repo` (full control)
- Note: "Klynt release publishing"
- Copy the token and save as `RELEASE_REPO_TOKEN` secret above.

- [ ] **Step 3: Initialize the public release repo**

```bash
# Clone the public repo
git clone git@github.com:KlyntLabs/klynt-bot.git /tmp/klynt-bot
cd /tmp/klynt-bot

# Add a minimal README
echo "# Klynt\n\nPersonal AI agent. Download the latest release from the [Releases](https://github.com/KlyntLabs/klynt-bot/releases) page." > README.md
git add README.md && git commit -m "Initial commit" && git push
```

---

## Chunk 4: Version Bump and First Release

### Task 7: Create a version bump script

**Files:**
- Create: `scripts/bump-version.sh`

- [ ] **Step 1: Write the version bump script**

```bash
#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?Usage: bump-version.sh <version>}"

# Validate semver format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in semver format (e.g., 0.2.0)"
    exit 1
fi

echo "Bumping to version $VERSION..."

# 1. Cargo.toml (workspace version)
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# 2. tauri.conf.json
cd crates/desktop
python3 -c "
import json
with open('tauri.conf.json', 'r') as f:
    conf = json.load(f)
conf['version'] = '$VERSION'
with open('tauri.conf.json', 'w') as f:
    json.dump(conf, f, indent=2)
    f.write('\n')
"
cd ../..

# 3. desktop-ui/package.json
cd desktop-ui
python3 -c "
import json
with open('package.json', 'r') as f:
    pkg = json.load(f)
pkg['version'] = '$VERSION'
with open('package.json', 'w') as f:
    json.dump(pkg, f, indent=2)
    f.write('\n')
"
cd ..

echo "Version bumped to $VERSION in:"
echo "  - Cargo.toml"
echo "  - crates/desktop/tauri.conf.json"
echo "  - desktop-ui/package.json"
echo ""
echo "Next steps:"
echo "  git add -A && git commit -m 'chore: bump version to $VERSION'"
echo "  git tag v$VERSION"
echo "  git push origin main --tags"
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/bump-version.sh
```

- [ ] **Step 3: Commit**

```bash
git add scripts/bump-version.sh
git commit -m "chore: add version bump script"
```

### Task 8: Do a dry-run local build

- [ ] **Step 1: Build the Tauri app in release mode**

```bash
cargo tauri build --target aarch64-apple-darwin
```

Expected: Produces artifacts in `target/aarch64-apple-darwin/release/bundle/`:
- `dmg/Klynt_0.1.0_aarch64.dmg`
- `macos/Klynt.app.tar.gz` (updater artifact)

- [ ] **Step 2: Test the .dmg**

Open the `.dmg`, drag Klynt to Applications, launch it. Verify the app starts normally.

- [ ] **Step 3: Test MCP mode from the installed app**

```bash
echo '{"jsonrpc":"2.0","method":"initialize","params":{"capabilities":{}},"id":1}' | /Applications/Klynt.app/Contents/MacOS/Klynt mcp serve --stdio
```

Expected: JSON response with MCP server capabilities.

- [ ] **Step 4: Tag and release (when ready)**

```bash
./scripts/bump-version.sh 0.1.0
git add -A && git commit -m "chore: bump version to 0.1.0"
git checkout main && git merge dev
git tag v0.1.0
git push origin main --tags
```

The GitHub Actions workflow will build and publish the release.

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

#!/usr/bin/env python3
"""Generate a Tauri v2 compatible latest.json from built release assets."""
import argparse
import json
import re
from datetime import datetime, timezone
from pathlib import Path

PLATFORM_MAP = {
    "_aarch64.dmg": "darwin-aarch64",
    "_x86_64.dmg": "darwin-x86_64",
    "_amd64.AppImage": "linux-x86_64",
    "_x64-setup.exe": "windows-x86_64",
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
        if suffix == "_aarch64.dmg":
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

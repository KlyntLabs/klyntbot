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

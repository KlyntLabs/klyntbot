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

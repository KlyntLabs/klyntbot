#!/usr/bin/env bash
# adapt_codex_vendor.sh — mechanical rename pass for Codex-vendored crates.
#
# Plan 1: skeleton; only --help is implemented.
# Plan 2: full renames — codex_* → klynt_*, ~/.codex/ → ~/.klyntbot/, etc.
#
# Usage:
#   adapt_codex_vendor.sh --help
#   adapt_codex_vendor.sh --crate <klynt-protocol|klynt-execpolicy|...> --source <path>
#
# See spec §3 "Codex adaptation rules" for the full rename table.

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 [--help] [--crate <name>] [--source <path>]

Adapts a Codex source tree into a klynt-* crate by mechanical rename:
  - codex_*    → klynt_*    (modules)
  - CodexEvent → KlyntEvent (types)
  - ~/.codex/  → ~/.klyntbot/ (paths)
  - CODEX_API_KEY → KLYNT_API_KEY (env vars)

Plan 1: only --help is implemented. Plan 2 fills the body.
EOF
}

main() {
    local crate=""
    local source_path=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help|-h) usage; exit 0 ;;
            --crate) crate="$2"; shift 2 ;;
            --source) source_path="$2"; shift 2 ;;
            *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
        esac
    done

    if [[ -z "$crate" || -z "$source_path" ]]; then
        echo "ERROR: --crate and --source are required (Plan 2 implements adaptation)." >&2
        usage >&2
        exit 2
    fi

    echo "Plan 2 will adapt $source_path into crates/$crate/"
}

main "$@"

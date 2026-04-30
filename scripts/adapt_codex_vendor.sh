#!/usr/bin/env bash
# adapt_codex_vendor.sh — mechanical rename pass for Codex-vendored crates.
#
# Usage:
#   adapt_codex_vendor.sh --from-tar <tgz> --source <path> --dest <path> --rename old=new ...
#   adapt_codex_vendor.sh --from-dir <dir> --source <path> --dest <path> --rename old=new ...

set -euo pipefail

usage() {
    cat <<EOF
Usage: $0 --from-tar <tgz>  --source <path> --dest <path> [--rename old=new]...
       $0 --from-dir <dir> --source <path> --dest <path> [--rename old=new]...

Copies <source> from a tarball or directory into <dest>, then performs
mechanical renames (old → new) across all .rs and .toml files.
EOF
}

main() {
    local from_tar=""
    local from_dir=""
    local source=""
    local dest=""
    local renames=()
    tmp_extract=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help|-h) usage; exit 0 ;;
            --from-tar) from_tar="$2"; shift 2 ;;
            --from-dir) from_dir="$2"; shift 2 ;;
            --source) source="$2"; shift 2 ;;
            --dest) dest="$2"; shift 2 ;;
            --rename)
                renames+=("$2")
                shift 2
                ;;
            *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
        esac
    done

    if [[ -z "$source" || -z "$dest" ]]; then
        echo "ERROR: --source and --dest are required." >&2
        usage >&2
        exit 2
    fi

    if [[ -n "$from_tar" && -n "$from_dir" ]]; then
        echo "ERROR: --from-tar and --from-dir are mutually exclusive." >&2
        usage >&2
        exit 2
    fi

    if [[ -z "$from_tar" && -z "$from_dir" ]]; then
        echo "ERROR: One of --from-tar or --from-dir is required." >&2
        usage >&2
        exit 2
    fi

    # Idempotent: remove dest before copying
    rm -rf "$dest"

    local src_path=""
    if [[ -n "$from_tar" ]]; then
        tmp_extract=$(mktemp -d)
        trap '[[ -n "$tmp_extract" ]] && rm -rf "$tmp_extract"' EXIT
        tar xzf "$from_tar" -C "$tmp_extract"
        src_path="$tmp_extract/$source"
    else
        src_path="$from_dir/$source"
    fi

    if [[ ! -d "$src_path" ]]; then
        echo "ERROR: Source path does not exist: $src_path" >&2
        exit 3
    fi

    mkdir -p "$(dirname "$dest")"
    cp -R "$src_path" "$dest"

    # Apply renames
    for r in "${renames[@]}"; do
        local old_val="${r%%=*}"
        local new_val="${r#*=}"
        if [[ -z "$old_val" ]]; then
            echo "ERROR: Invalid --rename (empty old): $r" >&2
            exit 4
        fi
        find "$dest" -type f \( -name '*.rs' -o -name '*.toml' \) -exec \
            perl -pi -e "s/\\Q$old_val\\E/$new_val/g" {} +
    done
}

main "$@"

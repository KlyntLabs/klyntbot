# Phase 6 — manual git post-commit hook

Until the desktop UI installs the post-commit hook (deferred), users wire it manually.

## Per-repo install

```sh
cat > .git/hooks/post-commit <<'HOOK'
#!/usr/bin/env sh
# klyntbot Phase-6 post-commit invalidation
hash=$(git rev-parse HEAD)
parent=$(git rev-parse HEAD^ 2>/dev/null || echo null)
root=$(git rev-parse --show-toplevel)
files=$(git diff-tree --no-commit-id --name-only -r HEAD | jq -R . | jq -s .)

printf '{"commitHash":"%s","parentHash":%s,"repoRoot":"%s","changedFiles":%s}\n' \
    "$hash" \
    "$( [ "$parent" = "null" ] && echo null || printf '"%s"' "$parent" )" \
    "$root" \
    "$files" \
  | klyntbot-hook git-post-commit
HOOK
chmod +x .git/hooks/post-commit
```

The `klyntbot-hook` binary must be on `PATH`. Install via the desktop app or
`cargo install --path crates/coding-ingest` from the workspace.

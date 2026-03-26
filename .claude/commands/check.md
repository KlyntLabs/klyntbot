Run full CI checks on the workspace (format, lint, test)

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features && cargo nextest run --workspace
```

Report any failures with the exact error output. If all pass, confirm with a summary.

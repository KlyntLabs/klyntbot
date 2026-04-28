# feature-launcher

Launcher search backend: file index, search sources, and ranking.

## Benchmarks

Run benchmarks against the saved `initial` baseline:

```bash
cargo bench -p feature-launcher --bench inverted_index -- --baseline initial
```

To update the baseline after intentional algorithmic changes:

```bash
cargo bench -p feature-launcher --bench inverted_index -- --save-baseline initial
```

### Large corpus (200k entries)

Gated behind an environment variable so default `cargo bench` stays fast:

```bash
BENCH_LARGE=1 cargo bench -p feature-launcher --bench inverted_index
```

## Release checklist

- [ ] `cargo nextest run -p feature-launcher`
- [ ] `cargo clippy -p feature-launcher -- -D warnings`
- [ ] `cargo bench -p feature-launcher --bench inverted_index -- --baseline initial`
- [ ] Paste benchmark results into PR description

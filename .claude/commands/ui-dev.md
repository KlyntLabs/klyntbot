Start the desktop UI development environment

```bash
cd desktop-ui && bun install && bun run dev
```

Then in a separate terminal, start Tauri:

```bash
cargo tauri dev
```

Open http://localhost:1420 for browser-only dev (uses dev server on :3456).
